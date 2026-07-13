use bevy_app::{App, Update};
use bevy_ecs::prelude::*;
use shared_api::{GameLogicSet, InputState, PlayerControl, Velocity};

bevy_dll_mod_api::export_type_id_probes!();

#[no_mangle]
pub unsafe extern "C" fn setup_mod(app_ptr: *mut std::ffi::c_void) {
    let app = &mut *(app_ptr as *mut App);
    app.add_systems(Update, player_control_system.in_set(GameLogicSet::Update));
}

fn player_control_system(
    input: Res<InputState>,
    mut query: Query<(&PlayerControl, &mut Velocity)>,
) {
    for (ctrl, mut vel) in query.iter_mut() {
        let move_axis = input.move_axis();
        if move_axis == 0.0 {
            vel.vec[0] *= 0.8;
        } else {
            vel.vec[0] = move_axis * ctrl.speed;
        }

        if input.space_pressed && vel.vec[1].abs() < 0.1 {
            vel.vec[1] = -ctrl.jump_force;
        }
    }
}
