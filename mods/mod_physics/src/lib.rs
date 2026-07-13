use bevy_app::{App, Update};
use bevy_ecs::prelude::*;
use shared_api::{Collider, GameLogicSet, Gravity, Transform, Velocity};

bevy_dll_mod_api::export_type_id_probes!();

#[no_mangle]
pub unsafe extern "C" fn setup_mod(app_ptr: *mut std::ffi::c_void) {
    let app = &mut *(app_ptr as *mut App);
    app.add_systems(
        Update,
        (gravity_system, movement_system, ground_collision_system)
            .chain()
            .in_set(GameLogicSet::Update),
    );
}

fn gravity_system(mut query: Query<(&Gravity, &mut Velocity)>) {
    for (grav, mut vel) in query.iter_mut() {
        vel.vec[1] += grav.force;
    }
}

fn movement_system(mut query: Query<(&Velocity, &mut Transform)>) {
    for (vel, mut transform) in query.iter_mut() {
        transform.position[0] += vel.vec[0];
        transform.position[1] += vel.vec[1];
    }
}

fn ground_collision_system(mut query: Query<(&mut Transform, &mut Velocity, Option<&Collider>)>) {
    // Hardcoded ground line at Y = 550
    let ground_y = 550.0;
    for (mut transform, mut vel, collider) in query.iter_mut() {
        let half_height =
            collider.map_or(transform.size[1] / 2.0, |collider| collider.half_extents[1]);
        let bottom_edge = transform.position[1] + half_height;
        if bottom_edge > ground_y {
            transform.position[1] = ground_y - half_height;
            vel.vec[1] *= -0.4; // Bounce and dampen
            if vel.vec[1].abs() < 1.0 {
                vel.vec[1] = 0.0;
            }
        }

        // Wall collision (hardcoded screen bounds 0 ~ 800)
        let left_edge = transform.position[0] - transform.size[0] / 2.0;
        let right_edge = transform.position[0] + transform.size[0] / 2.0;

        if left_edge < 0.0 {
            transform.position[0] = transform.size[0] / 2.0;
            vel.vec[0] *= -0.5;
        }
        if right_edge > 800.0 {
            transform.position[0] = 800.0 - transform.size[0] / 2.0;
            vel.vec[0] *= -0.5;
        }
    }
}
