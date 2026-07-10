//! Load path: instantiate a prefab class into a target world.
//!
//! `substitute` → `rebuild` → resolve nets → lower `Pins` → expand `Use`
//! recursively (arbitrary nesting depth).

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use std::collections::{HashMap, HashSet};

use crate::format::{Library, Net, Pins, Use};
use crate::registry::PrefabRegistry;
use crate::subst::{
    lookup_param_path_mut, param_name, qualify, substitute, try_resolve_globals, try_substitute,
};

const NS_SEP: &str = "::";

fn entities_in_body_order(
    body: &[crate::format::EntityRecord],
    map: &HashMap<u32, Entity>,
) -> Vec<Entity> {
    body.iter().map(|record| map[&record.id]).collect()
}

fn namespace_of(name: &str) -> Option<&str> {
    name.rsplit_once(NS_SEP).map(|(namespace, _)| namespace)
}

fn resolve_prefab_name(
    lib: &Library,
    current: Option<&str>,
    reference: &str,
) -> Result<String, String> {
    if reference.contains(NS_SEP) {
        return lib
            .contains_key(reference)
            .then(|| reference.to_string())
            .ok_or_else(|| format!("unknown prefab '{reference}'"));
    }

    if let Some(namespace) = current.and_then(namespace_of) {
        let candidate = format!("{namespace}{NS_SEP}{reference}");
        if lib.contains_key(&candidate) {
            return Ok(candidate);
        }
    }

    lib.contains_key(reference)
        .then(|| reference.to_string())
        .ok_or_else(|| {
            let scope = current
                .and_then(namespace_of)
                .map(|namespace| format!(" in namespace '{namespace}'"))
                .unwrap_or_default();
            format!("unknown prefab '{reference}'{scope}")
        })
}

fn collect_param_refs(value: &serde_json::Value, refs: &mut HashSet<String>) {
    match value {
        serde_json::Value::String(symbol) => {
            if let Some(name) = param_name(symbol) {
                refs.insert(name.to_string());
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_param_refs(item, refs);
            }
        }
        serde_json::Value::Object(fields) => {
            for item in fields.values() {
                collect_param_refs(item, refs);
            }
        }
        _ => {}
    }
}

fn merge_json(dst: &mut serde_json::Value, src: serde_json::Value) {
    match (dst, src) {
        (serde_json::Value::Object(dst), serde_json::Value::Object(src)) => {
            for (key, value) in src {
                if let Some(dst_value) = dst.get_mut(&key) {
                    merge_json(dst_value, value);
                } else {
                    dst.insert(key, value);
                }
            }
        }
        (dst, src) => *dst = src,
    }
}

fn merge_param_overlay(
    params: &mut serde_json::Map<String, serde_json::Value>,
    overlay: &serde_json::Map<String, serde_json::Value>,
) {
    for (key, value) in overlay {
        if let Some(dst) = params.get_mut(key) {
            merge_json(dst, value.clone());
        } else {
            params.insert(key.clone(), value.clone());
        }
    }
}

fn find_null_path(value: &serde_json::Value, path: &str) -> Option<String> {
    match value {
        serde_json::Value::Null => Some(path.to_string()),
        serde_json::Value::Array(items) => items
            .iter()
            .enumerate()
            .find_map(|(index, item)| find_null_path(item, &format!("{path}[{index}]"))),
        serde_json::Value::Object(fields) => fields.iter().find_map(|(field, item)| {
            let child_path = if path.is_empty() {
                field.clone()
            } else {
                format!("{path}.{field}")
            };
            find_null_path(item, &child_path)
        }),
        _ => None,
    }
}

fn collect_string_values(value: &serde_json::Value, values: &mut Vec<String>) {
    match value {
        serde_json::Value::String(value) => values.push(value.clone()),
        serde_json::Value::Array(items) => {
            for item in items {
                collect_string_values(item, values);
            }
        }
        serde_json::Value::Object(fields) => {
            for item in fields.values() {
                collect_string_values(item, values);
            }
        }
        _ => {}
    }
}

fn net_params_for_class(
    lib: &Library,
    name: &str,
    net_components: &HashSet<String>,
    cache: &mut HashMap<String, HashSet<String>>,
    visiting: &mut HashSet<String>,
) -> Result<HashSet<String>, String> {
    if let Some(params) = cache.get(name) {
        return Ok(params.clone());
    }
    if !visiting.insert(name.to_string()) {
        return Err(format!("cyclic prefab composition involving '{name}'"));
    }
    let class = lib
        .get(name)
        .ok_or_else(|| format!("unknown prefab '{name}'"))?;
    let mut result = HashSet::new();
    for record in &class.body {
        for (component, value) in &record.components {
            if net_components.contains(component) {
                collect_param_refs(value, &mut result);
                continue;
            }
            if component != "Use" {
                continue;
            }
            let use_component: Use = serde_json::from_value(value.clone())
                .map_err(|err| format!("prefab '{name}' has invalid Use: {err}"))?;
            let child_name = resolve_prefab_name(lib, Some(name), &use_component.prefab)
                .map_err(|err| format!("prefab '{name}' Use '{}': {err}", use_component.prefab))?;
            let child_params =
                net_params_for_class(lib, &child_name, net_components, cache, visiting)?;
            for child_param in child_params {
                if let Some(argument) = use_component.params.get(&child_param) {
                    collect_param_refs(argument, &mut result);
                }
            }
        }
    }
    visiting.remove(name);
    cache.insert(name.to_string(), result.clone());
    Ok(result)
}

