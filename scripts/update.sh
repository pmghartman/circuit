#!/usr/bin/env bash
# pull the latest main, build the server, restart the service if installed
# usage: scripts/update.sh
set -euo pipefail

# whole body parsed first: pull may rewrite this file
main() {
    cd "$(dirname "$0")/.."
    git pull -q --ff-only origin main
    scripts/build.sh web
    if [[ $(uname) == Linux ]] && systemctl --user is-enabled -q circuit 2>/dev/null; then
        systemctl --user restart circuit
        echo "restarted circuit"
    fi
}
main "$@"; exit
