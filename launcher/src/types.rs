#[derive(Clone)]
struct ModEntry {
    enabled: bool,
    name: String,
    title: String,
    version: String,
    author: String,
    category: String,
    tags: Vec<String>,
    origin: String,
    manifest_path: PathBuf,
    mod_dir: PathBuf,
    dll_path: Option<PathBuf>,
    prefabs_dir: PathBuf,
    templates_dir: PathBuf,
    components: Vec<String>,
    prefab_files: Vec<String>,
    library_entries: Vec<LibraryEntry>,
    dependencies: Vec<String>,
    description: String,
}

#[derive(Clone)]
struct LibraryEntry {
    mod_name: String,
    mod_title: String,
    id: String,
    kind: String,
    name: String,
    category: String,
    summary: String,
    source: String,
    uses: Vec<String>,
    parameters: Vec<LibraryField>,
    signals: Vec<LibraryField>,
    raw_preview: String,
}

#[derive(Clone)]
struct LibraryField {
    name: String,
    unit: String,
    description: String,
}

#[derive(serde::Deserialize)]
struct ModManifest {
    name: String,
    #[serde(default)]
    title: String,
    version: String,
    #[serde(default)]
    path: Option<PathBuf>,
    #[serde(default = "default_prefabs_dir")]
    prefabs_dir: PathBuf,
    #[serde(default = "default_templates_dir")]
    templates_dir: PathBuf,
    #[serde(default)]
    author: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    components: Vec<String>,
    #[serde(default)]
    description: String,
    #[serde(default)]
    dependencies: Vec<String>,
    platform: ModPlatform,
}

#[derive(serde::Deserialize)]
struct ModPlatform {
    #[cfg(target_os = "windows")]
    windows: ModDllPath,
    #[cfg(target_os = "linux")]
    linux: ModDllPath,
    #[cfg(target_os = "macos")]
    macos: ModDllPath,
}

#[derive(serde::Deserialize)]
struct ModDllPath {
    dll_path_debug: String,
    dll_path_release: String,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct PlaysetToml {
    name: String,
    mods: Vec<PlaysetModToml>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct PlaysetModToml {
    descriptor: String,
}

#[derive(serde::Deserialize, Default)]
struct LibraryToml {
    #[serde(default)]
    entries: Vec<LibraryEntryToml>,
}

#[derive(serde::Deserialize, Clone)]
struct LibraryEntryToml {
    id: String,
    kind: String,
    name: String,
    #[serde(default)]
    category: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    source: String,
    #[serde(default)]
    uses: Vec<String>,
    #[serde(default)]
    parameters: Vec<LibraryFieldToml>,
    #[serde(default)]
    signals: Vec<LibraryFieldToml>,
}

#[derive(serde::Deserialize, Clone)]
struct LibraryFieldToml {
    name: String,
    #[serde(default)]
    unit: String,
    #[serde(default)]
    description: String,
}

impl ModEntry {
    pub fn status(&self) -> &'static str {
        if self.enabled { "ON " } else { "OFF" }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct LauncherToml {
    #[serde(default = "default_start_in")]
    pub start_in: String,
    #[serde(default = "default_runner_cmd")]
    pub runner_cmd: String,
}

fn default_start_in() -> String {
    ".".to_string()
}

fn default_runner_cmd() -> String {
    "cargo run -p game_runner".to_string()
}

impl Default for LauncherToml {
    fn default() -> Self {
        Self {
            start_in: default_start_in(),
            runner_cmd: default_runner_cmd(),
        }
    }
}
