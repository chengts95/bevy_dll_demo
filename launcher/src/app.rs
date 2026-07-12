#[Window(
    events = ButtonEvents + ListBoxEvents + TabEvents + CommandBarEvents,
    commands: Home+ManageMods+BrowseLibrary+SelectSave+RunSave+Toggle+MoveUp+MoveDown+Validate+Quit
)]
struct LauncherWindow {
    root: PathBuf,
    launcher_config: LauncherToml,
    tabs: Handle<Tab>,
    mods: Handle<ListBox>,
    library: Handle<ListBox>,
    library_details: Handle<TextArea>,
    saves: Handle<ListBox>,
    save_details: Handle<TextArea>,
    details: Handle<TextArea>,
    run_log: Handle<TextArea>,
    status: Handle<Label>,
    mods_data: Vec<ModEntry>,
    library_entries: Vec<LibraryEntry>,
}

impl LauncherWindow {
    fn new() -> Self {
        let root = app_root();
        let (mods_data, startup_message) = match scan_mods(&root) {
            Ok(mods) if mods.is_empty() => (
                mods,
                "No mod.toml files found under Data/ or mods/.".to_string(),
            ),
            Ok(mods) => {
                let count = mods.len();
                (
                    mods,
                    format!(
                        "Loaded {count} real mod.toml file(s). F2 edits the enabled load list."
                    ),
                )
            }
            Err(err) => (Vec::new(), format!("Failed to scan mod.toml files: {err}")),
        };

        let launcher_config = match fs::read_to_string(app_root().join("launcher.toml")) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => LauncherToml::default(),
        };

        let mut win = Self {
            base: window!("' Bevy Engine Launcher ',l:0,t:0,r:0,b:0"),
            root,
            launcher_config,
            tabs: Handle::None,
            mods: Handle::None,
            library: Handle::None,
            library_details: Handle::None,
            saves: Handle::None,
            save_details: Handle::None,
            details: Handle::None,
            run_log: Handle::None,
            status: Handle::None,
            mods_data,
            library_entries: Vec::new(),
        };

        win.add(label!(
            "'BEVY DLL ENGINE  |  mod launcher',l:2,t:1,r:28,h:1"
        ));
        let status = win.add(label!("'',r:2,t:1,w:25,h:1"));

        let mut tabs = Tab::new(layout!("l:1,t:3,r:1,b:3"), tab::Flags::TabsBar);
        let home_tab = tabs.add_tab("Home");
        let mods_tab = tabs.add_tab("Mods");
        let library_tab = tabs.add_tab("Library");
        let saves_tab = tabs.add_tab("Saves");
        let run_tab = tabs.add_tab("Run");

        build_home_page(&mut tabs, home_tab);
        let (mods, details) = build_mods_page(&mut tabs, mods_tab);
        let (library, library_details) = build_library_page(&mut tabs, library_tab);
        let (saves, save_details) = build_saves_page(&mut tabs, saves_tab);
        let run_log = build_run_page(&mut tabs, run_tab);

        // Map desktop navigation buttons
        win.add(button!("'F2 Mods',l:1,b:0,w:11"));
        win.add(button!("'F3 Library',l:13,b:0,w:13"));
        win.add(button!("'F4 Saves',l:27,b:0,w:12"));
        win.add(button!("'F5 Run',l:40,b:0,w:10"));

