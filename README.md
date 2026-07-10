# Bevy ECS DLL Dynamic Mod Demo

This project demonstrates a fully decoupled, data-driven ECS architecture using **Bevy ECS** and dynamic library loading (`cdylib`), heavily inspired by `emt_rs`.

## Architecture

The project consists of several independent modules, entirely decoupled through the ECS World:
- **`shared_api`**: Defines the central Component schemas (`Transform`, `Visual`, `Velocity`, `InputState`, etc.).
- **`mod_canvas`**: A completely stateless renderer using `minifb` and `tiny-skia`. It also reads keyboard inputs and syncs them to the `InputState` ECS Resource.
- **`mod_block`**: A pure logic engine that continuously rotates entities that possess the `Spin` component.
- **`mod_physics`**: A pure physics engine that applies gravity, integrates velocity into positional transforms, and resolves ground and wall collisions.
- **`mod_player`**: A dedicated player controller mod that reads `InputState` (Left/Right/Space) and translates it into physical `Velocity` vectors.
- **`mod_game_loader`**: A structural mod that reads the `game.json` case file and injects `ecs_prefab` entities into the game world at startup.

### Core Dependencies (Vendor)
To make this repository 100% self-contained, the highly customized engine cores are bundled in the `libs/` directory:
- **`libs/bevy_base`**: A stripped-down, customized version of Bevy's ECS and App components, tailored for this dynamic multi-DLL architecture.
- **`libs/ecs_prefab`**: The JSON snapshot/deserialization library that injects entity hierarchies at runtime.

### Mod Loading via DLL
The `game_runner` executable acts as a thin host. It:
1. Reads `playset.toml`.
2. Locates each `mod.toml` defined in the playset.
3. Loads the specified `.so` Linux shared library for each mod dynamically.
4. Searches the mod's `prefabs/` directory to aggregate a global `AppModPrefabs` library.
5. Invokes `load_case()` via `mod_game_loader` which then merges inline JSON prefabs and spawns the map entities.

## Building and Running

Because the mods are compiled as dynamic libraries (`cdylib`), they are output to Cargo's target folder. To ensure the runner can find them via the `mod.toml` distribution path, use the provided script:

```bash
./build_and_run.sh
```

This script will compile the workspace, distribute the generated `.so` libraries to their respective `mods/...` folders, and execute the `game_runner`.
