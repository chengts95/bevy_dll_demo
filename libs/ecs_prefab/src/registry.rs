//! Type registry + per-type serde verbs + flat (de)serialization.
//!
//! Components are (de)serialized through monomorphized fn pointers keyed by
//! short type name — zero reflection, no `bevy_archive` dependency. The one
//! reference edge, `ChildOf`, is stored as a `u32` index and rebuilt through a
//! `u32 -> Entity` remap.

use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::*;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::bind::{NameTag, PrefabEntityId, Refs};
use crate::format::{EntityRecord, Local, Net, Pins, PrefabClass, Snapshot, Use};
use crate::subst::{is_param_symbol, lookup_param_path, param_name, try_resolve_globals};

use bevy_archive::bevy_registry::SnapshotRegistry;

const PREFAB_DATA_COMPONENTS: &[&str] = &["Use", "Local", "Net", "Pins", "Refs", "NameTag"];
const STRUCTURAL_COMPONENTS: &[&str] = &["ChildOf"];

// ── registry ────────────────────────────────────────────────────────────────

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct PrefabRoot {
    pub name: String,
    pub params_def: serde_json::Map<String, serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ParamOverride {
    pub entity_id: u32,
    pub component_name: String,
    // We could store json paths here, but for now we'll just store the raw JSON tree
    // that contains "$param" variables, so during substitution we can just use it directly!
    pub original_json: serde_json::Value,
}

#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct ParameterMapping {
    pub overrides: Vec<ParamOverride>,
}

/// Type name -> serde verbs, plus a global symbol table. A plain resource.
#[derive(Resource)]
pub struct PrefabRegistry {
    pub archive_registry: SnapshotRegistry,
    /// Global symbol table for `@x` references (e.g. `gnd`). Domain-populated;
    /// the framework only provides the `@` lookup mechanism.
    pub(crate) globals: serde_json::Map<String, serde_json::Value>,
    /// Type names whose value is a list of net symbols (e.g. `PortNames`). The
    /// **only** domain knowledge the framework needs, so it can qualify a
    /// nested instance's internal net names during recursive composition.
    pub(crate) net_components: HashSet<String>,
    /// The Dual-World ECS storage for strongly-typed templates.
    pub prefab_world: World,
    /// Maps prefab name to its root Entity in the prefab_world.
    pub templates: HashMap<String, Entity>,
}

impl Default for PrefabRegistry {
    fn default() -> Self {
        Self::from_snapshot_registry(SnapshotRegistry::default())
    }
}

impl PrefabRegistry {
    /// Rebuild the `templates` hashmap from the `PrefabRoot` components in the world.
    /// This is necessary after loading the `prefab_world` from an external archive.
    pub fn rebuild_templates(&mut self) {
        self.templates.clear();
        let mut query = self.prefab_world.query::<(Entity, &PrefabRoot)>();
        for (entity, root) in query.iter(&self.prefab_world) {
            self.templates.insert(root.name.clone(), entity);
        }
    }
}

impl Clone for PrefabRegistry {
    fn clone(&self) -> Self {
        if !self.templates.is_empty() {
            panic!(
                "cannot clone PrefabRegistry after load_library; prefab_world templates are not cloneable"
            );
        }
        Self {
            archive_registry: self.archive_registry.clone(),
            globals: self.globals.clone(),
            net_components: self.net_components.clone(),
            prefab_world: World::new(),
            templates: HashMap::new(),
        }
    }
}

impl PrefabRegistry {
    /// Create a PrefabRegistry from an existing SnapshotRegistry.
    pub fn from_snapshot_registry(
        archive_registry: bevy_archive::bevy_registry::SnapshotRegistry,
    ) -> Self {
        let mut registry = Self {
            archive_registry,
            globals: Default::default(),
            net_components: Default::default(),
            prefab_world: World::new(),
            templates: Default::default(),
        };
        registry.register_prefab_components();
        registry
    }

    fn register_prefab_components(&mut self) {
        self.register_prefab_component::<Use>();
        self.register_prefab_component::<Local>();
        self.register_prefab_component::<Net>();
        self.register_prefab_component::<Pins>();
        self.register_prefab_component::<Refs>();
        self.register_prefab_component::<NameTag>();
        self.drop_structural_components();
    }

    fn register_prefab_component<T>(&mut self)
    where
        T: Component + Serialize + DeserializeOwned,
    {
        let name = bevy_archive::bevy_registry::short_type_name::<T>();
        if !self.archive_registry.entries.contains_key(name) {
            self.archive_registry.register::<T>();
        }
    }

