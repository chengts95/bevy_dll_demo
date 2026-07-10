fn default_prefabs_dir() -> PathBuf {
    PathBuf::from("prefabs")
}

fn default_templates_dir() -> PathBuf {
    PathBuf::from("templates")
}

fn scan_mods(root: &Path) -> Result<Vec<ModEntry>, Box<dyn std::error::Error>> {
    let mut mods = Vec::new();
    let mut scanned_roots = Vec::new();
    scan_mod_root(root, "libs", &mut scanned_roots, &mut mods)?;
    scan_mod_root(root, "Data", &mut scanned_roots, &mut mods)?;
    scan_mod_root(root, "mods", &mut scanned_roots, &mut mods)?;
    scan_mod_root(root, "Mods", &mut scanned_roots, &mut mods)?;
    apply_saved_playset(root, &mut mods)?;
    Ok(mods)
}

fn apply_saved_playset(
    root: &Path,
    mods: &mut Vec<ModEntry>,
) -> Result<(), Box<dyn std::error::Error>> {
    let playset_path = root.join("playset.toml");
    if !playset_path.exists() {
        for mod_entry in mods.iter_mut() {
            mod_entry.enabled = mod_entry
                .dll_path
                .as_ref()
                .map(|path| path.exists())
                .unwrap_or(false);
        }
        return Ok(());
    }

    let data = fs::read_to_string(&playset_path)?;
    let playset: PlaysetToml = toml::from_str(&data)
        .map_err(|err| format!("failed to parse {}: {err}", playset_path.display()))?;
    let playset_dir = playset_path.parent().unwrap_or(root);
    let mut ordered = Vec::new();
    let mut used = vec![false; mods.len()];

    for playset_mod in playset.mods {
        let descriptor = resolve_path(playset_dir, Path::new(&playset_mod.descriptor));
        let descriptor = descriptor.canonicalize().unwrap_or(descriptor);
        if let Some((index, mod_entry)) = mods.iter().enumerate().find(|(_, mod_entry)| {
            mod_entry
                .manifest_path
                .canonicalize()
                .unwrap_or_else(|_| mod_entry.manifest_path.clone())
                == descriptor
        }) {
            let mut mod_entry = mod_entry.clone();
            mod_entry.enabled = true;
            used[index] = true;
            ordered.push(mod_entry);
        }
    }

    for (index, mod_entry) in mods.iter().enumerate() {
        if !used[index] {
            let mut mod_entry = mod_entry.clone();
            mod_entry.enabled = false;
            ordered.push(mod_entry);
        }
    }

    *mods = ordered;
    Ok(())
}

fn scan_mod_root(
    root: &Path,
    dir_name: &str,
    scanned_roots: &mut Vec<PathBuf>,
    mods: &mut Vec<ModEntry>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mods_dir = root.join(dir_name);
    if !mods_dir.is_dir() {
        return Ok(());
    }
    let canonical_mods_dir = mods_dir.canonicalize()?;
    if scanned_roots
        .iter()
        .any(|scanned| scanned == &canonical_mods_dir)
    {
        return Ok(());
    }
    scanned_roots.push(canonical_mods_dir);

    for entry in fs::read_dir(&mods_dir)? {
        let entry = entry?;
        let mod_dir = entry.path();
        if !mod_dir.is_dir() {
            continue;
        }

        let manifest_path = mod_dir.join("mod.toml");
        if !manifest_path.exists() {
            continue;
        }

        let data = fs::read_to_string(&manifest_path)?;
        let manifest: ModManifest = toml::from_str(&data)
            .map_err(|err| format!("failed to parse {}: {err}", manifest_path.display()))?;
        let actual_mod_dir = manifest
            .path
            .as_deref()
            .map(|path| resolve_path(&mod_dir, path))
            .unwrap_or_else(|| mod_dir.clone());
        let dll_path = resolve_manifest_dll(&manifest, &actual_mod_dir);
        let prefab_files = scan_prefab_files(&actual_mod_dir.join(&manifest.prefabs_dir));
        let title = if manifest.title.is_empty() {
            manifest.name.clone()
        } else {
            manifest.title
        };
        let library_entries = scan_library_entries(
            &manifest.name,
            &title,
            &manifest.category,
            &actual_mod_dir,
            &manifest.prefabs_dir,
            &prefab_files,
        )?;
        mods.push(ModEntry {
            enabled: true,
            name: manifest.name,
            title,
            version: manifest.version,
            author: manifest.author,
            category: manifest.category,
            tags: manifest.tags,
            origin: dir_name.to_string(),
            manifest_path,
            mod_dir: actual_mod_dir,
            dll_path,
            prefabs_dir: manifest.prefabs_dir,
            templates_dir: manifest.templates_dir,
            components: manifest.components,
            prefab_files,
            library_entries,
            dependencies: manifest.dependencies,
            description: manifest.description,
        });
    }

    Ok(())
}

