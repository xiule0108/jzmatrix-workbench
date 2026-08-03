use serde_json::Value;
use tauri::Manager;

#[tauri::command]
fn doctor(app: tauri::AppHandle) -> matrix_core::CliResponse {
    match app.path().app_data_dir() {
        Ok(path) => matrix_core::run_doctor(&path),
        Err(_) => matrix_core::blocked_response("app_data_unavailable", "无法解析应用数据目录"),
    }
}

#[tauri::command]
fn offline_demo() -> Result<Value, String> {
    matrix_core::offline_demo().map_err(|_| "随包离线 fixture 校验失败".to_owned())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![doctor, offline_demo])
        .run(tauri::generate_context!())
        .expect("error while running JZMatrix Workbench");
}