        win.tabs = win.add(tabs);
        win.mods = mods;
        win.details = details;
        win.library = library;
        win.library_details = library_details;
        win.saves = saves;
        win.save_details = save_details;
        win.run_log = run_log;
        win.status = status;
        win.library_entries = build_library_index(&win.mods_data);
        win.refresh_mod_list(0);
        win.refresh_library_list(0);
        win.refresh_save_selection();
        win.refresh_save_details();
        win.refresh_run_log(&startup_message);
        win.refresh_status();
        win
    }

    fn refresh_status(&mut self) {
        let enabled = self
            .mods_data
            .iter()
            .filter(|mod_entry| mod_entry.enabled)
            .count();
        let disabled = self.mods_data.len().saturating_sub(enabled);
        let text = format!("{enabled} enabled / {disabled} disabled");
        let status_handle = self.status;
        if let Some(status) = self.control_mut(status_handle) {
            status.set_caption(&text);
        }
    }

    fn refresh_mod_list(&mut self, select_index: usize) {
        let rows = self
            .mods_data
            .iter()
            .enumerate()
            .map(|(index, mod_entry)| (format_mod_row(index, mod_entry), mod_entry.enabled))
            .collect::<Vec<_>>();
        let mods_len = self.mods_data.len();
        let mods_handle = self.mods;
        if let Some(mods) = self.control_mut(mods_handle) {
            mods.clear();
            for (text, enabled) in rows {
                mods.add_item(listbox::Item::new(&text, enabled));
            }
            if mods_len > 0 {
                mods.set_index(select_index.min(mods_len - 1));
            }
        }
        self.refresh_details();
        self.refresh_status();
    }

    fn refresh_details(&mut self) {
        let details = self
            .selected_mod_index()
            .and_then(|index| {
                self.mods_data
                    .get(index)
                    .map(|mod_entry| format_mod_details(index, mod_entry))
            })
            .unwrap_or_else(|| "No mod selected.".to_string());

        let details_handle = self.details;
        if let Some(area) = self.control_mut(details_handle) {
            area.set_text(&details);
        }
    }

    fn refresh_library_list(&mut self, select_index: usize) {
        let rows = self
            .library_entries
            .iter()
            .enumerate()
            .map(|(index, entry)| format_library_row(index, entry))
            .collect::<Vec<_>>();
        let entry_count = self.library_entries.len();
        let library_handle = self.library;
        if let Some(library) = self.control_mut(library_handle) {
            library.clear();
            for text in rows {
                library.add(&text);
            }
            if entry_count > 0 {
                library.set_index(select_index.min(entry_count - 1));
            }
        }
        self.refresh_library_details();
    }

    fn refresh_library_details(&mut self) {
        let details = self
            .selected_library_index()
            .and_then(|index| self.library_entries.get(index).map(format_library_details))
            .unwrap_or_else(|| "No library entry selected.".to_string());

        let details_handle = self.library_details;
        if let Some(area) = self.control_mut(details_handle) {
            area.set_text(&details);
        }
    }

    fn refresh_save_selection(&mut self) {
        let save_files = scan_game_files(&self.root);
        let saves_handle = self.saves;
        if let Some(saves) = self.control_mut(saves_handle) {
            saves.clear();
            for save in save_files {
                saves.add(&save);
            }
            if saves.count() > 0 {
                saves.set_index(0);
            }
        }
        self.refresh_save_details();
    }

    fn refresh_save_details(&mut self) {
        let save = self.selected_save_path();
        let details = format_save_details(&self.root, &save);
        let details_handle = self.save_details;
        if let Some(area) = self.control_mut(details_handle) {
            area.set_text(&details);
        }
    }

    fn refresh_run_log(&mut self, text: &str) {
        let run_log_handle = self.run_log;
        if let Some(log) = self.control_mut(run_log_handle) {
            log.set_text(text);
        }
    }

    fn selected_mod_index(&self) -> Option<usize> {
        self.control(self.mods)
            .map(|mods| mods.index())
            .filter(|index| *index < self.mods_data.len())
    }

    fn selected_library_index(&self) -> Option<usize> {
        self.control(self.library)
            .map(|library| library.index())
            .filter(|index| *index < self.library_entries.len())
    }

    fn selected_save_path(&self) -> String {
        self.control(self.saves)
            .and_then(|saves| {
                saves
                    .item(saves.index())
                    .map(|item| item.text().to_string())
            })
            .unwrap_or_else(|| "game.json".to_string())
    }

    fn set_tab(&mut self, index: usize) {
        let tabs_handle = self.tabs;
        if let Some(tabs) = self.control_mut(tabs_handle) {
            tabs.set_current_tab(index);
        }
    }

    fn toggle_selected_mod(&mut self) {
        let Some(index) = self.selected_mod_index() else {
            return;
        };
        self.mods_data[index].enabled = !self.mods_data[index].enabled;
        let name = self.mods_data[index].name.clone();
        let state = self.mods_data[index].status();
        self.refresh_mod_list(index);
        self.refresh_run_log(&format!("{name} is now {state}. Load order was preserved."));
    }

    fn move_selected_mod(&mut self, direction: MoveDirection) {
        let Some(index) = self.selected_mod_index() else {
            return;
        };

        let target = match direction {
            MoveDirection::Up if index > 0 => index - 1,
            MoveDirection::Down if index + 1 < self.mods_data.len() => index + 1,
            _ => {
                self.refresh_run_log("Selected mod is already at that edge of the load order.");
                return;
            }
        };

        self.mods_data.swap(index, target);
        let name = self.mods_data[target].name.clone();
        self.refresh_mod_list(target);
        self.refresh_run_log(&format!("{name} moved to load order slot {}.", target + 1));
    }

    fn validate_mods(&mut self) {
        let enabled = self.enabled_mods();
        self.refresh_run_log(&format!(
            "MOD LIST\n\nEnabled load order:\n{}\n\nThe runner receives this ordered mod list and the save selected on the Saves tab.",
            format_mod_load_list(&enabled)
        ));
    }

    fn enabled_mods(&self) -> Vec<ModEntry> {
        self.mods_data
            .iter()
            .filter(|mod_entry| mod_entry.enabled)
            .cloned()
            .collect()
    }

    fn run_selected_save(&mut self) {
        self.set_tab(4); // Switch to run tab just to show log
        match self.save_playset_and_run() {
            Ok(text) => self.refresh_run_log(&text),
            Err(err) => self.refresh_run_log(&format!("RUN FAILED\n\n{err}")),
        }
    }

    fn save_playset_and_run(&self) -> Result<String, Box<dyn std::error::Error>> {
        let playset_path = self.save_current_playset()?;
        
        let selected_save = self.selected_save_path();
        let pid = launch_runner(
            &self.root,
            &self.launcher_config.runner_cmd,
            &selected_save,
        )?;

        let enabled = self.enabled_mods();
        Ok(format!(
             "GAME LAUNCHED IN BACKGROUND\n\n\
             Process ID\n  {}\n\n\
             Playset\n  {}\n\n\
             Save\n  {}\n\n\
             Enabled mod load list\n{}\n\n\
             The final exit code or signal will be appended to launcher_run.log.",
            pid,
            playset_path.display(),
            selected_save,
            format_mod_load_list(&enabled),
        ))
    }

    fn save_current_playset(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let playset_path = self.root.join("playset.toml");
        
        let playset = PlaysetToml {
            name: "launcher".to_string(),
            mods: self
                .mods_data
                .iter()
                .filter(|mod_entry| mod_entry.enabled)
                .map(|mod_entry| PlaysetModToml {
                    descriptor: descriptor_for_playset(
                        &mod_entry.manifest_path,
                        &playset_path,
                        &self.root,
                    ),
                })
                .collect(),
        };
        fs::write(&playset_path, toml::to_string_pretty(&playset)?)?;
        Ok(playset_path)
    }

    fn dispatch_command(&mut self, command_id: launcherwindow::Commands) {
        match command_id {
            launcherwindow::Commands::Home => self.set_tab(0),
            launcherwindow::Commands::ManageMods => self.set_tab(1),
            launcherwindow::Commands::BrowseLibrary => self.set_tab(2),
            launcherwindow::Commands::SelectSave => self.set_tab(3),
            launcherwindow::Commands::RunSave => self.run_selected_save(),
            launcherwindow::Commands::Toggle => self.toggle_selected_mod(),
            launcherwindow::Commands::MoveUp => self.move_selected_mod(MoveDirection::Up),
            launcherwindow::Commands::MoveDown => self.move_selected_mod(MoveDirection::Down),
            launcherwindow::Commands::Validate => self.validate_mods(),
            launcherwindow::Commands::Quit => self.close(),
        }
    }
}

