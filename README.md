# Bevy DLL Demo Engine

A fully modular, data-driven game engine architecture built on top of a customized Bevy ECS. This project demonstrates how to build an industrial-grade plugin architecture where the core game logic is entirely separated into dynamic libraries (DLLs/.so), orchestrated by a thin, native host executable.

## Features & Achieved Effects

This repository successfully achieves the following architectural milestones:

- **True Cross-DLL ECS:** By customizing Bevy's internal `TypeId` generation to use deterministic, name-based hashing (`StableTypeId`), `World` and `App` pointers can be passed securely across FFI boundaries. Components spawned in one DLL can be queried and modified by systems in completely different DLLs.
- **Pure Data-Driven Assembly:** The engine knows absolutely nothing about the game it is running. The host application (`game_runner`) relies entirely on a `playset.toml` blueprint to discover, load, and distribute plugins dynamically at runtime.
- **Prefab Hydration:** World states and entity templates are loaded dynamically from `game.json` using the custom `ecs_prefab` library. Game design is driven by JSON configuration rather than hardcoded Rust initialization.
- **Standalone Release Packaging:** The architecture perfectly separates build artifacts from the source tree. A single packaging script can extract only the required executables, `.toml`/`.json` configs, and compiled `.so`/`.dll` libraries into a standalone, shippable folder.

## Architecture Structure

- `host/`: The root workspace orchestrator.
- `game_runner/`: The minimal host executable. It parses `playset.toml`, loads the specified dynamic libraries, and delegates the ECS tick loop to the loaded plugins.
- `shared_api/`: The critical ABI contract. Contains the shared components (e.g., `InputState`, `Velocity`) and resources that must be understood by multiple DLLs.
- `mods/`: The dynamic plugins (Mods). Each Mod is compiled as a `cdylib` and exposes a standard C-ABI `setup_mod` function.
  - `mod_canvas`: Handles window creation, rendering loops, and passing raw input states to the ECS.
  - `mod_physics`: Handles core physics ticks, gravity, and position integrations.
  - `mod_player`: Reads input events and applies logic/velocity to the player entity.
  - `mod_block`: Handles static world geometry logic.
  - `mod_game_loader`: A utility mod that reads `game.json` and spawns the initial world entities via `ecs_prefab`.
- `libs/`: Core engine dependencies.
  - `bevy_base`: A custom fork of `bevy_ecs` and `bevy_app` optimized for cross-DLL TypeId stability.
  - `ecs_prefab`: A powerful serialization library designed to convert JSON schemas into active ECS entities.

## Getting Started

### 1. Build and Play (TUI Launcher)

The engine comes with a powerful, Paradox-style TUI Launcher that visualizes the modular architecture. 

To launch it in your development environment, run:

```bash
cargo run -p launcher
```

The launcher reads the game command from `launcher.toml`. The checked-in
development configuration intentionally uses Cargo:

```toml
runner_cmd = "cargo run -p game_runner"
```

This keeps one development command working on both Linux and Windows, where
the native executable names and invocation syntax differ. Cargo is only a
development-time runner here; it is not required by a packaged game.

For a packaged build, `launcher.toml` must invoke the native executable instead:

```toml
# Linux package
runner_cmd = "./game_runner"

# Windows package
runner_cmd = "game_runner.exe"
```

`package.sh` writes the Linux packaged command automatically. A Windows
packaging process should write the `.exe` command shown above. The launcher
parses `runner_cmd` as a program plus arguments and does not execute it through
a shell, so shell operators such as `>`, `&&`, and `&` must not be used.

**Interactive Demo:** 
To truly see the power of the data-driven architecture, try navigating to the **Mods (F2)** tab in the Launcher. 
* Uncheck `mod_physics` -> Click `Launch Game` (F5) -> *Notice that gravity completely disappears from the game.*
* Uncheck `mod_player` -> Click `Launch Game` (F5) -> *Notice that the player character can no longer be controlled.*

### Swap the Player Avatar

`mod_player` is the standard controller and is independent of the player's
appearance. The selected prefab only needs `PlayerControl` and `Velocity` to
receive the same left/right/jump commands.

In `game.json`, change the player instance between these two prefab names:

```json
{ "id": "player", "prefab": "player_rect" }
```

```json
{ "id": "player", "prefab": "player_car" }
```

The rectangle is controlled without the car mod. The car uses the exact same
controller contract; `mod_car` only adds car-specific wheel presentation.

Because the engine relies purely on `playset.toml`, the Launcher acts as a Mod Manager that builds this playset. Disabling a mod simply prevents the host from injecting the DLL, turning off those features cleanly and instantly without modifying any source code.

### 2. Manual Build and Run (Scripted)

If you prefer to bypass the launcher, you can compile the entire workspace and launch the game locally via bash:

```bash
./build_and_run.sh
```

*(You can also run `./build_and_run.sh --release` for optimized performance).*

This script will automatically:
1. Compile the host and all mods.
2. Launch `game_runner`, reading from the static `playset.toml` in the root directory.

### 3. Package for Distribution

When you are ready to ship your game to players, run the packaging script:

```bash
./package.sh
```

This will compile the project in `--release` mode and assemble a clean, standalone `release_package/` directory. This folder contains only the compiled host executable, the configured `.toml`/`.json` files, and the isolated `mods/` directory containing the dynamic libraries. You can zip this folder and distribute it to players—they do not need the Rust toolchain to run it.

## Modding and Extending

To add a new feature or mod:
1. Create a new library crate in `mods/` (e.g., `mod_audio`) with `crate-type = ["cdylib"]`.
2. Define a `mod.toml` inside the mod folder to declare its platform distribution paths.
3. Register the new mod in `playset.toml` by adding its descriptor path.
4. Run `./build_and_run.sh`. The system will automatically compile, distribute, and load your new DLL!

## License

This project is dual-licensed under either of:

- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

at your option.

The `bevy_base` fork within this repository retains its original dual MIT/Apache 2.0 licenses from the official Bevy engine project.