fn qualify_net_value(value: &mut serde_json::Value, prefix: &str) {
    match value {
        serde_json::Value::String(symbol) => {
            if !symbol.starts_with('$') && !symbol.starts_with('@') {
                *symbol = format!("{prefix}{symbol}");
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                qualify_net_value(item, prefix);
            }
        }
        serde_json::Value::Object(fields) => {
            for item in fields.values_mut() {
                qualify_net_value(item, prefix);
            }
        }
        _ => {}
    }
}

fn qualify_use_params(
    body: &mut crate::format::Snapshot,
    lib: &Library,
    current: &str,
    net_components: &HashSet<String>,
    prefix: &str,
) -> Result<(), String> {
    if prefix.is_empty() {
        return Ok(());
    }
    let mut cache = HashMap::new();
    let mut visiting = HashSet::new();
    for record in body {
        let Some(value) = record.components.get_mut("Use") else {
            continue;
        };
        let prefab = value
            .get("prefab")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                format!(
                    "prefab '{current}' record {} has invalid Use.prefab",
                    record.id
                )
            })?;
        let child_name = resolve_prefab_name(lib, Some(current), prefab)
            .map_err(|err| format!("prefab '{current}' Use '{prefab}': {err}"))?;
        let net_params =
            net_params_for_class(lib, &child_name, net_components, &mut cache, &mut visiting)?;
        let Some(params) = value
            .get_mut("params")
            .and_then(serde_json::Value::as_object_mut)
        else {
            continue;
        };
        for name in net_params {
            if let Some(argument) = lookup_param_path_mut(params, &name) {
                qualify_net_value(argument, prefix);
            }
        }
    }
    Ok(())
}

/// Sequential node-id allocator; held by the caller so successive instances get
/// distinct internal nodes.
pub struct NodeAlloc {
    pub next: u32,
}
impl NodeAlloc {
    pub fn new(start: u32) -> Self {
        Self { next: start }
    }
    pub fn alloc(&mut self) -> u32 {
        let n = self.next;
        self.next += 1;
        n
    }
}

impl PrefabRegistry {
    /// Domain-handoff entry: substitute params (`$`/`@`), qualify a nested
    /// instance's internal net names, rebuild, and **recursively expand `Use`** —
    /// leaving net/node resolution to the domain (e.g. emt_core's `resolve_ports`,
    /// which keeps the names in `NodeNames` for the measurement system). Produces
    /// net **names** (no `Pins`): "data to prefab, symbols to the domain".
    ///
    /// `prefix` qualifies this instance's internal nets (`""` at the top level so
    /// the outermost nets stay as authored and remain probeable by name).
    /// Ports are ordinary declared `$param` values. Returns this instance's
    /// directly-materialized entities.
    pub fn spawn_class(
        &self,
        lib: &Library,
        name: &str,
        params_in: &serde_json::Map<String, serde_json::Value>,
        prefix: &str,
        world: &mut World,
    ) -> Vec<Entity> {
        self.try_spawn_class(lib, name, params_in, prefix, world)
            .unwrap_or_else(|err| panic!("{err}"))
    }

    /// Fallible domain-handoff entry. Namespace resolution is intentionally
    /// simple: `a::b` is absolute; bare `b` first resolves beside the current
    /// class, then falls back to a global bare class name.
    pub fn try_spawn_class(
        &self,
        lib: &Library,
        name: &str,
        params_in: &serde_json::Map<String, serde_json::Value>,
        prefix: &str,
        world: &mut World,
    ) -> Result<Vec<Entity>, String> {
        let resolved = resolve_prefab_name(lib, None, name)?;
        self.try_spawn_class_resolved(lib, &resolved, params_in, prefix, world)
    }