impl ButtonEvents for LauncherWindow {
    fn on_pressed(&mut self, handle: Handle<Button>) -> EventProcessStatus {
        let Some(caption) = self
            .control(handle)
            .map(|button| button.caption().to_string())
        else {
            return EventProcessStatus::Ignored;
        };

        match caption.as_str() {
            "F2 Mods" | "Mods" => self.set_tab(1),
            "F3 Library" | "Library" => self.set_tab(2),
            "F4 Saves" | "Saves" => self.set_tab(3),
            "F5 Run" | "Run" | "Launch Game" => self.run_selected_save(),
            "Esc Home" | "Back Home" => self.set_tab(0),
            "Toggle" => self.toggle_selected_mod(),
            "Move Up" | "Up" => self.move_selected_mod(MoveDirection::Up),
            "Move Down" | "Down" => self.move_selected_mod(MoveDirection::Down),
            "Validate" | "Check" => self.validate_mods(),
            "Quit" => self.close(),
            _ => return EventProcessStatus::Ignored,
        }
        EventProcessStatus::Processed
    }
}

impl ListBoxEvents for LauncherWindow {
    fn on_current_item_changed(
        &mut self,
        handle: Handle<ListBox>,
        _index: usize,
    ) -> EventProcessStatus {
        if handle == self.mods {
            self.refresh_details();
        } else if handle == self.library {
            self.refresh_library_details();
        } else if handle == self.saves {
            self.refresh_save_details();
        }
        EventProcessStatus::Processed
    }

