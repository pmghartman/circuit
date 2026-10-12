// desktop shell over the same ui, api via invoke
#[tauri::command]
fn solve(parts: Vec<circuit::PartIn>, width: f32, height: f32) -> Result<circuit::Sim, String> {
    circuit::run(&parts, width, height)
}

#[tauri::command]
fn version() -> &'static str {
    circuit::VERSION
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![solve, version])
        .run(tauri::generate_context!())
        .expect("tauri runtime");
}
