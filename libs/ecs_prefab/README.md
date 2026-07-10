# ECS Prefab

`ecs_prefab` is a domain-agnostic entity template system for Bevy ECS. It allows you to define reusable entity sub-trees (prefabs) using JSON, and seamlessly instantiate them into your game world at runtime.

## Features

- **Data-Driven Instantiation**: Define complex entity hierarchies and component data purely in JSON without hardcoding them in Rust.
- **Flat Entity Snapshot**: Prefabs are stored as flattened arrays of `EntityRecord`s, using `parent` ids to reconstruct the `ChildOf` hierarchy after parsing.
- **Domain Agnostic**: `ecs_prefab` knows nothing about your specific game components (like `Transform`, `Velocity`, `Player`). It relies on generic `serde_json::Value` parsing and dynamic component registration via `PrefabRegistry`.
- **Parameter Injection (`$param`)**: Pass dynamic parameters into your prefabs when spawning them (e.g. `$speed`, `$color`), allowing deep reuse of common entity structures.
- **Composition (`Use`)**: Prefabs can instantiate other prefabs inside themselves, enabling complex composition.

## Usage

Define your templates in a JSON file (e.g., `game.json`), parse it into a `Library`, and then use `PrefabRegistry::spawn_class` to inject the prefabs into your Bevy `World`.

All parsing and hierarchy reconstruction is handled automatically by the library!
