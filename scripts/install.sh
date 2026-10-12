#!/usr/bin/env bash
# linux: run the server from this checkout as a systemd user service on $PORT
# run as the service user, not root; the checkout must be writable by that user
# later updates: scripts/update.sh
# env: PORT
set -euo pipefail
[[ $(uname) == Linux ]] || { echo "linux only (systemd); desktop app: scripts/app_build.sh" >&2 && exit 1; }
[[ $EUID -ne 0 ]] || { echo "run without sudo: installs a systemd user service for the calling user" >&2 && exit 1; }
dir=$(cd "$(dirname "$0")/.." && pwd)
[[ -w $dir && -w $dir/.git ]] || { echo "$dir not writable by $USER: sudo chown -R $USER: $dir" >&2 && exit 1; }
port=${PORT:-8099}
config=${XDG_CONFIG_HOME:-$HOME/.config}
units=$config/systemd/user

# drop the old webhook watcher and poll timer from earlier installs
systemctl --user disable --now circuit-update.timer circuit-update.path 2>/dev/null || true
rm -f "$units"/circuit-update.{service,timer,path} "$config/circuit/hook.env"
rmdir "$config/circuit" 2>/dev/null || true

mkdir -p "$units"
cat >"$units/circuit.service" <<UNIT
[Unit]
Description=circuit on :$port

[Service]
Environment=PORT=$port
ExecStart=$dir/target/release/circuit-server
Restart=on-failure
NoNewPrivileges=true

[Install]
WantedBy=default.target
UNIT

# keep user services up without a login session
loginctl enable-linger "$USER" 2>/dev/null || sudo loginctl enable-linger "$USER"
systemctl --user daemon-reload
systemctl --user enable circuit.service
"$dir/scripts/update.sh"
echo "serving on :$port; update with scripts/update.sh"
echo "logs: journalctl --user -u circuit -f"
