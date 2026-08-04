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

#[tauri::command]
fn group_templates() -> Value {
    serde_json::json!({
        "templates": matrix_core::builtin_group_templates(),
        "source": "built_in_fixture",
        "local_only": true,
    })
}

#[tauri::command]
fn create_group(
    app: tauri::AppHandle,
    goal: String,
    template_id: String,
    idempotency_key: String,
) -> matrix_core::CliResponse {
    let Ok(app_data_dir) = app.path().app_data_dir() else {
        return matrix_core::blocked_command_response(
            "group.create",
            "app_data_unavailable",
            "无法解析应用数据目录",
        );
    };
    match matrix_core::create_collaboration_group(
        &app_data_dir.join("db/app.sqlite3"),
        matrix_core::CreateGroupInput {
            goal: &goal,
            template_id: &template_id,
            idempotency_key: &idempotency_key,
        },
    ) {
        Ok(group) => matrix_core::CliResponse {
            contract: "jzmatrix.cli-response".to_owned(),
            version: "1.0.0".to_owned(),
            command: "group.create".to_owned(),
            request_id: matrix_core::new_request_id(),
            ok: true,
            status: matrix_core::ResponseStatus::Pass,
            outcome: matrix_core::Outcome::Committed,
            data: serde_json::to_value(group).unwrap_or_else(|_| serde_json::json!({})),
            errors: Vec::new(),
            warnings: vec![matrix_core::WarningItem {
                code: "local_only".to_owned(),
                message: "仅写入本机事实源，不创建、启动或发送到外部工具".to_owned(),
            }],
            evidence: vec![matrix_core::EvidenceRef {
                kind: "sqlite".to_owned(),
                reference: "evidence:local-group".to_owned(),
            }],
            next_actions: Vec::new(),
            redactions: matrix_core::Redactions {
                profile: "v1".to_owned(),
                fields: vec!["secrets".to_owned(), "absolute_paths".to_owned()],
            },
            extensions: serde_json::json!({
                "network": false,
                "external_processes": false,
                "idempotent": true,
            }),
        },
        Err(error) => matrix_core::blocked_command_response(
            "group.create",
            error.code(),
            "本地协作组操作未完成",
        ),
    }
}

#[tauri::command]
fn discover_tools(app: tauri::AppHandle) -> matrix_core::CliResponse {
    let Ok(app_data_dir) = app.path().app_data_dir() else {
        return matrix_core::blocked_command_response(
            "tools.discover",
            "app_data_unavailable",
            "无法解析应用数据目录",
        );
    };
    match matrix_core::discover_local_tools(&app_data_dir.join("db/app.sqlite3")) {
        Ok(snapshot) => {
            let available = snapshot
                .tools
                .iter()
                .filter(|tool| tool.status == "available")
                .count();
            matrix_core::CliResponse {
                contract: "jzmatrix.cli-response".to_owned(),
                version: "1.0.0".to_owned(),
                command: "tools.discover".to_owned(),
                request_id: matrix_core::new_request_id(),
                ok: true,
                status: matrix_core::ResponseStatus::Pass,
                outcome: matrix_core::Outcome::Committed,
                data: serde_json::to_value(snapshot).unwrap_or_else(|_| serde_json::json!({})),
                errors: Vec::new(),
                warnings: vec![matrix_core::WarningItem {
                    code: "allowlisted_user_triggered_probe".to_owned(),
                    message: format!(
                        "只检查固定白名单二进制的 --version/--help，发现 {available} 个可用入口；未读取配置或会话"
                    ),
                }],
                evidence: vec![matrix_core::EvidenceRef {
                    kind: "sqlite".to_owned(),
                    reference: "evidence:tool-discovery-snapshot".to_owned(),
                }],
                next_actions: Vec::new(),
                redactions: matrix_core::Redactions {
                    profile: "v1".to_owned(),
                    fields: vec![
                        "secrets".to_owned(),
                        "absolute_paths".to_owned(),
                        "raw_stdout".to_owned(),
                        "raw_stderr".to_owned(),
                    ],
                },
                extensions: serde_json::json!({
                    "network_requested": false,
                    "network_activity_observed": "not_captured",
                    "external_processes": "allowlisted_user_triggered_only",
                    "external_agent_processes": false,
                    "existing_sessions_read": false,
                }),
            }
        }
        Err(error) => matrix_core::blocked_command_response(
            "tools.discover",
            error.code(),
            "工具发现未完成，本机事实源未接受不完整快照",
        ),
    }
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            doctor,
            offline_demo,
            group_templates,
            create_group,
            discover_tools
        ])
        .run(tauri::generate_context!())
        .expect("error while running JZMatrix Workbench");
}
