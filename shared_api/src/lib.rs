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

impl InputState {
    /// Horizontal player intent shared by every controllable avatar.
    pub fn move_axis(&self) -> f32 {
        match (self.left_down, self.right_down) {
            (true, false) => -1.0,
            (false, true) => 1.0,
            _ => 0.0,
        }
    }
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
    /// Maximum horizontal speed for whichever avatar owns this component.
    pub speed: f32,
    /// Vertical impulse requested by the standard action button.
    pub jump_force: f32,
}

#[derive(Component, Deserialize, Serialize, Clone)]
pub struct Collider {
    pub half_extents: [f32; 2],
}

/// Marker for the root of a visual car hierarchy. This is used only by the
/// car mod; physics uses the domain-neutral `Collider` component instead.
#[derive(Component, Deserialize, Serialize, Clone)]
pub struct CarBody {}

#[derive(Component, Deserialize, Serialize, Clone)]
pub struct CarWheel {
    pub offset: [f32; 2],
    pub spin_factor: f32,
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum GameLogicSet {
    Update,
    PostUpdate,
}

#[derive(SystemSet, Debug, Hash, PartialEq, Eq, Clone)]
pub enum RenderSet {
    Clear,
    DrawOpaque,
    DrawUI,
}

pub struct MainThreadMarker;

#[cfg(test)]
mod tests {
    use super::InputState;

    #[test]
    fn horizontal_input_is_normalized_for_all_avatars() {
        let mut input = InputState::default();
        assert_eq!(input.move_axis(), 0.0);

        input.left_down = true;
        assert_eq!(input.move_axis(), -1.0);

        input.right_down = true;
        assert_eq!(input.move_axis(), 0.0);

        input.left_down = false;
        assert_eq!(input.move_axis(), 1.0);
    }
}