fn scan_library_entries(
    mod_name: &str,
    mod_title: &str,
    mod_category: &str,
    mod_dir: &Path,
    prefabs_dir: &Path,
    prefab_files: &[String],
) -> Result<Vec<LibraryEntry>, Box<dyn std::error::Error>> {
    let mut entries = Vec::new();
    let library_path = mod_dir.join("library.toml");
    if library_path.exists() {
        let data = fs::read_to_string(&library_path)?;
        let library: LibraryToml = toml::from_str(&data)
            .map_err(|err| format!("failed to parse {}: {err}", library_path.display()))?;
        for entry in library.entries {
            entries.push(library_entry_from_toml(
                mod_name,
                mod_title,
                mod_category,
                mod_dir,
                entry,
            ));
        }
    }

    for prefab_file in prefab_files {
        let source = prefabs_dir.join(prefab_file).display().to_string();
        let covered = entries.iter().any(|entry| {
            entry.source == source
                || Path::new(&entry.source)
                    .file_name()
                    .and_then(|name| name.to_str())
                    == Some(prefab_file.as_str())
        });
        if covered {
            continue;
        }
        entries.push(auto_prefab_entry(
            mod_name,
            mod_title,
            mod_category,
            mod_dir,
            prefabs_dir,
            prefab_file,
        ));
    }

    entries.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(a.category.cmp(&b.category))
            .then(a.name.cmp(&b.name))
    });
    Ok(entries)
}

fn library_entry_from_toml(
    mod_name: &str,
    mod_title: &str,
    mod_category: &str,
    mod_dir: &Path,
    entry: LibraryEntryToml,
) -> LibraryEntry {
    let raw_preview = if entry.source.is_empty() {
        String::new()
    } else {
        read_source_preview(&mod_dir.join(&entry.source))
    };

    LibraryEntry {
        mod_name: mod_name.to_string(),
        mod_title: mod_title.to_string(),
        id: entry.id,
        kind: entry.kind,
        name: entry.name,
        category: if entry.category.is_empty() {
            mod_category.to_string()
        } else {
            entry.category
        },
        summary: entry.summary,
        source: entry.source,
        uses: entry.uses,
        parameters: entry
            .parameters
            .into_iter()
            .map(|field| LibraryField {
                name: field.name,
                unit: field.unit,
                description: field.description,
            })
            .collect(),
        signals: entry
            .signals
            .into_iter()
            .map(|field| LibraryField {
                name: field.name,
                unit: field.unit,
                description: field.description,
            })
            .collect(),
        raw_preview,
    }
}

fn auto_prefab_entry(
    mod_name: &str,
    mod_title: &str,
    mod_category: &str,
    mod_dir: &Path,
    prefabs_dir: &Path,
    prefab_file: &str,
) -> LibraryEntry {
    let source_path = prefabs_dir.join(prefab_file);
    let source = source_path.display().to_string();
    let stem = Path::new(prefab_file)
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(prefab_file);
    let id = format!("{mod_name}.{stem}");
    LibraryEntry {
        mod_name: mod_name.to_string(),
        mod_title: mod_title.to_string(),
        id,
        kind: "prefab".to_string(),
        name: title_from_id(stem),
        category: if mod_category.is_empty() {
            "Prefab".to_string()
        } else {
            mod_category.to_string()
        },
        summary: "Auto-discovered prefab library. Add library.toml metadata to describe usage, parameters, signals, and model references.".to_string(),
        source,
        uses: Vec::new(),
        parameters: Vec::new(),
        signals: Vec::new(),
        raw_preview: read_source_preview(&mod_dir.join(&source_path)),
    }
}

