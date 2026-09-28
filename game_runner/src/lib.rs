use bevy_app::{App, AppExit};
use bevy_dll_mod_api::{
    TYPE_ID_PROBE_ABI_VERSION, TYPE_ID_PROBE_ABI_VERSION_SYMBOL, TYPE_ID_PROBES_SYMBOL,
    TypeIdProbeAbiVersionFn, TypeIdProbeError, TypeIdProbesFn, check_owned_type_id_probes,
};
use ecs_prefab::Library;
use serde::Deserialize;
use std::path::{Path, PathBuf};

use shared_api::AppModPrefabs;

pub type RunnerResult<T> = Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, Clone)]
pub struct RunnerOptions {
    pub playset_path: PathBuf,
}

impl RunnerOptions {
    pub fn new(playset_path: impl Into<PathBuf>) -> Self {
        Self {
            playset_path: playset_path.into(),
        }
    }
}

impl Default for RunnerOptions {
    fn default() -> Self {
        Self::new("playset.toml")
    }
}

pub struct GameRunner {
    app: App,
    loaded_libs: Vec<libloading::Library>,
}

#[derive(Clone, Debug)]
pub struct RunnerModResource {
    pub name: String,
    pub mod_dir: PathBuf,
    pub prefabs_dir: PathBuf,
    pub templates_dir: PathBuf,
}

pub trait RunnerHooks {
    fn build_app(&mut self) -> App {
        App::new()
    }

    fn insert_mod_resources(
        &mut self,
        app: &mut App,
        prefabs: Library,
        _mods: Vec<RunnerModResource>,
    ) -> RunnerResult<()> {
        app.insert_resource(AppModPrefabs(prefabs));
        Ok(())
    }
}

#[derive(Default)]
pub struct DemoRunnerHooks;

impl RunnerHooks for DemoRunnerHooks {}

impl GameRunner {
    pub fn from_playset(playset_path: impl Into<PathBuf>) -> RunnerResult<Self> {
        Self::from_options(RunnerOptions::new(playset_path))
    }

    pub fn from_options(options: RunnerOptions) -> RunnerResult<Self> {
        Self::from_options_with_hooks(options, &mut DemoRunnerHooks)
    }

    pub fn from_options_with_hooks(
        options: RunnerOptions,
        hooks: &mut impl RunnerHooks,
    ) -> RunnerResult<Self> {
        let mut app = hooks.build_app();
        let mut loaded_libs = Vec::new();

        if let Err(error) = load_playset(&mut app, &mut loaded_libs, hooks, &options.playset_path) {
            // ECS component destructors can live in the loaded DLLs.
            drop(app);
            drop(loaded_libs);
            return Err(error);
        }

        Ok(Self { app, loaded_libs })
    }

    pub fn app(&self) -> &App {
        &self.app
    }

    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    pub fn run(mut self) -> AppExit {
        let exit = self.app.run();
        std::mem::forget(self.loaded_libs);
        exit
    }
}

pub fn run_playset(playset_path: impl Into<PathBuf>) -> RunnerResult<AppExit> {
    Ok(GameRunner::from_playset(playset_path)?.run())
}

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
    #[serde(default = "default_templates_dir")]
    templates_dir: PathBuf,
}

fn default_prefabs_dir() -> PathBuf {
    PathBuf::from("prefabs")
}

