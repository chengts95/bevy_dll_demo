use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Resource)]
pub struct AppModPrefabs(pub ecs_prefab::Library);

#[derive(Component, Deserialize, Serialize, Clone)]
pub struct Transform {
    pub position: [f32; 3],
    pub size: [f32; 3],
    pub rotation: f32,
}

#[derive(Component, Deserialize, Serialize, Clone)]
pub struct Visual {
    pub color: [f32; 4],
}

#[derive(Component, Deserialize, Serialize, Clone)]
pub struct Spin {
    pub speed: f32,
}

#[derive(Resource, Default, Clone)]
pub struct InputState {
    pub left_down: bool,
    pub right_down: bool,
    pub space_pressed: bool,
}

#[derive(Component, Deserialize, Serialize, Clone)]
pub struct Velocity {
    pub vec: [f32; 2],
}

#[derive(Component, Deserialize, Serialize, Clone)]
pub struct Gravity {
    pub force: f32,
}

#[derive(Component, Deserialize, Serialize, Clone)]
pub struct PlayerControl {
    pub speed: f32,
    pub jump_force: f32,
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum GameLogicSet { Update }

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum RenderSet { Clear, DrawOpaque, DrawUI }

pub struct MainThreadMarker;
