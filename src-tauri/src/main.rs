// desktop shell over the same ui, api via invoke; the header's update button
// rebuilds the app from the checkout it was built from (scripts/app_build.sh)
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use tauri::Emitter;

#[tauri::command]
fn solve(parts: Vec<circuit::PartIn>, width: f32, height: f32) -> Result<circuit::Sim, String> {
    circuit::run(&parts, width, height)
}

#[tauri::command]
fn version() -> &'static str {
    circuit::VERSION
}

// the checkout this app rebuilds itself from: CIRCUIT_SRC, else the one it
// was built from (baked in at build time)
fn source_dir() -> Result<PathBuf, String> {
    let dir = std::env::var_os("CIRCUIT_SRC")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/..")));
    let dir = dir.canonicalize().unwrap_or(dir);
    if !dir.join("scripts/app_build.sh").is_file() {
        return Err(format!("no circuit checkout at {} (set CIRCUIT_SRC to one)", dir.display()));
    }
    Ok(dir)
}

#[tauri::command]
fn source() -> Result<String, String> {
    source_dir().map(|d| d.display().to_string())
}

// scripts/app_build.sh in the checkout, its output sent to the page line by
// line as "update-log"; the exit code when it ends
#[tauri::command]
async fn update_from_source(app: tauri::AppHandle) -> Result<i32, String> {
    let dir = source_dir()?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut child = Command::new("bash")
            .args(["-c", "exec 2>&1; exec bash scripts/app_build.sh"])
            .current_dir(&dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| format!("couldn't run scripts/app_build.sh in {}: {e}", dir.display()))?;
        let out = child.stdout.take().ok_or("no output")?;
        let _ = app.emit("update-log", format!("[app] updating from {}", dir.display()));
        for line in BufReader::new(out).lines() {
            let Ok(line) = line else { break };
            let _ = app.emit("update-log", line);
        }
        let status = child.wait().map_err(|e| e.to_string())?;
        Ok(status.code().unwrap_or(-1))
    })
    .await
    .map_err(|e| e.to_string())?
}

// this app started again, the new build when the update installed one
#[tauri::command]
fn relaunch(app: tauri::AppHandle) {
    app.restart()
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![solve, version, source, update_from_source, relaunch])
        .run(tauri::generate_context!())
        .expect("tauri runtime");
}
