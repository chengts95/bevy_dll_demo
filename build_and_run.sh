#!/bin/bash
set -e

BUILD_MODE="debug"
CARGO_ARGS=""

if [[ "$1" == "--release" ]]; then
    BUILD_MODE="release"
    CARGO_ARGS="--release"
fi

echo "==> Building Workspace ($BUILD_MODE)..."
cargo build $CARGO_ARGS

# Dynamically extract Cargo's target directory (handles overrides in .cargo/config.toml)
CARGO_TARGET_DIR=$(cargo metadata --no-deps --format-version 1 | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
TARGET_DIR="$CARGO_TARGET_DIR/$BUILD_MODE"

echo "==> Distributing DLLs based on playset.toml and mod.toml ($BUILD_MODE)..."

DESCRIPTORS=$(grep -h 'descriptor' playset.toml playsets/*.toml | sed -E 's/.*descriptor = "(.*)".*/\1/' | sed 's#^\.\./##' | sort -u)

for desc in $DESCRIPTORS; do
    MOD_DIR=$(dirname "$desc")
    CRATE_NAME=$(basename "$MOD_DIR")
    
    if [[ "$BUILD_MODE" == "release" ]]; then
        DLL_TARGET=$(grep -A 5 '\[platform.linux\]' "$desc" | grep 'dll_path_release' | head -n 1 | sed -E 's/.*= *"(.*)".*/\1/')
    else
        DLL_TARGET=$(grep -A 5 '\[platform.linux\]' "$desc" | grep 'dll_path_debug' | head -n 1 | sed -E 's/.*= *"(.*)".*/\1/')
    fi
    
    # Fallback to default if not found
    if [[ -z "$DLL_TARGET" ]]; then 
        DLL_TARGET="lib${CRATE_NAME}.so"
    fi
    
    SOURCE_DLL="$TARGET_DIR/lib${CRATE_NAME}.so"
    echo "  -> Copying $SOURCE_DLL to $MOD_DIR/$DLL_TARGET"
    cp "$SOURCE_DLL" "$MOD_DIR/$DLL_TARGET"
done

echo "==> Running Game Runner ($BUILD_MODE)..."
cargo run --bin game_runner $CARGO_ARGS