    fn try_spawn_class_resolved(
        &self,
        lib: &Library,
        name: &str,
        params_in: &serde_json::Map<String, serde_json::Value>,
        prefix: &str,
        world: &mut World,
    ) -> Result<Vec<Entity>, String> {
        let class = self.try_export_class(name)?;

        for key in params_in.keys() {
            if !class.params.contains_key(key) {
                return Err(format!("prefab '{name}': unknown param '{key}'"));
            }
        }

        // params: declared defaults overlaid with caller-supplied (ports included)
        let mut params = class.params.clone();
        merge_param_overlay(&mut params, params_in);
        for (key, value) in &mut params {
            *value = try_resolve_globals(value, &self.globals)
                .map_err(|err| format!("prefab '{name}' param '{key}': {err}"))?;
        }
        if let Some(key) = params
            .iter()
            .find_map(|(key, value)| find_null_path(value, key))
        {
            return Err(format!(
                "prefab '{name}': required param '{key}' was not provided"
            ));
        }

        let mut body = class.body;

        // qualify internal net names, then resolve `$`/`@`, then rebuild
        qualify_use_params(&mut body, lib, name, &self.net_components, prefix)?;
        qualify(&mut body, &self.net_components, prefix);
        try_substitute(&mut body, &params, &self.globals)
            .map_err(|err| format!("prefab '{name}': {err}"))?;
        let map = self
            .try_rebuild(&body, world)
            .map_err(|err| format!("prefab '{name}': {err}"))?;

        let entities = entities_in_body_order(&body, &map);

        // recursively expand `Use`: all child inputs come from declared params
        let uses: Vec<(Entity, Use)> = entities
            .iter()
            .filter_map(|&e| world.get::<Use>(e).cloned().map(|u| (e, u)))
            .collect();
        for (i, (e, u)) in uses.into_iter().enumerate() {
            let child_prefix = format!("{prefix}{i}.");
            let child_name = resolve_prefab_name(lib, Some(name), &u.prefab)
                .map_err(|err| format!("prefab '{name}' Use '{}': {err}", u.prefab))?;
            let kids =
                self.try_spawn_class_resolved(lib, &child_name, &u.params, &child_prefix, world)?;
            for &k in &kids {
                if world.get::<ChildOf>(k).is_none() {
                    world.entity_mut(k).insert(ChildOf(e));
                }
            }
            world.entity_mut(e).remove::<Use>();
        }

        // return the instances, excluding nested ones
        let mut children = Vec::new();
        for r in &body {
            if r.parent.is_none() {
                children.push(map[&r.id]);
            }
        }
        Ok(children)
    }

    /// Instantiate a class recursively: translate params at the Value level,
    /// rebuild, resolve nets, then expand every `Use` child at any depth.
    /// Returns this instance's directly-materialized entities.
    pub fn instantiate_class(
        &self,
        lib: &Library,
        name: &str,
        params_in: &serde_json::Map<String, serde_json::Value>,
        binding: &HashMap<String, u32>,
        alloc: &mut NodeAlloc,
        world: &mut World,
    ) -> Vec<Entity> {
        let class = lib
            .get(name)
            .unwrap_or_else(|| panic!("unknown prefab '{name}'"));

        // 1. params: declared defaults overlaid with what the caller passed in
        let mut params = class.params.clone();
        merge_param_overlay(&mut params, params_in);
        for value in params.values_mut() {
            *value = try_resolve_globals(value, &self.globals)
                .unwrap_or_else(|err| panic!("prefab '{name}': {err}"));
        }

        // 2. translate params at the serde Value level: `$x` from this scope's
        //    params, `@x` from the global table
        let mut body = class.body.clone();
        substitute(&mut body, &params, &self.globals);

        // 3. rebuild the body into the world (entities + ChildOf)
        let map = self.rebuild(&body, world);
        let entities = entities_in_body_order(&body, &map);

        // 4. resolve nets: gnd=0, caller bindings preserved, rest freshened
        let mut nets: HashMap<String, u32> = HashMap::from([("gnd".to_string(), 0)]);
        for (name, &id) in binding {
            nets.insert(name.clone(), id);
        }
        for &e in &entities {
            if let Some(Net(names)) = world.get::<Net>(e) {
                for n in names.clone() {
                    nets.entry(n).or_insert_with(|| alloc.alloc());
                }
            }
            if let Some(u) = world.get::<Use>(e).cloned() {
                let mut values = Vec::new();
                for argument in u.params.values() {
                    collect_string_values(argument, &mut values);
                }
                for value in values {
                    nets.entry(value).or_insert_with(|| alloc.alloc());
                }
            }
        }

        // 5. lower leaf terminals to integer pins
        for &e in &entities {
            if let Some(Net(names)) = world.get::<Net>(e).cloned() {
                let ids = names.iter().map(|n| nets[n]).collect();
                world.entity_mut(e).insert(Pins(ids));
            }
        }

        // 6. expand Use children recursively (arbitrary nesting depth)
        let uses: Vec<(Entity, Use)> = entities
            .iter()
            .filter_map(|&e| world.get::<Use>(e).cloned().map(|u| (e, u)))
            .collect();
        for (e, u) in uses {
            let mut child_binding = HashMap::new();
            for argument in u.params.values() {
                let mut values = Vec::new();
                collect_string_values(argument, &mut values);
                for value in values {
                    if let Some(&node) = nets.get(&value) {
                        child_binding.insert(value, node);
                    }
                }
            }
            let children =
                self.instantiate_class(lib, &u.prefab, &u.params, &child_binding, alloc, world);
            for &c in &children {
                if world.get::<ChildOf>(c).is_none() {
                    world.entity_mut(c).insert(ChildOf(e));
                }
            }
            world.entity_mut(e).remove::<Use>();
        }

        entities
    }
}
