use std::path::PathBuf;

fn main() {
    let playset_path = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("playset.toml"));

    if let Err(err) = game_runner::run_playset(playset_path) {
        eprintln!("Game Runner: {err}");
        std::process::exit(1);
    }
}
