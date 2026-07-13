#!/bin/bash
set -e

CARGO_TARGET_DIR=$(cargo metadata --no-deps --format-version 1 | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')
TARGET_DIR="$CARGO_TARGET_DIR/release"
echo "==> Using existing release artifacts from $TARGET_DIR"

echo "==> Assembling release_package..."
rm -rf release_package
mkdir -p release_package/mods

for bin in game_runner launcher; do
    if [[ ! -x "$TARGET_DIR/$bin" ]]; then
        echo "Missing release binary: $TARGET_DIR/$bin" >&2
        echo "Build it first with: cargo build --release" >&2
        exit 1
    fi
done

cp "$TARGET_DIR/game_runner" release_package/
cp "$TARGET_DIR/launcher" release_package/
cp game.json release_package/
cp -R playsets release_package/

cat << 'EOF' > release_package/launcher.toml
start_in = "."
runner_cmd = "./game_runner"
EOF

# Create an initial playset.toml for the release package
echo 'name = "launcher"' > release_package/playset.toml
echo '' >> release_package/playset.toml

DESCRIPTORS=$(grep -h 'descriptor' playset.toml playsets/*.toml | sed -E 's/.*descriptor = "(.*)".*/\1/' | sed 's#^\.\./##' | sort -u)

for desc in $DESCRIPTORS; do
    # desc is e.g. "libs/mod_physics/mod.toml"
    CRATE_NAME=$(basename $(dirname "$desc"))
    
    DLL_TARGET=$(grep -A 5 '\[platform.linux\]' "$desc" | grep 'dll_path_release' | head -n 1 | sed -E 's/.*= *"(.*)".*/\1/')
    if [[ -z "$DLL_TARGET" ]]; then DLL_TARGET="lib${CRATE_NAME}.so"; fi
    
    # We want to deploy to release_package/mods/<crate_name>
    DEPLOY_DIR="release_package/mods/$CRATE_NAME"
    mkdir -p "$DEPLOY_DIR"
    
    # Copy mod.toml
    cp "$desc" "$DEPLOY_DIR/mod.toml"
    
    # Copy the compiled .so from target dir directly to the release package!
    SOURCE_DLL="$TARGET_DIR/lib${CRATE_NAME}.so"
    if [[ ! -f "$SOURCE_DLL" ]]; then
        echo "Missing release DLL for $CRATE_NAME: $SOURCE_DLL" >&2
        echo "Build it first with: cargo build --release" >&2
        exit 1
    fi
    echo "  -> Packaging $CRATE_NAME..."
    cp "$SOURCE_DLL" "$DEPLOY_DIR/$DLL_TARGET"

done

for desc in $(grep 'descriptor' playset.toml | sed -E 's/.*descriptor = "(.*)".*/\1/'); do
    CRATE_NAME=$(basename $(dirname "$desc"))
    echo '[[mods]]' >> release_package/playset.toml
    echo "descriptor = \"mods/$CRATE_NAME/mod.toml\"" >> release_package/playset.toml
    echo '' >> release_package/playset.toml
done

echo "==> Done! Your standalone game is ready to ship in the 'release_package' folder."
