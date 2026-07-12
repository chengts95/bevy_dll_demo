fn build_home_page(tabs: &mut Tab, tab: u32) {
    let mut welcome =
        Panel::with_type("Welcome", layout!("l:1,t:1,w:65%,b:1"), panel::Type::Window);
    welcome.add(label!("'Bevy Engine',x:2,y:2,w:32,h:1"));
    welcome.add(label!(
        "'Dynamic mod launcher for runner and saves.',x:2,y:4,w:68,h:1"
    ));
    welcome.add(TextArea::new(
        "Distribution layout\n\n\
         Data/     product-shipped mods\n\
         Mods/     user mods\n\
         Saves/    user and sample saves\n\
         Runs/     output directories\n\n\
         Launcher flow\n\n\
         1. Scan mod.toml from Data and mods\n\
         2. Browse the component and prefab library\n\
         3. Enable mods and arrange load order\n\
         4. Select a save and build the dynamic runner command\n\
         5. Start bevy_dll_runner with the selected mod list",
        layout!("l:2,t:7,r:2,b:2"),
        textarea::Flags::ReadOnly,
    ));
    tabs.add(tab, welcome);

    let mut menu = Panel::with_type(
        "Main Menu",
        layout!("r:1,t:1,w:33%,b:1"),
        panel::Type::Raised,
    );
    menu.add(button!("'F2 Mods',x:3,y:3,w:28"));
    menu.add(button!("'F3 Library',x:3,y:6,w:28"));
    menu.add(button!("'F4 Saves',x:3,y:9,w:28"));
    menu.add(button!("'F5 Run',x:3,y:12,w:28"));
    menu.add(label!(
        "'Command bar shortcuts are active.',x:3,y:14,w:30,h:2"
    ));
    menu.add(button!("'Quit',x:3,y:21,w:28"));
    tabs.add(tab, menu);
}

fn build_library_page(tabs: &mut Tab, tab: u32) -> (Handle<ListBox>, Handle<TextArea>) {
    let mut browser = Panel::with_type(
        "Asset Library",
        layout!("l:1,t:1,w:55%,b:1"),
        panel::Type::Sunken,
    );
    browser.add(label!(
        "'Kind        Name                         Mod                  Category',x:1,y:1,w:62,h:1"
    ));
    let library = browser.add(ListBox::new(
        layout!("l:1,t:3,r:1,b:4"),
        listbox::Flags::ScrollBars
            | listbox::Flags::SearchBar
            | listbox::Flags::HighlightSelectedItemWhenInactive,
    ));
    browser.add(button!("'Back Home',l:1,b:1,w:14"));
    tabs.add(tab, browser);

    let mut info = Panel::with_type(
        "Entry Info",
        layout!("r:1,t:1,w:43%,b:1"),
        panel::Type::Sunken,
    );
    let details = info.add(TextArea::new(
        "",
        layout!("l:1,t:1,r:1,b:1"),
        textarea::Flags::ReadOnly,
    ));
    tabs.add(tab, info);
    (library, details)
}

fn build_mods_page(tabs: &mut Tab, tab: u32) -> (Handle<ListBox>, Handle<TextArea>) {
    let mut load_order = Panel::with_type(
        "Mod Load Order",
        layout!("l:1,t:1,w:62%,b:1"),
        panel::Type::Sunken,
    );
    load_order.add(label!(
        "'Slot        Mod                            Version   Origin   State',x:1,y:1,w:72,h:1"
    ));
    let mods = load_order.add(ListBox::new(
        layout!("l:1,t:3,r:1,b:4"),
        listbox::Flags::ScrollBars
            | listbox::Flags::SearchBar
            | listbox::Flags::CheckBoxes
            | listbox::Flags::HighlightSelectedItemWhenInactive,
    ));
    load_order.add(button!("'Toggle',l:1,b:1,w:8"));
    load_order.add(button!("'Up',l:10,b:1,w:7"));
    load_order.add(button!("'Down',l:18,b:1,w:7"));
    load_order.add(button!("'Check',l:26,b:1,w:8"));
    load_order.add(button!("'Run',l:35,b:1,w:7"));
    tabs.add(tab, load_order);

    let mut meta = Panel::with_type(
        "Mod Metadata",
        layout!("r:1,t:1,w:36%,b:1"),
        panel::Type::Sunken,
    );
    let details = meta.add(TextArea::new(
        "",
        layout!("l:1,t:1,r:1,b:4"),
        textarea::Flags::ReadOnly,
    ));
    meta.add(button!("'Back Home',l:1,b:1,w:14"));
    tabs.add(tab, meta);
    (mods, details)
}