    fn on_item_checked(
        &mut self,
        handle: Handle<ListBox>,
        index: usize,
        checked: bool,
    ) -> EventProcessStatus {
        if handle == self.mods {
            if let Some(mod_entry) = self.mods_data.get_mut(index) {
                mod_entry.enabled = checked;
            }
            self.refresh_mod_list(index);
        }
        EventProcessStatus::Processed
    }
}

impl TabEvents for LauncherWindow {
    fn on_tab_changed(
        &mut self,
        _handle: Handle<Tab>,
        _new_tab_index: u32,
        _old_tab_index: u32,
    ) -> EventProcessStatus {
        self.refresh_status();
        EventProcessStatus::Processed
    }
}

impl CommandBarEvents for LauncherWindow {
    fn on_update_commandbar(&self, commandbar: &mut CommandBar) {
        commandbar.set(key!("F1"), "Home", launcherwindow::Commands::Home);
        commandbar.set(key!("F2"), "Mods", launcherwindow::Commands::ManageMods);
        commandbar.set(
            key!("F3"),
            "Library",
            launcherwindow::Commands::BrowseLibrary,
        );
        commandbar.set(key!("F4"), "Saves", launcherwindow::Commands::SelectSave);
        commandbar.set(key!("F5"), "Run", launcherwindow::Commands::RunSave);
        commandbar.set(key!("F6"), "Toggle", launcherwindow::Commands::Toggle);
        commandbar.set(key!("F7"), "Move Up", launcherwindow::Commands::MoveUp);
        commandbar.set(key!("F8"), "Move Down", launcherwindow::Commands::MoveDown);
        commandbar.set(key!("Escape"), "Home", launcherwindow::Commands::Home);
        commandbar.set(key!("Alt+F4"), "Quit", launcherwindow::Commands::Quit);
    }

    fn on_event(&mut self, command_id: launcherwindow::Commands) {
        self.dispatch_command(command_id);
    }
}

#[derive(Clone, Copy)]
enum MoveDirection {
    Up,
    Down,
}
