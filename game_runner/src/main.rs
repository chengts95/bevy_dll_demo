use bevy_app::App;
use bevy_dll_mod_api::{
    check_owned_type_id_probes, TypeIdProbeAbiVersionFn, TypeIdProbesFn,
    TYPE_ID_PROBE_ABI_VERSION, TYPE_ID_PROBE_ABI_VERSION_SYMBOL, TYPE_ID_PROBES_SYMBOL,
};
use ecs_prefab::Library;
use serde::Deserialize;
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

fn default_prefabs_dir() -> PathBuf {
    PathBuf::from("prefabs")
}
#[allow(dead_code)]
#[derive(Deserialize, Clone)]
struct ModPlatform {
    linux: Option<ModDllPath>,
    windows: Option<ModDllPath>,
}

#[derive(Deserialize, Clone)]
struct ModDllPath {
    dll_path_debug: String,
    dll_path_release: String,
}

fn main() {
    let mut app = App::new();

    let playset_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("playset.toml"));
    let playset_str = std::fs::read_to_string(&playset_path)
        .unwrap_or_else(|_| panic!("Failed to read {}", playset_path.display()));
    let playset: Playset = toml::from_str(&playset_str).expect("Failed to parse playset.toml");
    let playset_dir = playset_path.parent().unwrap_or_else(|| Path::new("."));

    let mut mod_prefabs = Library::new();
    let mut loaded_libs = Vec::new();

    for playset_mod in playset.mods {
        let descriptor = if playset_mod.descriptor.is_absolute() {
            playset_mod.descriptor
        } else {
            playset_dir.join(playset_mod.descriptor)
        };
        let manifest_str = std::fs::read_to_string(&descriptor)
            .unwrap_or_else(|_| panic!("Failed to read {}", descriptor.display()));
        let manifest: ModManifest = toml::from_str(&manifest_str)
            .unwrap_or_else(|_| panic!("Failed to parse {}", descriptor.display()));

        println!("Game Runner: Loading Mod '{}'", manifest.name);

        let mod_dir = descriptor.parent().unwrap_or_else(|| Path::new("."));
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

        #[cfg(target_os = "windows")]
        let platform_cfg = manifest
            .platform
            .windows
            .as_ref()
            .expect("Missing [platform.windows] in mod.toml");
        #[cfg(target_os = "linux")]
        let platform_cfg = manifest
            .platform
            .linux
            .as_ref()
            .expect("Missing [platform.linux] in mod.toml");

        let dll_name = if cfg!(debug_assertions) {
            &platform_cfg.dll_path_debug
        } else {
            &platform_cfg.dll_path_release
        };

        // Correct distribution path: relative to the directory containing mod.toml
        let dll_path = mod_dir.join(dll_name);

        match unsafe { libloading::Library::new(&dll_path) } {
            Ok(lib) => {
                if let Err(err) = check_mod_abi(&lib) {
                    eprintln!("Failed ABI check for {}: {}", dll_path.display(), err);
                    continue;
                }
                if let Ok(setup) =
                    unsafe { lib.get::<unsafe extern "C" fn(*mut std::ffi::c_void)>(b"setup_mod") }
                {
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
        if let Ok(load_case) =
            unsafe { lib.get::<unsafe extern "C" fn(*mut std::ffi::c_void) -> i32>(b"load_case") }
        {
            let code = unsafe { load_case(&mut app as *mut _ as *mut std::ffi::c_void) };
            if code != 0 {
                eprintln!("Game Loader failed with code {}", code);
            }
        }
    }

    app.run();
    std::mem::forget(loaded_libs);
}

fn check_mod_abi(lib: &libloading::Library) -> Result<(), Box<dyn std::error::Error>> {
    let version = unsafe {
        lib.get::<TypeIdProbeAbiVersionFn>(TYPE_ID_PROBE_ABI_VERSION_SYMBOL)?
    };
    let version = unsafe { version() };
    if version != TYPE_ID_PROBE_ABI_VERSION {
        return Err(format!(
            "unsupported TypeId probe ABI version: expected {}, got {}",
            TYPE_ID_PROBE_ABI_VERSION, version
        )
        .into());
    }

    let probes = unsafe { lib.get::<TypeIdProbesFn>(TYPE_ID_PROBES_SYMBOL)? };
    let probes = unsafe { probes().to_owned_vec()? };
    check_owned_type_id_probes(&bevy_dll_mod_api::bevy_type_id_probes(), &probes)?;
    Ok(())
}
