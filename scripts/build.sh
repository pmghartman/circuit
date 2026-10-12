#!/usr/bin/env bash
# build web server and/or desktop bundles; installs missing toolchain
# usage: scripts/build.sh [web|desktop|all]
set -euo pipefail
cd "$(dirname "$0")/.."
what=${1:-all}

rust() {
    export PATH="$HOME/.cargo/bin:$PATH"
    command -v cargo >/dev/null || curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
}

# webview + gtk headers on linux, xcode tools on macos
desktop_deps() {
    case $(uname) in
    Darwin) xcode-select -p >/dev/null 2>&1 || xcode-select --install ;;
    Linux)
        pkg-config --exists webkit2gtk-4.1 2>/dev/null && return
        if command -v apt-get >/dev/null; then
            sudo apt-get install -y build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev libayatana-appindicator3-dev
        elif command -v dnf >/dev/null; then
            sudo dnf install -y gcc pkgconf webkit2gtk4.1-devel gtk3-devel librsvg2-devel libappindicator-gtk3-devel
        elif command -v pacman >/dev/null; then
            sudo pacman -S --needed --noconfirm base-devel webkit2gtk-4.1 gtk3 librsvg
        else
            echo "install webkit2gtk-4.1 and gtk3 dev packages" >&2
            exit 1
        fi
        ;;
    esac
}

# major.minor from Cargo.toml, patch = commit count
version() {
    local mm
    mm=$(sed -n 's/^version = "\([0-9]*\.[0-9]*\)\..*"/\1/p' Cargo.toml | head -1)
    echo "$mm.$(git rev-list --count HEAD 2>/dev/null || echo 0)"
}

rust
case $what in web | desktop | all) ;; *) echo "usage: $0 [web|desktop|all]" >&2 && exit 2 ;; esac

if [[ $what != desktop ]]; then
    cargo build --release -p circuit-server
    echo "web $(version): target/release/circuit-server"
fi
if [[ $what != web ]]; then
    desktop_deps
    cargo tauri --version >/dev/null 2>&1 || cargo install tauri-cli --version '^2' --locked
    cargo tauri build --config "{\"version\":\"$(version)\"}"
    echo "desktop $(version): target/release/bundle/"
fi
