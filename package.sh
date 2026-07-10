#!/bin/bash
set -e

echo "==> Building Release Workspace..."
cargo build --release

CARGO_TARGET_DIR=$(cargo metadata --no-deps --format-version 1 | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
TARGET_DIR="$CARGO_TARGET_DIR/release"

echo "==> Assembling release_package..."
rm -rf release_package
mkdir -p release_package/mods

cp "$TARGET_DIR/game_runner" release_package/
cp playset.toml release_package/
cp game.json release_package/

for desc in $(grep 'descriptor' playset.toml | sed -E 's/.*descriptor = "(.*)".*/\1/'); do
    MOD_DIR=$(dirname "$desc")
    CRATE_NAME=$(basename "$MOD_DIR")
    
    DLL_TARGET=$(grep -A 5 '\[platform.linux\]' "$desc" | grep 'dll_path_release' | head -n 1 | sed -E 's/.*= *"(.*)".*/\1/')
    if [[ -z "$DLL_TARGET" ]]; then DLL_TARGET="lib${CRATE_NAME}.so"; fi
    
    # Create the mod folder in release_package
    mkdir -p "release_package/$MOD_DIR"
    
    # Copy mod.toml
    cp "$desc" "release_package/$desc"
    
    # Copy the compiled .so from target dir directly to the release package!
    SOURCE_DLL="$TARGET_DIR/lib${CRATE_NAME}.so"
    echo "  -> Packaging $CRATE_NAME..."
    cp "$SOURCE_DLL" "release_package/$MOD_DIR/$DLL_TARGET"
done

echo "==> Done! Your standalone game is ready to ship in the 'release_package' folder."
