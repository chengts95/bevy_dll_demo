use bevy_app::App;
use shared_api::{CarBody, CarWheel, Collider, Transform, Visual, Spin};
use ecs_prefab::{Library, PrefabRegistry};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct GameManifest {
    #[serde(default)]
    pub prefabs: HashMap<String, serde_json::Value>,
    pub instances: Vec<InstanceSpec>,
}

#[derive(Deserialize)]
pub struct InstanceSpec {
    pub id: String,
    pub prefab: String,
    #[serde(default)]
    pub params: serde_json::Map<String, serde_json::Value>,
}

#[no_mangle]
pub unsafe extern "C" fn setup_mod(_app_ptr: *mut std::ffi::c_void) {}

#[no_mangle]
pub unsafe extern "C" fn load_case(app_ptr: *mut std::ffi::c_void) -> i32 {
    let app = unsafe { &mut *(app_ptr as *mut App) };
    
    let game_path = std::env::var_os("BEVY_GAME_FILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("game.json"));
    let data = match std::fs::read_to_string(&game_path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Game Loader: Failed to read {}: {}", game_path.display(), e);
            return 1;
        }
    };

    let manifest: GameManifest = match serde_json::from_str(&data) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Game Loader: Failed to parse {}: {}", game_path.display(), e);
            return 2;
        }
    };

    let mut reg = PrefabRegistry::default();
    reg.register::<Transform>()
       .register::<Visual>()
       .register::<Spin>()
       .register::<shared_api::Velocity>()
       .register::<shared_api::Gravity>()
       .register::<shared_api::PlayerControl>()
       .register::<Collider>()
       .register::<CarBody>()
       .register::<CarWheel>();

    let mut lib = Library::new();

    // 1. Fetch Global Prefabs from ModRunner
    if let Some(mod_prefabs) = app.world_mut().remove_resource::<shared_api::AppModPrefabs>() {
        for (name, class) in mod_prefabs.0 {
            lib.insert(name, class);
        }
    }

    // 2. Merge Inline Prefabs from game.json
    for (name, value) in &manifest.prefabs {
        if let Ok(class) = serde_json::from_value(value.clone()) {
            lib.insert(name.clone(), class);
        } else {
            eprintln!("Game Loader: Failed to parse inline prefab '{}'", name);
        }
    }

    if let Err(e) = reg.load_library(&lib) {
        eprintln!("Game Loader: Failed to load library into registry: {}", e);
        return 3;
    }

    let world = app.world_mut();
    for instance in &manifest.instances {
        if let Err(e) = reg.try_spawn_class(
            &lib,
            &instance.prefab,
            &instance.params,
            &format!("{}.", instance.id),
            world
        ) {
            eprintln!("Game Loader: Failed to spawn instance '{}': {}", instance.id, e);
            return 4;
        }
    }

    println!("Game Loader: Successfully spawned {} instances via ecs_prefab!", manifest.instances.len());
    0
}