    fn drop_structural_components(&mut self) {
        for name in STRUCTURAL_COMPONENTS {
            self.archive_registry.entries.remove(name);
        }
    }

    fn is_prefab_data_component(name: &str) -> bool {
        PREFAB_DATA_COMPONENTS.contains(&name)
    }

    fn is_structural_component(name: &str) -> bool {
        STRUCTURAL_COMPONENTS.contains(&name)
    }

    /// Register one component type's serde verbs.
    pub fn register<T>(&mut self) -> &mut Self
    where
        T: Component + Serialize + DeserializeOwned,
    {
        self.try_register::<T>()
            .unwrap_or_else(|err| panic!("{err}"))
    }

    /// Fallible component registration. Duplicate names are rejected here so a
    /// caller can patch/overlay intentionally via [`merge_from`] instead of
    /// silently replacing a hook during default registration.
    pub fn try_register<T>(&mut self) -> Result<&mut Self, String>
    where
        T: Component + Serialize + DeserializeOwned + 'static,
    {
        // the snapshot registry allows registering named.
        let name = bevy_archive::bevy_registry::short_type_name::<T>();
        if Self::is_structural_component(name) {
            self.drop_structural_components();
            return Ok(self);
        }
        if self.archive_registry.entries.contains_key(name) {
            if Self::is_prefab_data_component(name) {
                return Ok(self);
            }
            return Err(format!("component '{name}' is already registered"));
        }
        self.archive_registry.register::<T>();
        self.drop_structural_components();
        Ok(self)
    }

    /// Register a global symbol resolvable as `@name` (e.g. `("gnd", "gnd")`).
    pub fn register_global(&mut self, name: &str, value: serde_json::Value) -> &mut Self {
        self.try_register_global(name, value)
            .unwrap_or_else(|err| panic!("{err}"))
    }

    /// Fallible global registration. Use [`merge_from`] when replacement is
    /// intended as an overlay.
    pub fn try_register_global(
        &mut self,
        name: &str,
        value: serde_json::Value,
    ) -> Result<&mut Self, String> {
        if self.globals.contains_key(name) {
            return Err(format!("global '{name}' is already registered"));
        }
        self.globals.insert(name.to_string(), value);
        Ok(self)
    }

    fn check_has_param(val: &serde_json::Value, has_param: &mut bool) {
        match val {
            serde_json::Value::String(s) => {
                if is_param_symbol(s) {
                    *has_param = true;
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    Self::check_has_param(v, has_param);
                }
            }
            serde_json::Value::Object(obj) => {
                for v in obj.values() {
                    Self::check_has_param(v, has_param);
                }
            }
            _ => {}
        }
    }

