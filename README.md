# circuit

Schematic editor with a live Rust circuit solver. Web server or Tauri desktop app.

| Path | Contents |
| --- | --- |
| `core/` | solver, transient, graph layout |
| `server/` | web server, UI embedded in the binary |
| `src-tauri/` | desktop app |
| `ui/` | HTML/JS/CSS, no build step |
| `scripts/` | app_build, update, install, uninstall, build |

## Build

```bash
scripts/app_build.sh        # pull main, build the desktop app (macOS: install to /Applications)
scripts/update.sh           # pull main, build the server, restart its service if installed
scripts/build.sh web|desktop  # build only, no pull
```

Installs Rust and any missing system packages. Dev: `cargo run -p circuit-server`, `cargo test`.

On macOS, `scripts/uninstall.sh` removes the desktop app; `--purge` also deletes its saved data.

## Server setup (Linux)

As the service user, not `sudo`:

```bash
git clone https://github.com/pmghartman/circuit.git ~/circuit
PORT=8099 ~/circuit/scripts/install.sh
```

This runs the server as a systemd user service. There is no automatic redeploy: run `scripts/update.sh` to pull and rebuild. Logs: `journalctl --user -u circuit -f`. Remove: `scripts/uninstall.sh`.

## API

| Endpoint | Body | Returns |
| --- | --- | --- |
| `POST api/solve` | `{ parts, width, height }`, part = `{ name, kind, from, to, value, ctrl }` | `{ elapsedMs, skipped, frames }`; `400` bad JSON, `413` over 1 MiB, `422 { error }` |
| `GET api/version` | | `{ version }` |
| `GET *` | | the UI |

`kind` is `R C V I E F G H`; nets are named, `gnd` is ground; `ctrl` is `[net+, net-]` for E/G or `[part]` for F/H.