fn build_library_index(mods: &[ModEntry]) -> Vec<LibraryEntry> {
    let mut entries = Vec::new();
    for mod_entry in mods {
        entries.extend(mod_entry.library_entries.clone());
    }
    entries.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(a.category.cmp(&b.category))
            .then(a.mod_title.cmp(&b.mod_title))
            .then(a.name.cmp(&b.name))
    });
    entries
}

fn read_source_preview(path: &Path) -> String {
    let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
        return String::new();
    };
    if ext != "json" {
        return format!("{} preview is not available in the terminal library.", ext);
    }
    let Ok(data) = fs::read_to_string(path) else {
        return "source file could not be read".to_string();
    };
    truncate_preview(&data, 2200)
}

fn truncate_preview(value: &str, max_chars: usize) -> String {
    let mut output = String::new();
    for ch in value.chars().take(max_chars) {
        output.push(ch);
    }
    if value.chars().count() > max_chars {
        output.push_str("\n... truncated ...");
    }
    output
}

fn title_from_id(value: &str) -> String {
    value
        .split(['_', '-', '.'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn scan_prefab_files(prefabs_dir: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(prefabs_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
                continue;
            };
            if !matches!(ext, "json" | "msgpack" | "bin") {
                continue;
            }
            if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
                files.push(name.to_string());
            }
        }
    }
    files.sort();
    files
}

fn resolve_manifest_dll(manifest: &ModManifest, mod_dir: &Path) -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let platform = &manifest.platform.windows;
    #[cfg(target_os = "linux")]
    let platform = &manifest.platform.linux;
    #[cfg(target_os = "macos")]
    let platform = &manifest.platform.macos;

    let rel_path = if cfg!(debug_assertions) {
        &platform.dll_path_debug
    } else {
        &platform.dll_path_release
    };
    Some(mod_dir.join(rel_path))
}

fn resolve_path(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

fn scan_saves(root: &Path) -> Vec<String> {
    let mut saves = Vec::new();
    for dir_name in ["Saves", "saves"] {
        let save_dir = root.join(dir_name);
        if let Ok(entries) = fs::read_dir(&save_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|value| value.to_str()) == Some("json")
                    && path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .map(|name| name.ends_with(".save.json"))
                        .unwrap_or(false)
                {
                    let display = path
                        .strip_prefix(root)
                        .unwrap_or(&path)
                        .display()
                        .to_string();
                    saves.push(display);
                }
            }
        }
    }
    saves.sort();
    saves.dedup();
    if saves.is_empty() {
        saves.push("game.json".to_string());
    }
    saves
}

fn descriptor_for_playset(descriptor: &Path, playset_path: &Path, root: &Path) -> String {
    let playset_dir = playset_path.parent().unwrap_or_else(|| Path::new("."));
    if let Ok(root_relative) = descriptor.strip_prefix(root) {
        if playset_dir == root {
            root_relative.display().to_string()
        } else {
            Path::new("..").join(root_relative).display().to_string()
        }
    } else if descriptor.is_absolute() {
        descriptor.display().to_string()
    } else {
        descriptor.display().to_string()
    }
}

fn find_runner_exe(root: &Path) -> PathBuf {
    let exe_name = if cfg!(target_os = "windows") {
        "bevy_dll_runner.exe"
    } else {
        "bevy_dll_runner"
    };
    if let Ok(current) = std::env::current_exe() {
        if let Some(dir) = current.parent() {
            let sibling = dir.join(exe_name);
            if sibling.exists() {
                return sibling;
            }
        }
    }
    root.join("target").join("debug").join(exe_name)
}

fn app_root() -> PathBuf {
    if let Some(arg_root) = std::env::args().nth(1) {
        let path = PathBuf::from(arg_root);
        if path.is_dir() {
            return path;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

