use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};
use matrix_core::{
    CliResponse, CreateGroupInput, EvidenceRef, Outcome, Redactions, ResponseStatus,
};
use serde_json::json;

#[derive(Debug, Parser)]
#[command(
    name = "jzmatrix",
    version,
    about = "JZMatrix Workbench offline-first CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Check the local product runtime without contacting a network.
    Doctor {
        /// Emit exactly one versioned JSON response on stdout.
        #[arg(long)]
        json: bool,
    },
    /// Inspect one of the bundled synthetic platform fixtures without filesystem or process access.
    Fixture {
        #[command(subcommand)]
        command: FixtureCommands,
    },
    /// Manage local-only collaboration groups without starting external tools.
    Group {
        #[command(subcommand)]
        command: GroupCommands,
    },
}

#[derive(Debug, Subcommand)]
enum FixtureCommands {
    /// Parse a built-in fixture and print its evidence summary.
    Inspect {
        /// Built-in fixture id; arbitrary paths are not accepted.
        #[arg(long)]
        id: String,
        /// Emit exactly one versioned JSON response on stdout.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum GroupCommands {
    /// List the built-in collaboration templates.
    Templates {
        /// Emit exactly one versioned JSON response on stdout.
        #[arg(long)]
        json: bool,
    },
    /// Create or replay a local collaboration group.
    Create {
        /// One-sentence collaboration goal.
        #[arg(long)]
        goal: String,
        /// Built-in template id.
        #[arg(long, default_value = "compact")]
        template: String,
        /// Stable caller key. Reusing it with the same input is idempotent.
        #[arg(long)]
        idempotency_key: String,
        /// Emit exactly one versioned JSON response on stdout.
        #[arg(long)]
        json: bool,
    },
    /// Read one local collaboration group by id.
    Show {
        /// Local collaboration group id.
        #[arg(long)]
        id: String,
        /// Emit exactly one versioned JSON response on stdout.
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        let mut command = Cli::command();
        let _ = command.print_help();
        eprintln!();
        return ExitCode::from(3);
    };

    match command {
        Commands::Doctor { json } => {
            let response = match matrix_core::default_app_data_dir() {
                Some(path) => matrix_core::run_doctor(&path),
                None => {
                    matrix_core::blocked_response("app_data_unavailable", "无法解析应用数据目录")
                }
            };
            if json {
                match serde_json::to_string(&response) {
                    Ok(serialized) => println!("{serialized}"),
                    Err(_) => return ExitCode::from(4),
                }
            } else {
                println!("jzmatrix doctor: {:?}", &response.status);
            }
            ExitCode::from(response.exit_code() as u8)
        }
        Commands::Fixture {
            command:
                FixtureCommands::Inspect {
                    id,
                    json: emit_json,
                },
        } => {
            let response = fixture_inspection_response(&id);
            if emit_json {
                match serde_json::to_string(&response) {
                    Ok(serialized) => println!("{serialized}"),
                    Err(_) => return ExitCode::from(4),
                }
            } else if response.ok {
                println!("jzmatrix fixture inspect: pass");
            } else {
                println!("jzmatrix fixture inspect: blocked");
            }
            ExitCode::from(response.exit_code() as u8)
        }
        Commands::Group { command } => match command {
            GroupCommands::Templates { json: emit_json } => {
                let response = group_templates_response();
                print_group_response(&response, emit_json, "jzmatrix group templates: pass");
                ExitCode::from(response.exit_code() as u8)
            }
            GroupCommands::Create {
                goal,
                template,
                idempotency_key,
                json: emit_json,
            } => {
                let response = group_create_response(&goal, &template, &idempotency_key);
                print_group_response(&response, emit_json, "jzmatrix group create: pass");
                ExitCode::from(response.exit_code() as u8)
            }
            GroupCommands::Show {
                id,
                json: emit_json,
            } => {
                let response = group_show_response(&id);
                print_group_response(&response, emit_json, "jzmatrix group show: pass");
                ExitCode::from(response.exit_code() as u8)
            }
        },
    }
}

fn print_group_response(response: &CliResponse, emit_json: bool, success_label: &str) {
    if emit_json {
        match serde_json::to_string(response) {
            Ok(serialized) => println!("{serialized}"),
            Err(_) => println!("{}", serde_json::json!({"ok": false, "status": "blocked"})),
        }
    } else if response.ok {
        println!("{success_label}");
    } else {
        println!("jzmatrix group: blocked");
    }
}

fn group_templates_response() -> CliResponse {
    CliResponse {
        contract: "jzmatrix.cli-response".to_owned(),
        version: "1.0.0".to_owned(),
        command: "group.templates".to_owned(),
        request_id: matrix_core::new_request_id(),
        ok: true,
        status: ResponseStatus::Pass,
        outcome: Outcome::NotStarted,
        data: json!({
            "templates": matrix_core::builtin_group_templates(),
            "source": "built_in_fixture",
            "local_only": true,
        }),
        errors: Vec::new(),
        warnings: vec![matrix_core::WarningItem {
            code: "local_only".to_owned(),
            message: "模板只用于本地建组，不会启动或连接外部工具".to_owned(),
        }],
        evidence: vec![EvidenceRef {
            kind: "fixture_manifest".to_owned(),
            reference: "fixture:collaboration-group-templates-v1".to_owned(),
        }],
        next_actions: Vec::new(),
        redactions: Redactions {
            profile: "v1".to_owned(),
            fields: vec!["secrets".to_owned(), "absolute_paths".to_owned()],
        },
        extensions: json!({"network": false, "external_processes": false}),
    }
}

fn group_create_response(goal: &str, template: &str, idempotency_key: &str) -> CliResponse {
    let Some(app_data_dir) = matrix_core::default_app_data_dir() else {
        return matrix_core::blocked_command_response(
            "group.create",
            "app_data_unavailable",
            "无法解析应用数据目录",
        );
    };
    match matrix_core::create_collaboration_group(
        &app_data_dir.join("db/app.sqlite3"),
        CreateGroupInput {
            goal,
            template_id: template,
            idempotency_key,
        },
    ) {
        Ok(group) => {
            let replayed = group.replayed;
            CliResponse {
                contract: "jzmatrix.cli-response".to_owned(),
                version: "1.0.0".to_owned(),
                command: "group.create".to_owned(),
                request_id: matrix_core::new_request_id(),
                ok: true,
                status: ResponseStatus::Pass,
                outcome: Outcome::Committed,
                data: serde_json::to_value(group).unwrap_or_else(|_| json!({})),
                errors: Vec::new(),
                warnings: vec![matrix_core::WarningItem {
                    code: "local_only".to_owned(),
                    message: "仅写入本机事实源，不创建、启动或发送到外部工具".to_owned(),
                }],
                evidence: vec![EvidenceRef {
                    kind: "sqlite".to_owned(),
                    reference: "evidence:local-group".to_owned(),
                }],
                next_actions: Vec::new(),
                redactions: Redactions {
                    profile: "v1".to_owned(),
                    fields: vec!["secrets".to_owned(), "absolute_paths".to_owned()],
                },
                extensions: json!({
                    "network": false,
                    "external_processes": false,
                    "idempotent": true,
                    "replayed": replayed,
                }),
            }
        }
        Err(error) => matrix_core::blocked_command_response(
            "group.create",
            error.code(),
            group_error_message(error.code()),
        ),
    }
}

fn group_show_response(id: &str) -> CliResponse {
    let Some(app_data_dir) = matrix_core::default_app_data_dir() else {
        return matrix_core::blocked_command_response(
            "group.show",
            "app_data_unavailable",
            "无法解析应用数据目录",
        );
    };
    match matrix_core::load_collaboration_group(&app_data_dir.join("db/app.sqlite3"), id) {
        Ok(group) => CliResponse {
            contract: "jzmatrix.cli-response".to_owned(),
            version: "1.0.0".to_owned(),
            command: "group.show".to_owned(),
            request_id: matrix_core::new_request_id(),
            ok: true,
            status: ResponseStatus::Pass,
            outcome: Outcome::NotStarted,
            data: serde_json::to_value(group).unwrap_or_else(|_| json!({})),
            errors: Vec::new(),
            warnings: vec![matrix_core::WarningItem {
                code: "local_only".to_owned(),
                message: "只读取本机事实源，不连接外部工具".to_owned(),
            }],
            evidence: vec![EvidenceRef {
                kind: "sqlite".to_owned(),
                reference: "evidence:local-group".to_owned(),
            }],
            next_actions: Vec::new(),
            redactions: Redactions {
                profile: "v1".to_owned(),
                fields: vec!["secrets".to_owned(), "absolute_paths".to_owned()],
            },
            extensions: json!({"network": false, "external_processes": false}),
        },
        Err(error) => matrix_core::blocked_command_response(
            "group.show",
            error.code(),
            group_error_message(error.code()),
        ),
    }
}

fn group_error_message(code: &str) -> &'static str {
    match code {
        "invalid_group_input" => "目标、模板或幂等键不符合本地建组输入约束",
        "group_template_not_found" => "模板不在随包白名单中",
        "idempotency_conflict" => "幂等键已用于另一组输入，未修改本机事实源",
        "group_not_found" => "本机事实源中没有这个协作组",
        "database_integrity_failed" => "本机事实源完整性检查未通过，未继续写入",
        _ => "本地协作组操作未完成",
    }
}

