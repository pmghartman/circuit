// an update of this checkout from upstream, pressed from the ui: scripts/update.sh
// run outside the server's own process and, under systemd, in a transient unit
// of its own, since the restart it ends in would otherwise kill it along with
// the server. Progress is read back from files under target/update, so the
// server that comes up after the restart still reports how it went
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

// an update still "running" past this is taken as lost
const STALE: Duration = Duration::from_secs(30 * 60);
const LOG_TAIL: usize = 6000;

// the checkout: CIRCUIT_SRC, else the one this binary was built from
pub fn source_dir() -> PathBuf {
    let dir = std::env::var_os("CIRCUIT_SRC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/..")));
    dir.canonicalize().unwrap_or(dir)
}

fn now_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(root).args(args).output().ok().filter(|o| o.status.success())?;
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

// the checkout's version as it stands, major.minor from Cargo.toml plus the
// commit count (as build.sh stamps it); differs from the running one after
// an update that didn't restart the server
pub fn disk_version(root: &Path) -> Option<String> {
    let cargo = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    let mm = cargo.lines().find_map(|l| {
        let v = l.strip_prefix("version = \"")?.strip_suffix('"')?;
        let mut it = v.split('.');
        Some(format!("{}.{}", it.next()?, it.next()?))
    })?;
    Some(format!("{mm}.{}", git(root, &["rev-list", "--count", "HEAD"])?))
}

pub struct Updater {
    root: PathBuf,
    dir: PathBuf,
}

impl Updater {
    pub fn new(root: PathBuf) -> Self {
        let dir = root.join("target/update");
        Self { root, dir }
    }

    fn read(&self, name: &str) -> Option<String> {
        fs::read_to_string(self.dir.join(name)).ok()
    }

    pub fn can_update(&self) -> bool {
        self.root.join(".git").exists() && self.root.join("scripts/update.sh").is_file()
    }

    // where the last or current update stands
    pub fn status(&self) -> Value {
        let mut out = json!({ "version": circuit::VERSION, "canUpdate": self.can_update(), "state": "idle", "log": "" });
        if !self.can_update() {
            out["why"] = json!(format!("{} is not a git checkout with scripts/update.sh", self.root.display()));
        }
        let Some(started) = self.read("started") else { return out };
        let meta: Value = serde_json::from_str(&started).unwrap_or(Value::Null);
        out["startedAt"] = meta["at"].clone();
        out["from"] = meta["from"].clone();
        let log = self.read("log").unwrap_or_default();
        out["log"] = json!(if log.len() > LOG_TAIL {
            let cut = &log[log.len() - LOG_TAIL..];
            cut.split_once('\n').map_or(cut, |(_, rest)| rest)
        } else {
            &log
        });
        if let Some(exit) = self.read("exit") {
            let code: i64 = exit.trim().parse().unwrap_or(-1);
            out["state"] = json!(if code == 0 { "ok" } else { "failed" });
            out["code"] = json!(code);
            if let Ok(t) = fs::metadata(self.dir.join("exit")).and_then(|m| m.modified()) {
                out["finishedAt"] = json!(t.duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0));
            }
            let to = disk_version(&self.root);
            out["restartNeeded"] = json!(code == 0 && to.as_deref().is_some_and(|v| v != circuit::VERSION));
            out["to"] = json!(to);
            return out;
        }
        let at = meta["at"].as_u64().unwrap_or(0) as u128;
        if now_ms().saturating_sub(at) > STALE.as_millis() {
            out["state"] = json!("failed");
            out["error"] = json!("the update never finished");
            return out;
        }
        out["state"] = json!("running");
        out
    }

    // the update started, or the one already running
    pub fn start(&self) -> Result<Value, String> {
        if !self.can_update() {
            return Err(format!("{} is not a git checkout with scripts/update.sh; update it by hand", self.root.display()));
        }
        let cur = self.status();
        if cur["state"] == "running" {
            return Ok(cur);
        }
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let _ = fs::remove_file(self.dir.join("exit"));
        fs::write(self.dir.join("log"), "").map_err(|e| e.to_string())?;
        fs::write(self.dir.join("started"), json!({ "at": now_ms() as u64, "from": circuit::VERSION }).to_string()).map_err(|e| e.to_string())?;
        let sh = format!(
            "exec >>{log} 2>&1; bash {script}; echo $? > {exit}",
            log = sh_quote(&self.dir.join("log").display().to_string()),
            script = sh_quote(&self.root.join("scripts/update.sh").display().to_string()),
            exit = sh_quote(&self.dir.join("exit").display().to_string()),
        );
        if !self.via_systemd(&sh) {
            let mut cmd = Command::new("bash");
            cmd.args(["-c", &sh]).current_dir(&self.root).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                cmd.process_group(0);
            }
            if let Err(e) = cmd.spawn() {
                let _ = fs::write(self.dir.join("log"), format!("couldn't start the update: {e}\n"));
                let _ = fs::write(self.dir.join("exit"), "1\n");
            }
        }
        Ok(self.status())
    }

    // inside a systemd unit (INVOCATION_ID is systemd's), the update runs as a
    // transient unit, so restarting circuit's unit doesn't take it down
    fn via_systemd(&self, sh: &str) -> bool {
        if !cfg!(target_os = "linux") || std::env::var_os("INVOCATION_ID").is_none() {
            return false;
        }
        let mut cmd = Command::new("systemd-run");
        cmd.args(["--user", "--collect", "--quiet", &format!("--unit=circuit-update-{}", now_ms()), &format!("--working-directory={}", self.root.display())]);
        if let Ok(path) = std::env::var("PATH") {
            cmd.arg(format!("--setenv=PATH={path}"));
        }
        cmd.args(["bash", "-c", sh]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        cmd.status().map(|s| s.success()).unwrap_or(false)
    }
}
