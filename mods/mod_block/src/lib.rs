use bevy_app::{App, Update};
use bevy_ecs::prelude::*;
use shared_api::{GameLogicSet, Spin, Transform};

bevy_dll_mod_api::export_type_id_probes!();

#[no_mangle]
pub unsafe extern "C" fn setup_mod(app_ptr: *mut std::ffi::c_void) {
    let app = &mut *(app_ptr as *mut App);
    app.add_systems(Update, spin_system.in_set(GameLogicSet::Update));
}

fn spin_system(mut query: Query<(&mut Transform, &Spin)>) {
    for (mut transform, spin) in query.iter_mut() {
        transform.rotation += spin.speed;
    }
}