fn build_saves_page(tabs: &mut Tab, tab: u32) -> (Handle<ListBox>, Handle<TextArea>) {
    let mut save_panel =
        Panel::with_type("Game Files", layout!("l:1,t:1,w:55%,b:1"), panel::Type::Sunken);
    let saves = save_panel.add(ListBox::new(
        layout!("l:1,t:1,r:1,b:4"),
        listbox::Flags::ScrollBars | listbox::Flags::HighlightSelectedItemWhenInactive,
    ));
    save_panel.add(button!("'Launch Game',l:1,b:1,w:14"));
    save_panel.add(button!("'Back Home',l:16,b:1,w:14"));
    tabs.add(tab, save_panel);

    let mut info = Panel::with_type(
        "Game Manifest",
        layout!("r:1,t:1,w:43%,b:1"),
        panel::Type::Sunken,
    );
    let details = info.add(TextArea::new(
        "",
        layout!("l:1,t:1,r:1,b:1"),
        textarea::Flags::ReadOnly,
    ));
    tabs.add(tab, info);
    (saves, details)
}

fn build_run_page(tabs: &mut Tab, tab: u32) -> Handle<TextArea> {
    let mut console = Panel::with_type(
        "Run Console",
        layout!("l:1,t:1,r:1,b:1"),
        panel::Type::Sunken,
    );
    let log = console.add(TextArea::new(
        "",
        layout!("l:1,t:1,r:1,b:4"),
        textarea::Flags::ReadOnly,
    ));
    console.add(button!("'Validate',l:1,b:1,w:12"));
    console.add(button!("'Launch Game',l:14,b:1,w:14"));
    console.add(button!("'Back Home',l:29,b:1,w:14"));
    tabs.add(tab, console);
    log
}

fn format_mod_row(index: usize, mod_entry: &ModEntry) -> String {
    format!(
        "{:02}          {:<30} {:<8} {:<7} {}",
        index + 1,
        mod_entry.title,
        mod_entry.version,
        mod_entry.origin,
        mod_entry.status()
    )
}

fn format_mod_details(index: usize, mod_entry: &ModEntry) -> String {
    format!(
        "{}\n{}\n\nLoad order: {}\nStatus: {}\nVersion: {}\nAuthor: {}\nCategory: {}\nTags: {}\nOrigin: {}\nManifest: {}\nMod dir: {}\nDLL: {}\nPrefabs: {}\nTemplates: {}\n\nComponents:\n{}\n\nPrefab files:\n{}\n\nDependencies:\n{}\n\n{}",
        mod_entry.title,
        mod_entry.name,
        index + 1,
        mod_entry.status(),
        mod_entry.version,
        display_or_none(&mod_entry.author),
        display_or_none(&mod_entry.category),
        format_inline_list(&mod_entry.tags),
        mod_entry.origin,
        mod_entry.manifest_path.display(),
        mod_entry.mod_dir.display(),
        mod_entry
            .dll_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none".to_string()),
        mod_entry.mod_dir.join(&mod_entry.prefabs_dir).display(),
        mod_entry.mod_dir.join(&mod_entry.templates_dir).display(),
        format_string_lines(&mod_entry.components),
        format_string_lines(&mod_entry.prefab_files),
        format_string_lines(&mod_entry.dependencies),
        display_or_none(&mod_entry.description),
    )
}

