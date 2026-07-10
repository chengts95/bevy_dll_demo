#!/bin/bash
set -e

echo "==> Building Workspace..."
cargo build

echo "==> Distributing DLLs to Mod Directories..."
TARGET_DIR="/tmp/bevy_macroquad_target/debug"

cp "$TARGET_DIR/libmod_canvas.so" "mods/mod_canvas/"
cp "$TARGET_DIR/libmod_block.so" "mods/mod_block/"
cp "$TARGET_DIR/libmod_physics.so" "mods/mod_physics/"
cp "$TARGET_DIR/libmod_player.so" "mods/mod_player/"
cp "$TARGET_DIR/libmod_game_loader.so" "mods/mod_game_loader/"

echo "==> Running Game Runner..."
cargo run --bin game_runner
