#!/usr/bin/env bash
# Builds CoinCraft installers for this computer's system (macOS: .app + .dmg; Linux: .deb + AppImage).
# Run from anywhere: ./scripts/package.sh   The results go in dist/.
set -euo pipefail
cd "$(dirname "$0")/.."

if ! command -v cargo >/dev/null 2>&1; then
    echo "Rust isn't installed. Get it from https://rustup.rs and run this again." >&2
    exit 1
fi
if ! cargo packager --version >/dev/null 2>&1; then
    echo "Installing cargo-packager (first time only)…"
    cargo install cargo-packager --locked
fi

if [ "$(uname)" = "Linux" ] && command -v dpkg >/dev/null 2>&1; then
    for lib in libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev pkg-config; do
        if ! dpkg -s "$lib" >/dev/null 2>&1; then
            echo "Missing $lib. Install with: sudo apt install libxkbcommon-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev pkg-config" >&2
            exit 1
        fi
    done
fi

cargo packager --release
echo
echo "Done. Installers are in: $(pwd)/dist"
ls -1 dist