fn format_library_row(_index: usize, entry: &LibraryEntry) -> String {
    format!(
        "{:<11} {:<28} {:<20} {}",
        entry.kind,
        entry.name,
        entry.mod_title,
        display_or_none(&entry.category),
    )
}

fn format_library_details(entry: &LibraryEntry) -> String {
    format!(
        "{}\n{}\n\nKind: {}\nMod: {} ({})\nCategory: {}\nSource: {}\n\n{}\n\nUses:\n{}\n\nParameters:\n{}\n\nSignals:\n{}\n\nRaw preview:\n{}",
        entry.name,
        entry.id,
        entry.kind,
        entry.mod_title,
        entry.mod_name,
        display_or_none(&entry.category),
        display_or_none(&entry.source),
        display_or_none(&entry.summary),
        format_string_lines(&entry.uses),
        format_library_fields(&entry.parameters),
        format_library_fields(&entry.signals),
        display_or_none(&entry.raw_preview),
    )
}

fn format_mod_load_list(mods: &[ModEntry]) -> String {
    if mods.is_empty() {
        "  none".to_string()
    } else {
        mods.iter()
            .enumerate()
            .map(|(index, mod_entry)| {
                format!(
                    "  {:02}. {} {} ({})",
                    index + 1,
                    mod_entry.name,
                    mod_entry.version,
                    mod_entry.origin
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn format_string_lines(values: &[String]) -> String {
    if values.is_empty() {
        "  none".to_string()
    } else {
        values
            .iter()
            .map(|value| format!("  - {value}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn format_library_fields(values: &[LibraryField]) -> String {
    if values.is_empty() {
        "  none".to_string()
    } else {
        values
            .iter()
            .map(|value| {
                let unit = if value.unit.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", value.unit)
                };
                format!(
                    "  - {}{}: {}",
                    value.name,
                    unit,
                    display_or_none(&value.description)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn format_inline_list(values: &[String]) -> String {
    if values.is_empty() {
        "none".to_string()
    } else {
        values.join(", ")
    }
}

fn display_or_none(value: &str) -> &str {
    if value.is_empty() { "none" } else { value }
}

fn format_save_details(root: &Path, save: &str) -> String {
    let path = root.join(save);
    let meta = if path.exists() {
        "Game manifest verified and ready to load."
    } else {
        "Game manifest could not be found."
    };
    
    format!(
        "{}\n\n{}\n\nF5 runs the game with this save and the enabled mod load order.",
        save,
        meta
    )
}

fn turbo_theme() -> Theme {
    let mut theme = Theme::new(Themes::DarkGray);
    let blue = CharAttribute::new(Color::Silver, Color::DarkBlue, CharFlags::None);
    let blue_focus = CharAttribute::new(Color::White, Color::Blue, CharFlags::None);
    let cyan = CharAttribute::new(Color::Aqua, Color::DarkBlue, CharFlags::None);
    let black_edit = CharAttribute::new(Color::Silver, Color::Black, CharFlags::None);

    theme.desktop.character = Character::with_attributes(' ', blue);
    theme.window.normal = blue;
    theme.window.info = blue;
    theme.window.bar.focus = blue_focus;
    theme.window.bar.normal = CharAttribute::new(Color::White, Color::DarkBlue, CharFlags::None);
    theme.border.normal = cyan;
    theme.border.focused = CharAttribute::new(Color::White, Color::DarkBlue, CharFlags::None);
    theme.lines.normal = cyan;
    theme.editor.normal = black_edit;
    theme.editor.focused = CharAttribute::new(Color::White, Color::Black, CharFlags::None);
    theme.list_current_item.focus = CharAttribute::new(Color::Black, Color::Aqua, CharFlags::None);
    theme.list_current_item.over_inactive =
        CharAttribute::new(Color::Black, Color::Silver, CharFlags::None);
    theme.searchbar.normal = CharAttribute::new(Color::Silver, Color::Black, CharFlags::None);
    theme.searchbar.focused = CharAttribute::new(Color::White, Color::Black, CharFlags::None);
    theme
}
