//! The prefab on-disk schema — pure data + serde, **no logic**.
//!
//! A library file is a JSON map `name -> PrefabClass`. A class declares its
//! `params` (with defaults) plus a flat `body` of entity records. Composition
//! is the `Use` *component*: a body record carrying `Use` is a nested instance,
//! expanded after load.

use bevy_ecs::prelude::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── name layer (symbolic references, never entity ids) ──────────────────────

/// An entity's local name within a prefab. `"r1"`, `"src"`.
#[derive(Component, Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Local(pub String);

/// The net symbols this entity's terminals touch. Entities sharing a net name
/// are connected; connectivity is by name, never by entity id.
#[derive(Component, Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Net(pub Vec<String>);

/// Lowered terminals: net symbols resolved to integer node ids (load product).
#[derive(Component, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Pins(pub Vec<u32>);

// ── use = a component ───────────────────────────────────────────────────────

/// `use` is a **component**, not syntax: a body record carrying `Use` is a
/// nested prefab instance. `params` are the only child-input channel: ordinary
/// values and symbolic port values are passed uniformly through this map.
#[derive(Component, Clone, Serialize, Deserialize, Default, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Use {
    pub prefab: String,
    #[serde(default)]
    pub params: serde_json::Map<String, serde_json::Value>,
}

// ── flat record form ────────────────────────────────────────────────────────

/// One entity in a body: a stable local `id`, an optional `parent` (the
/// `ChildOf` edge as an index, rebuilt through a `u32 -> Entity` remap), and a
/// map of `type name -> serde Value` (values may carry `"$param"` symbols).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntityRecord {
    pub id: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<u32>,
    pub components: serde_json::Map<String, serde_json::Value>,
}

/// A flat list of entity records (a prefab body, or a flattened world).
pub type Snapshot = Vec<EntityRecord>;

// ── prefab class & library ──────────────────────────────────────────────────

/// A prefab class: declared params (with defaults) and a flat body. Hierarchy
/// *within* the body is `ChildOf` (`parent`);
/// composition *across* prefabs is the `Use` component.
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct PrefabClass {
    #[serde(default)]
    pub params: serde_json::Map<String, serde_json::Value>,
    pub body: Snapshot,
}

/// Name -> class. A whole library is one JSON object.
pub type Library = HashMap<String, PrefabClass>;
