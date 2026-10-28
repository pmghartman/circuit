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

## Updating from source

Both versions carry an **update** button in the header, next to the version:

- the web server runs `scripts/update.sh` in its own checkout (the one it was built from, or `CIRCUIT_SRC`): pull `main`, rebuild, restart the systemd user service when one runs it. Under systemd the update runs as a transient unit of its own, so the restart doesn't kill it. The page shows the log and reloads once the new version answers. `CIRCUIT_UPDATES=off` in the unit's environment hides the button.
- the desktop app runs `scripts/app_build.sh` in the checkout it was built from (or `CIRCUIT_SRC`), shows the build log, and offers to relaunch once the new app is installed (macOS replaces `/Applications/circuit.app`; elsewhere install the bundle it names).

On macOS, `scripts/uninstall.sh` removes the desktop app; `--purge` also deletes its saved data.

## Server setup (Linux)

As the service user, not `sudo`:

```bash
git clone https://github.com/pmghartman/circuit.git ~/circuit
PORT=8099 ~/circuit/scripts/install.sh
```

This runs the server as a systemd user service. There is no automatic redeploy: press **update** in the page, or run `scripts/update.sh`, to pull and rebuild. Logs: `journalctl --user -u circuit -f`. Remove: `scripts/uninstall.sh`.

On a [phosphorus](https://github.com/pmghartman/phosphorus) host, publish it on the mesh desktop by adding to `config/services.export`:

```ini
[circuit]
port = 8099
mode = path
```

The UI pins its relative URLs to the page's own directory, so it works under phosphorus's `/circuit/` path as it does at the root.

## API

| Endpoint | Body | Returns |
| --- | --- | --- |
| `POST api/solve` | `{ parts, width, height }`, part = `{ name, kind, from, to, value, ctrl }` | `{ elapsedMs, skipped, frames }`; `400` bad JSON, `413` over 1 MiB, `422 { error }` |
| `GET api/version` | | `{ version }` |
| `GET api/update` | | `{ version, canUpdate, state: idle\|running\|ok\|failed, log, from, to, restartNeeded }` |
| `POST api/update` | | starts `scripts/update.sh`; `202` the status above, `409 { error }` |
| `GET *` | | the UI |

`kind` is `R C V I E F G H`; nets are named, `gnd` is ground; `ctrl` is `[net+, net-]` for E/G or `[part]` for F/H.
