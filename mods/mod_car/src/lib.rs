use bevy_app::{App, Update};
use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use shared_api::{CarBody, CarWheel, GameLogicSet, Transform, Velocity};

#[no_mangle]
pub unsafe extern "C" fn setup_mod(app_ptr: *mut std::ffi::c_void) {
    let app = &mut *(app_ptr as *mut App);
    app.add_systems(Update, sync_wheels_system.in_set(GameLogicSet::PostUpdate));
}

fn sync_wheels_system(
    links: Query<&ChildOf>,
    cars: Query<
        (&Transform, &Velocity),
        (With<CarBody>, Without<CarWheel>),
    >,
    mut wheels: Query<(&ChildOf, &CarWheel, &mut Transform), With<CarWheel>>,
) {
    for (link, wheel, mut transform) in wheels.iter_mut() {
        let mut ancestor = link.parent();
        let car = loop {
            if let Ok(car) = cars.get(ancestor) {
                break Some(car);
            }
            let Ok(parent_link) = links.get(ancestor) else {
                break None;
            };
            ancestor = parent_link.parent();
        };
        let Some((car_transform, velocity)) = car else {
            continue;
        };
        transform.position[0] = car_transform.position[0] + wheel.offset[0];
        transform.position[1] = car_transform.position[1] + wheel.offset[1];
        transform.rotation += velocity.vec[0] * wheel.spin_factor;
    }
}
