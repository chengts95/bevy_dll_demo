use appcui::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

include!("types.rs");
include!("app.rs");
include!("ui.rs");
include!("scanner.rs");
include!("runner.rs");

fn main() -> Result<(), appcui::system::Error> {
    let mut app = App::new()
        .single_window()
        .command_bar()
        .restore_screen(false)
        .build()?;
    App::set_theme(turbo_theme());
    app.add_window(LauncherWindow::new());
    app.run();
    Ok(())
}