fn default_templates_dir() -> PathBuf {
    PathBuf::from("templates")
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

fn load_playset(
    app: &mut App,
    loaded_libs: &mut Vec<libloading::Library>,
    hooks: &mut impl RunnerHooks,
    playset_path: &Path,
) -> RunnerResult<()> {
    let playset_str = std::fs::read_to_string(playset_path)
        .map_err(|err| format!("Failed to read {}: {err}", playset_path.display()))?;
    let playset: Playset = toml::from_str(&playset_str)
        .map_err(|err| format!("Failed to parse {}: {err}", playset_path.display()))?;
    let playset_dir = playset_path.parent().unwrap_or_else(|| Path::new("."));

    let mut mod_prefabs = Library::new();
    let mut mod_resources = Vec::new();

    for playset_mod in playset.mods {
        load_mod(
            app,
            loaded_libs,
            &mut mod_prefabs,
            &mut mod_resources,
            playset_dir,
            playset_mod,
        )?;
    }

    hooks.insert_mod_resources(app, mod_prefabs, mod_resources)?;
    load_cases(app, loaded_libs)?;

    Ok(())
}

fn load_mod(
    app: &mut App,
    loaded_libs: &mut Vec<libloading::Library>,
    mod_prefabs: &mut Library,
    mod_resources: &mut Vec<RunnerModResource>,
    playset_dir: &Path,
    playset_mod: PlaysetMod,
) -> RunnerResult<()> {
    let descriptor = if playset_mod.descriptor.is_absolute() {
        playset_mod.descriptor
    } else {
        playset_dir.join(playset_mod.descriptor)
    };
    let manifest_str = std::fs::read_to_string(&descriptor)
        .map_err(|err| format!("Failed to read {}: {err}", descriptor.display()))?;
    let manifest: ModManifest = toml::from_str(&manifest_str)
        .map_err(|err| format!("Failed to parse {}: {err}", descriptor.display()))?;

    println!("Game Runner: Loading Mod '{}'", manifest.name);

    let mod_dir = descriptor.parent().unwrap_or_else(|| Path::new("."));
    load_mod_prefabs(mod_prefabs, mod_dir, &manifest)?;
    mod_resources.push(RunnerModResource {
        name: manifest.name.clone(),
        mod_dir: mod_dir.to_path_buf(),
        prefabs_dir: manifest.prefabs_dir.clone(),
        templates_dir: manifest.templates_dir.clone(),
    });
    let dll_path = resolve_mod_dll(mod_dir, &manifest)?;

    let lib = unsafe { libloading::Library::new(&dll_path) }
        .map_err(|err| format!("Failed to load {}: {}", dll_path.display(), err))?;
    check_mod_abi(&lib).map_err(|err| {
        format!(
            "Failed ABI check for {} selected from {}: {}",
            dll_path.display(),
            descriptor.display(),
            err
        )
    })?;

    if let Ok(setup) =
        unsafe { lib.get::<unsafe extern "C" fn(*mut std::ffi::c_void)>(b"setup_mod") }
    {
        unsafe { setup(app as *mut _ as *mut std::ffi::c_void) };
    }
    loaded_libs.push(lib);

    Ok(())
}

fn load_mod_prefabs(
    mod_prefabs: &mut Library,
    mod_dir: &Path,
    manifest: &ModManifest,
) -> RunnerResult<()> {
    let prefabs_dir = mod_dir.join(&manifest.prefabs_dir);
    if !prefabs_dir.exists() {
        return Ok(());
    }

    for entry in std::fs::read_dir(&prefabs_dir)
        .map_err(|err| format!("Failed to read prefab dir {}: {err}", prefabs_dir.display()))?
    {
        let path = entry?.path();
        let ext = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        let lib = match ext {
            "json" => match read_json_prefab_library(&path)? {
                Some(lib) => lib,
                None => continue,
            },
            "msgpack" => {
                let data = std::fs::read(&path)
                    .map_err(|err| format!("Failed to read prefab {}: {err}", path.display()))?;
                rmp_serde::from_slice::<Library>(&data)
                    .map_err(|err| format!("Failed to decode prefab {}: {err}", path.display()))?
            }
            _ => continue,
        };

        for (key, value) in lib {
            mod_prefabs.insert(key, value);
        }
    }

    Ok(())
}

fn read_json_prefab_library(path: &Path) -> RunnerResult<Option<Library>> {
    let data = std::fs::read_to_string(path)
        .map_err(|err| format!("Failed to read prefab {}: {err}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&data)
        .map_err(|err| format!("Failed to parse prefab {}: {err}", path.display()))?;
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    if !object.values().all(|entry| {
        entry
            .as_object()
            .is_some_and(|class| class.contains_key("body"))
    }) {
        return Ok(None);
    }

    Ok(Some(serde_json::from_value(value).map_err(|err| {
        format!("Failed to decode prefab {}: {err}", path.display())
    })?))
}

fn resolve_mod_dll(mod_dir: &Path, manifest: &ModManifest) -> RunnerResult<PathBuf> {
    #[cfg(target_os = "windows")]
    let platform_cfg = manifest
        .platform
        .windows
        .as_ref()
        .ok_or("Missing [platform.windows] in mod.toml")?;
    #[cfg(target_os = "linux")]
    let platform_cfg = manifest
        .platform
        .linux
        .as_ref()
        .ok_or("Missing [platform.linux] in mod.toml")?;

    let dll_name = if cfg!(debug_assertions) {
        &platform_cfg.dll_path_debug
    } else {
        &platform_cfg.dll_path_release
    };

    Ok(mod_dir.join(dll_name))
}

fn load_cases(app: &mut App, loaded_libs: &[libloading::Library]) -> RunnerResult<()> {
    for lib in loaded_libs {
        if let Ok(load_case) =
            unsafe { lib.get::<unsafe extern "C" fn(*mut std::ffi::c_void) -> i32>(b"load_case") }
        {
            let code = unsafe { load_case(app as *mut _ as *mut std::ffi::c_void) };
            if code != 0 {
                return Err(format!("Game Loader failed with code {code}").into());
            }
        }
    }

    Ok(())
}

fn check_mod_abi(lib: &libloading::Library) -> RunnerResult<()> {
    let version = unsafe { lib.get::<TypeIdProbeAbiVersionFn>(TYPE_ID_PROBE_ABI_VERSION_SYMBOL)? };
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
    check_owned_type_id_probes(&bevy_dll_mod_api::bevy_type_id_probes(), &probes).map_err(
        |err| match err {
            TypeIdProbeError::Mismatch {
                name: "__META_IS_DEBUG_BUILD__",
                expected_hash,
                actual_hash,
                ..
            } => format!(
                "DLL build profile mismatch: runner is {}, DLL is {}",
                debug_profile_name(expected_hash),
                debug_profile_name(actual_hash)
            )
            .into(),
            err => Box::<dyn std::error::Error>::from(err),
        },
    )?;
    Ok(())
}

fn debug_profile_name(value: u64) -> &'static str {
    if value == 1 { "debug" } else { "release" }
}
