//! Entity-reference resolution (post-spawn, derived).
//!
//! References are persistent **names** that resolve to live `Entity` handles in
//! a re-runnable pass — never substituted at the Value level. Two scopes:
//!
//! - `@ref:label` — global: an entity tagged `NameTag(label)`.
//! - `$ref:label` — local: a sibling (same `ChildOf` parent) named `Local(label)`.
//!
//! An entity lists its references in a `Refs` component (role -> ref string);
//! the pass writes the resolved `Bound` (role -> Entity) — `Refs` is data,
//! `Bound` is the derived cache (not serialized).

use bevy_ecs::hierarchy::{ChildOf, Children};
use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::format::Local;

/// Globally addressable name on an entity (`@ref:label` resolves to it).
#[derive(Component, Clone, Debug, Serialize, Deserialize)]
pub struct NameTag(pub String);

/// Declared references: `"$id:8"`, `"$ref:local"`, or `"@ref:global"`.
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Refs(pub HashMap<String, String>);

/// Resolved references: role -> `Entity`. Derived cache, **not serialized**.
#[derive(Component, Clone, Debug, Default)]
pub struct Bound(pub HashMap<String, Entity>);

/// Derived identity of one materialized prefab record. `instance` separates
/// repeated instances whose JSON record ids are otherwise identical.
#[derive(Component, Clone, Copy, Debug)]
pub struct PrefabEntityId {
    pub instance: Entity,
    pub local: u32,
}

/// Resolve every `Refs` into a `Bound`. Re-runnable: rebuild the global index
/// and re-resolve each reference by name. Unknown names are skipped.
pub fn resolve_refs(world: &mut World) {
    // 1. global index: NameTag -> Entity
    let mut global: HashMap<String, Entity> = HashMap::new();
    {
        let mut q = world.query::<(Entity, &NameTag)>();
        for (e, t) in q.iter(world) {
            global.insert(t.0.clone(), e);
        }
    }

    let mut local_ids: HashMap<(Entity, u32), Entity> = HashMap::new();
    {
        let mut q = world.query::<(Entity, &PrefabEntityId)>();
        for (entity, id) in q.iter(world) {
            local_ids.insert((id.instance, id.local), entity);
        }
    }

    // 2. snapshot the resolution jobs (entity, its parent, its refs)
    let jobs: Vec<(
        Entity,
        Option<Entity>,
        Option<PrefabEntityId>,
        HashMap<String, String>,
    )> = {
        let mut q = world.query::<(Entity, &Refs, Option<&ChildOf>, Option<&PrefabEntityId>)>();
        q.iter(world)
            .map(|(e, r, p, id)| (e, p.map(|c| c.parent()), id.copied(), r.0.clone()))
            .collect()
    };

    // 3. resolve each reference by scope
    for (e, parent, local_id, refs) in jobs {
        let mut bound = HashMap::new();
        for (role, label) in &refs {
            let target = if let Some(name) = label.strip_prefix("@ref:") {
                global.get(name).copied()
            } else if let Some(name) = label.strip_prefix("$ref:") {
                parent.and_then(|p| sibling_named(world, p, name))
            } else if let Some(id) = label.strip_prefix("$id:") {
                id.parse::<u32>().ok().and_then(|id| {
                    local_id.and_then(|local| local_ids.get(&(local.instance, id)).copied())
                })
            } else {
                None
            };
            if let Some(t) = target {
                bound.insert(role.clone(), t);
            }
        }
        world.entity_mut(e).insert(Bound(bound));
    }
}

/// Find a child of `parent` carrying `Local(name)`.
fn sibling_named(world: &World, parent: Entity, name: &str) -> Option<Entity> {
    let children = world.get::<Children>(parent)?;
    children
        .iter()
        .find(|&c| world.get::<Local>(c).is_some_and(|l| l.0 == name))
}
