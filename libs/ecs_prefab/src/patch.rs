//! Patch path — a **separate verb** from prefab spawn.
//!
//! A patch targets an *existing* entity by global name (`@ref:label`) and either
//! merges fields into a component (`set`) or adds a whole component (`attach`).
//! It reuses the registry's serde hooks but never spawns a template.

use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::bind::NameTag;
use crate::registry::PrefabRegistry;

/// One patch operation against a named entity.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct Patch {
    /// `"@ref:label"` (or bare `"label"`) — the global name of the target.
    pub target: String,
    /// component type name -> partial fields, deep-merged onto the existing one.
    #[serde(default)]
    pub set: serde_json::Map<String, serde_json::Value>,
    /// component type name -> full value, inserted onto the target.
    #[serde(default)]
    pub attach: serde_json::Map<String, serde_json::Value>,
}

impl PrefabRegistry {
    /// Apply one patch. Returns `false` if the target name is unknown.
    pub fn apply_patch(&self, world: &mut World, patch: &Patch) -> bool {
        let name = patch.target.strip_prefix("@ref:").unwrap_or(&patch.target);
        let Some(target) = find_by_nametag(world, name) else {
            return false;
        };

        // set: serialize current component, deep-merge fields, decode back
        for (comp, fields) in &patch.set {
            if let Some(factory) = self.archive_registry.entries.get(comp.as_str()) {
                if let Some(mut cur) = (factory.js_value.export)(world, target) {
                    json_merge(&mut cur, fields);
                    (factory.js_value.import)(&cur, world, target).expect("patch set decode");
                }
            }
        }
        // attach: insert a whole component
        for (comp, val) in &patch.attach {
            if let Some(factory) = self.archive_registry.entries.get(comp.as_str()) {
                (factory.js_value.import)(val, world, target).expect("patch attach decode");
            }
        }
        true
    }
}

fn find_by_nametag(world: &mut World, name: &str) -> Option<Entity> {
    let mut q = world.query::<(Entity, &NameTag)>();
    q.iter(world).find(|(_, t)| t.0 == name).map(|(e, _)| e)
}

/// Deep-merge a JSON patch into a value (objects merge field-wise; leaves replace).
fn json_merge(dst: &mut serde_json::Value, patch: &serde_json::Value) {
    match (dst, patch) {
        (serde_json::Value::Object(d), serde_json::Value::Object(p)) => {
            for (k, v) in p {
                json_merge(d.entry(k.clone()).or_insert(serde_json::Value::Null), v);
            }
        }
        (d, p) => *d = p.clone(),
    }
}
