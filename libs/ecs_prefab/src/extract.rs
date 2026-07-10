//! Reverse direction: extract a prefab class from a selection of live entities,
//! auto-detecting boundary ports (a net touched both inside and outside the
//! selection is a port).

use bevy_ecs::prelude::*;
use std::collections::HashSet;

use crate::format::{Net, PrefabClass};
use crate::registry::PrefabRegistry;

impl PrefabRegistry {
    /// Capture the selected entities into a reusable [`PrefabClass`]: the body
    /// is the flattened selection; boundary nets are auto-detected (touched both
    /// inside and outside the selection is a boundary port). `params` is left
    /// empty — parametrize the captured template by hand if desired.
    pub fn extract(&self, world: &World, selected: &[Entity]) -> PrefabClass {
        let sel: HashSet<Entity> = selected.iter().copied().collect();

        let mut inside: HashSet<String> = HashSet::new();
        let mut outside: HashSet<String> = HashSet::new();
        for er in world.iter_entities() {
            if let Some(Net(names)) = er.get::<Net>() {
                let bucket = if sel.contains(&er.id()) {
                    &mut inside
                } else {
                    &mut outside
                };
                for n in names {
                    bucket.insert(n.clone());
                }
            }
        }
        let mut boundary: Vec<String> = inside.intersection(&outside).cloned().collect();
        boundary.sort();

        let mut body = self.flatten_filter(world, |e| sel.contains(&e));
        for record in &mut body {
            let Some(names) = record
                .components
                .get_mut("Net")
                .and_then(serde_json::Value::as_array_mut)
            else {
                continue;
            };
            for name in names {
                let Some(symbol) = name.as_str() else {
                    continue;
                };
                if boundary.iter().any(|port| port == symbol) {
                    *name = serde_json::Value::String(format!("${symbol}"));
                }
            }
        }
        let params = boundary
            .into_iter()
            .map(|name| (name, serde_json::Value::Null))
            .collect();
        PrefabClass { params, body }
    }
}