    fn replace_with_defaults(
        val: &mut serde_json::Value,
        params_def: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), String> {
        match val {
            serde_json::Value::String(s) => {
                if let Some(param_name) = param_name(s) {
                    let Some(def_val) = lookup_param_path(params_def, param_name) else {
                        return Err(format!("unknown param '${}'", param_name));
                    };
                    *val = def_val.clone();
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    Self::replace_with_defaults(v, params_def)?;
                }
            }
            serde_json::Value::Object(obj) => {
                for v in obj.values_mut() {
                    Self::replace_with_defaults(v, params_def)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Load a Library (parsed from JSON) into the PrefabWorld as strongly-typed entities.
    pub fn load_library(&mut self, lib: &crate::format::Library) -> Result<(), String> {
        for (name, class) in lib {
            if self.templates.contains_key(name) {
                return Err(format!("duplicate prefab class '{name}'"));
            }
            let mut seen_ids = HashSet::new();
            for record in &class.body {
                if !seen_ids.insert(record.id) {
                    return Err(format!(
                        "prefab '{name}': duplicate record id {}",
                        record.id
                    ));
                }
            }
            let root = self
                .prefab_world
                .spawn(PrefabRoot {
                    name: name.clone(),
                    params_def: class.params.clone(),
                })
                .id();

            let mut overrides = Vec::new();
            let mut id_map = HashMap::new();

            for record in &class.body {
                let mut e_mut = self.prefab_world.spawn_empty();
                let e = e_mut.id();
                id_map.insert(record.id, e);
                e_mut.insert(crate::bind::PrefabEntityId {
                    instance: root,
                    local: record.id,
                });
            }

            for record in &class.body {
                let e = id_map[&record.id];

                if let Some(parent_id) = record.parent {
                    if let Some(&parent_e) = id_map.get(&parent_id) {
                        self.prefab_world.entity_mut(e).insert(ChildOf(parent_e));
                    } else {
                        return Err(format!(
                            "prefab '{name}' record {}: parent id {parent_id} not found",
                            record.id
                        ));
                    }
                } else {
                    self.prefab_world.entity_mut(e).insert(ChildOf(root));
                }

                for (comp_name, comp_val) in &record.components {
                    let mut mut_val = comp_val.clone();
                    let mut has_param = false;
                    Self::check_has_param(&mut_val, &mut has_param);

                    if has_param {
                        overrides.push(ParamOverride {
                            entity_id: record.id,
                            component_name: comp_name.clone(),
                            original_json: comp_val.clone(),
                        });
                        Self::replace_with_defaults(&mut mut_val, &class.params)?;
                    }

                    let Some(factory) = self.archive_registry.entries.get(comp_name.as_str())
                    else {
                        return Err(format!(
                            "prefab '{name}' record {}: unknown component '{comp_name}'",
                            record.id
                        ));
                    };
                    let res = (factory.js_value.import)(&mut_val, &mut self.prefab_world, e);
                    if res.is_err() && !has_param {
                        return Err(format!(
                            "record {}: component '{}' error",
                            record.id, comp_name
                        ));
                    }
                }
            }

            self.prefab_world
                .entity_mut(root)
                .insert(ParameterMapping { overrides });
            self.templates.insert(name.clone(), root);
        }
        Ok(())
    }

    /// Export one loaded prefab template back into the pure data format.
    ///
    /// This is a semantic roundtrip, not a byte-for-byte JSON roundtrip:
    /// formatting and object ordering are serde's concern, while entity ids,
    /// hierarchy, params, components, and unresolved `$`/`@` template symbols
    /// are preserved.
    pub fn try_export_class(&self, name: &str) -> Result<PrefabClass, String> {
        let root = self
            .templates
            .get(name)
            .ok_or_else(|| format!("unknown prefab '{name}' (did you call load_library?)"))?;
        let root_comp = self
            .prefab_world
            .get::<PrefabRoot>(*root)
            .ok_or_else(|| format!("prefab '{name}' root is missing PrefabRoot"))?;
        if root_comp.name != name {
            return Err(format!(
                "prefab registry corrupted: template '{name}' stored as '{}'",
                root_comp.name
            ));
        }

        let subtree = collect_subtree(&self.prefab_world, *root);
        let mut body = self.flatten_filter(&self.prefab_world, |e| subtree.contains(&e));

        let param_mapping = self
            .prefab_world
            .get::<ParameterMapping>(*root)
            .ok_or_else(|| format!("prefab '{name}' root is missing ParameterMapping"))?;
        for override_item in &param_mapping.overrides {
            if let Some(record) = body.iter_mut().find(|r| r.id == override_item.entity_id) {
                record.components.insert(
                    override_item.component_name.clone(),
                    override_item.original_json.clone(),
                );
            }
        }

        Ok(PrefabClass {
            params: root_comp.params_def.clone(),
            body,
        })
    }

    /// Overlay and fully resolve a global symbol table.
    ///
    /// Overlay values may reference existing or sibling globals. Unknown names
    /// and cycles are rejected before the registry is mutated.
    pub fn try_overlay_globals(
        &mut self,
        overlay: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<&mut Self, String> {
        let mut merged = self.globals.clone();
        merged.extend(overlay.clone());

        let mut resolved = serde_json::Map::new();
        for name in merged.keys() {
            let symbol = serde_json::Value::String(format!("@{name}"));
            let value = try_resolve_globals(&symbol, &merged)
                .map_err(|err| format!("global '{name}': {err}"))?;
            resolved.insert(name.clone(), value);
        }
        self.globals = resolved;
        Ok(self)
    }

    /// Mark `T` as a net-name component (a `[name, …]` list), so `spawn_class`
    /// qualifies a nested instance's internal nets. e.g. `register_net::<PortNames>()`.
    pub fn register_net<T>(&mut self) -> &mut Self {
        self.try_register_net::<T>()
            .unwrap_or_else(|err| panic!("{err}"))
    }

    /// Fallible net marker registration. Re-registering the same marker is
    /// accepted as idempotent because markers are a capability set.
    pub fn try_register_net<T>(&mut self) -> Result<&mut Self, String> {
        self.net_components
            .insert(bevy_archive::bevy_registry::short_type_name::<T>().to_string());
        Ok(self)
    }

    /// Overlay another registry on top of this one.
    ///
    /// Existing type hooks and globals are replaced by `other`; net-component
    /// markers are unioned because they are capabilities, not values.
    pub fn merge_from(&mut self, other: &PrefabRegistry) -> &mut Self {
        use bevy_archive::bevy_registry::SnapshotMerge;
        self.archive_registry.merge(&other.archive_registry);
        self.register_prefab_components();
        self.globals.extend(other.globals.clone());
        self.net_components.extend(other.net_components.clone());
        self
    }

    /// Return a patched clone, leaving the base registry untouched.
    pub fn merged_with(&self, patch: &PrefabRegistry) -> Self {
        let mut merged = self.clone();
        merged.merge_from(patch);
        merged
    }

    pub fn is_registered(&self, name: &str) -> bool {
        self.archive_registry.entries.contains_key(name)
    }
}

// ── flatten / rebuild ───────────────────────────────────────────────────────

impl PrefabRegistry {
    /// Flatten a world: every entity carrying ≥1 registered component becomes a
    /// record. Deterministic (sorted by id) so files diff cleanly.
    pub fn flatten(&self, world: &World) -> Snapshot {
        self.flatten_filter(world, |_| true)
    }

    pub fn flatten_filter(&self, world: &World, keep: impl Fn(Entity) -> bool) -> Snapshot {
        let mut out = Vec::new();
        for er in world.iter_entities() {
            let e = er.id();
            if !keep(e) {
                continue;
            }
            let mut components = serde_json::Map::new();
            for (name, factory) in &self.archive_registry.entries {
                if let Some(v) = (factory.js_value.export)(world, e) {
                    components.insert(name.to_string(), v);
                }
            }
            let has_prefab_id = world.get::<crate::bind::PrefabEntityId>(e).is_some();
            if components.is_empty() && !has_prefab_id {
                continue;
            }

            let local_id = world
                .get::<crate::bind::PrefabEntityId>(e)
                .map_or(e.index_u32(), |p| p.local);
            let parent_id = world.get::<ChildOf>(e).and_then(|c| {
                if world.get::<crate::bind::PrefabEntityId>(e).is_some() {
                    world
                        .get::<crate::bind::PrefabEntityId>(c.parent())
                        .map(|p| p.local)
                } else {
                    Some(c.parent().index_u32())
                }
            });

            out.push(EntityRecord {
                id: local_id,
                parent: parent_id,
                components,
            });
        }
        out.sort_by_key(|r| r.id);
        out
    }

    /// Rebuild entities from records. Returns the `old id -> new entity` map.
    /// `ChildOf` is remapped through that map (the single reference verb).
    pub fn rebuild(&self, snap: &Snapshot, world: &mut World) -> HashMap<u32, Entity> {
        self.try_rebuild(snap, world)
            .unwrap_or_else(|err| panic!("{err}"))
    }

    /// Fallible rebuild variant for user-loaded libraries and cases. Returns
    /// contextual errors instead of panicking on unknown components or bad JSON.
    pub fn try_rebuild(
        &self,
        snap: &Snapshot,
        world: &mut World,
    ) -> Result<HashMap<u32, Entity>, String> {
        let mut seen = HashSet::new();
        for record in snap {
            if !seen.insert(record.id) {
                return Err(format!("duplicate record id {}", record.id));
            }
        }
        let mut map: HashMap<u32, Entity> = HashMap::new();
        for r in snap {
            map.insert(r.id, world.spawn_empty().id());
        }
        if let Some(first) = snap.first() {
            let instance = map[&first.id];
            for record in snap {
                world.entity_mut(map[&record.id]).insert(PrefabEntityId {
                    instance,
                    local: record.id,
                });
            }
        }
        for r in snap {
            let e = map[&r.id];
            for (name, v) in &r.components {
                let Some(factory) = self.archive_registry.entries.get(name.as_str()) else {
                    return Err(format!("record {}: unknown component '{name}'", r.id));
                };
                (factory.js_value.import)(v, world, e)
                    .map_err(|err| format!("record {} component '{name}': {err}", r.id))?;
            }
            if let Some(p) = r.parent {
                if let Some(&pe) = map.get(&p) {
                    world.entity_mut(e).insert(ChildOf(pe));
                } else {
                    return Err(format!("record {}: parent id {p} not found", r.id));
                }
            }
        }
        Ok(map)
    }
}

// ── graph traversal ─────────────────────────────────────────────────────────

/// Collect a whole ownership subtree (BFS over `Children`), root first.
pub fn collect_subtree(world: &World, root: Entity) -> Vec<Entity> {
    let mut out = vec![root];
    let mut i = 0;
    while i < out.len() {
        let e = out[i];
        i += 1;
        if let Some(children) = world.get::<Children>(e) {
            for c in children.iter() {
                out.push(c);
            }
        }
    }
    out
}