fn fixture_inspection_response(id: &str) -> CliResponse {
    match matrix_core::inspect_builtin_platform_fixture(id) {
        Ok(inspection) => CliResponse {
            contract: "jzmatrix.cli-response".to_owned(),
            version: "1.0.0".to_owned(),
            command: "fixture.inspect".to_owned(),
            request_id: matrix_core::new_request_id(),
            ok: true,
            status: ResponseStatus::Pass,
            outcome: Outcome::NotCommitted,
            data: serde_json::to_value(inspection).unwrap_or_else(|_| json!({})),
            errors: Vec::new(),
            warnings: vec![matrix_core::WarningItem {
                code: "synthetic_fixture_only".to_owned(),
                message: "仅解析随包合成 fixture，不代表真实平台协议或适配器已接入".to_owned(),
            }],
            evidence: vec![
                EvidenceRef {
                    kind: "fixture_manifest".to_owned(),
                    reference: "fixture:platform-events/manifest.json".to_owned(),
                },
                EvidenceRef {
                    kind: "fixture".to_owned(),
                    reference: format!("fixture:{id}"),
                },
            ],
            next_actions: Vec::new(),
            redactions: Redactions {
                profile: "v1".to_owned(),
                fields: vec![
                    "payload".to_owned(),
                    "transcript".to_owned(),
                    "secrets".to_owned(),
                    "absolute_paths".to_owned(),
                ],
            },
            extensions: json!({
                "offline": true,
                "external_processes": false,
                "arbitrary_paths": false,
                "real_sessions": false,
                "supported_fixture_ids": matrix_core::builtin_platform_fixture_ids(),
            }),
        },
        Err(error) => matrix_core::blocked_command_response(
            "fixture.inspect",
            error.code(),
            fixture_error_message(error.code()),
        ),
    }
}

fn fixture_error_message(code: &str) -> &'static str {
    match code {
        "fixture_not_found" => "请求的 fixture 不在内置清单中",
        "fixture_integrity_failed" => "内置 fixture 清单或摘要校验未通过",
        "fixture_sensitive_field" | "fixture_private_value" => {
            "fixture 含秘密、会话正文、远程 URL 或绝对路径，解析已关闭"
        }
        "fixture_event_conflict" => "fixture 事件存在冲突，解析已关闭",
        "fixture_schema_unsupported" => "fixture 版本或结构不受当前解析器支持",
        _ => "fixture 解析未完成",
    }
}
