#!/usr/bin/env bash
# pull the latest main and build the tauri desktop app
# macos: also replaces /Applications/circuit.app
# usage: scripts/app_build.sh
set -euo pipefail

# whole body parsed first: pull may rewrite this file
main() {
    cd "$(dirname "$0")/.."
    git pull -q --ff-only origin main
    scripts/build.sh desktop
    if [[ $(uname) == Darwin ]]; then
        rm -rf "/Applications/Circuit Simulator.app" /Applications/circuit.app
        cp -R target/release/bundle/macos/circuit.app /Applications/
        echo "installed /Applications/circuit.app"
    fi
}
main "$@"; exit
