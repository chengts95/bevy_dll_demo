use bevy_app::App;
use bevy_ecs::prelude::Resource;
use ecs_prefab::Library;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use shared_api::AppModPrefabs;

#[derive(Deserialize)]
struct Playset {
    mods: Vec<PlaysetMod>,
}

#[derive(Deserialize)]
struct PlaysetMod {
    descriptor: PathBuf,
}

#[derive(Deserialize, Clone)]
struct ModManifest {
    name: String,
    platform: ModPlatform,
    #[serde(default = "default_prefabs_dir")]
    prefabs_dir: PathBuf,
}

fn default_prefabs_dir() -> PathBuf { PathBuf::from("prefabs") }

#[derive(Deserialize, Clone)]
struct ModPlatform {
    linux: ModDllPath,
}

#[derive(Deserialize, Clone)]
struct ModDllPath {
    dll_path_debug: String,
    dll_path_release: String,
}

fn main() {
    let mut app = App::new();

    let playset_str = std::fs::read_to_string("playset.toml").expect("Failed to read playset.toml");
    let playset: Playset = toml::from_str(&playset_str).expect("Failed to parse playset.toml");

    let mut mod_prefabs = Library::new();
    let mut loaded_libs = Vec::new();

    for playset_mod in playset.mods {
        let manifest_str = std::fs::read_to_string(&playset_mod.descriptor).unwrap_or_else(|_| panic!("Failed to read {}", playset_mod.descriptor.display()));
        let manifest: ModManifest = toml::from_str(&manifest_str).unwrap_or_else(|_| panic!("Failed to parse {}", playset_mod.descriptor.display()));
        
        println!("Game Runner: Loading Mod '{}'", manifest.name);

        let mod_dir = playset_mod.descriptor.parent().unwrap_or_else(|| Path::new("."));
        let prefabs_dir = mod_dir.join(&manifest.prefabs_dir);

        // Load Global Prefabs from mod folder
        if prefabs_dir.exists() {
            for entry in std::fs::read_dir(prefabs_dir).unwrap() {
                let path = entry.unwrap().path();
                if path.extension().unwrap_or_default() == "json" {
                    let data = std::fs::read_to_string(&path).unwrap();
                    let value: serde_json::Value = serde_json::from_str(&data).unwrap();
                    let lib: Library = serde_json::from_value(value).unwrap();
                    for (k, v) in lib {
                        mod_prefabs.insert(k, v);
                    }
                }
            }
        }

        // Load DLL
        let target_dir = if cfg!(debug_assertions) { "debug" } else { "release" };
        let dll_name = if target_dir == "debug" { &manifest.platform.linux.dll_path_debug } else { &manifest.platform.linux.dll_path_release };
        
        // Correct distribution path: relative to the directory containing mod.toml
        let dll_path = mod_dir.join(dll_name);

        match unsafe { libloading::Library::new(&dll_path) } {
            Ok(lib) => {
                if let Ok(setup) = unsafe { lib.get::<unsafe extern "C" fn(*mut std::ffi::c_void)>(b"setup_mod") } {
                    unsafe { setup(&mut app as *mut _ as *mut std::ffi::c_void) };
                }
                loaded_libs.push(lib);
            }
            Err(e) => eprintln!("Failed to load {}: {}", dll_path.display(), e),
        }
    }

    // Insert AppModPrefabs
    app.insert_resource(AppModPrefabs(mod_prefabs));

    // Call load_case
    for lib in &loaded_libs {
        if let Ok(load_case) = unsafe { lib.get::<unsafe extern "C" fn(*mut std::ffi::c_void) -> i32>(b"load_case") } {
            let code = unsafe { load_case(&mut app as *mut _ as *mut std::ffi::c_void) };
            if code != 0 {
                eprintln!("Game Loader failed with code {}", code);
            }
        }
    }

    app.run();
    std::mem::forget(loaded_libs);
}
