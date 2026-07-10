use bevy_app::{App, Update};
use bevy_ecs::prelude::*;
use shared_api::{GameLogicSet, Velocity, PlayerControl, InputState};

#[no_mangle]
pub unsafe extern "C" fn setup_mod(app_ptr: *mut std::ffi::c_void) {
    let app = &mut *(app_ptr as *mut App);
    app.add_systems(
        Update,
        player_control_system.in_set(GameLogicSet::Update)
    );
}

fn player_control_system(input: Res<InputState>, mut query: Query<(&PlayerControl, &mut Velocity)>) {
    for (ctrl, mut vel) in query.iter_mut() {
        if input.left_down {
            vel.vec[0] = -ctrl.speed;
        } else if input.right_down {
            vel.vec[0] = ctrl.speed;
        } else {
            vel.vec[0] *= 0.8; // Apply friction when no keys are pressed
        }

        if input.space_pressed && vel.vec[1].abs() < 0.1 {
            vel.vec[1] = -ctrl.jump_force;
        }
    }
}
