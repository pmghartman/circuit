#!/usr/bin/env bash
# linux: remove the user service (and any old update timer / webhook watcher); keep the checkout
# macos: remove the desktop app, including the old "Circuit Simulator" build; --purge also drops its saved data
# usage: scripts/uninstall.sh [--purge]
set -euo pipefail

if [[ $(uname) == Darwin ]]; then
    for app in {/Applications,"$HOME/Applications"}/{circuit,"Circuit Simulator"}.app; do
        [[ -e $app ]] && rm -rf "$app" && echo "removed $app"
    done
    data=("$HOME"/Library/{Caches,WebKit,"Application Support",Preferences,"Saved Application State"}/dev.mulab.circuit{,simulator}{,.plist,.savedState})
    for d in "${data[@]}"; do
        [[ -e $d ]] || continue
        if [[ ${1:-} == --purge ]]; then rm -rf "$d" && echo "removed $d"; else echo "kept $d (--purge to remove)"; fi
    done
    exit
fi

units=${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user
systemctl --user disable --now circuit.service circuit-update.timer circuit-update.path 2>/dev/null || true
rm -f "$units"/circuit.service "$units"/circuit-update.{service,timer,path}
rm -rf "${XDG_CONFIG_HOME:-$HOME/.config}/circuit"
systemctl --user daemon-reload
echo "removed circuit units"
