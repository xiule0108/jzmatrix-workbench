use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{ensure_schema, now_utc, sha256_hex, CoreError};

pub const MIGRATION_SQL: &str = include_str!("../migrations/0003_tool_discovery.sql");
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(2);
const OUTPUT_LIMIT: usize = 16 * 1024;
const SAFE_SYSTEM_PATH: &str = "/usr/bin:/bin:/usr/sbin:/sbin:/usr/local/bin:/opt/homebrew/bin";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCatalogEntry {
    pub id: String,
    pub label: String,
    pub binary: String,
    pub description: String,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDiscoveryResult {
    pub id: String,
    pub label: String,
    pub binary: String,
    pub status: String,
    pub version: Option<String>,
    pub version_exit_code: Option<i32>,
    pub help_status: String,
    pub help_exit_code: Option<i32>,
    pub advertised_flags: Vec<String>,
    pub output_sha256: Option<String>,
    pub observed_at: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDiscoverySnapshot {
    pub contract: String,
    pub version: String,
    pub id: String,
    pub observed_at: String,
    pub data_source: String,
    pub invocation: String,
    pub network_requested: bool,
    pub existing_sessions_read: bool,
    pub external_writes: bool,
    pub tools: Vec<ToolDiscoveryResult>,
    pub persisted: bool,
}

#[derive(Clone, Copy)]
struct ToolSpec {
    id: &'static str,
    label: &'static str,
    binary: &'static str,
    description: &'static str,
    scope: &'static str,
    advertised_flags: &'static [&'static str],
}

const COMMON_FLAGS: &[&str] = &["--version", "--help"];
const CODEX_FLAGS: &[&str] = &["--json", "--resume", "exec", "app-server", "mcp-server"];
const CLAUDE_FLAGS: &[&str] = &[
    "-p",
    "--output-format",
    "--resume",
    "--continue",
    "--fork-session",
];
const CURSOR_FLAGS: &[&str] = &["--print", "--output-format", "--resume", "stream-json"];
const COPILOT_FLAGS: &[&str] = &["--json", "--continue", "--acp", "session"];
const ACP_FLAGS: &[&str] = &["--acp"];

const ALLOWED_TOOL_SPECS: &[ToolSpec] = &[
    ToolSpec {
        id: "codex_cli",
        label: "Codex CLI",
        binary: "codex",
        description: "OpenAI 的本地编码助手命令行",
        scope: "只检查可执行文件和无副作用帮助信息，不读取会话",
        advertised_flags: CODEX_FLAGS,
    },
    ToolSpec {
        id: "claude_code_cli",
        label: "Claude Code",
        binary: "claude",
        description: "Anthropic 的本地编码助手命令行",
        scope: "只检查可执行文件和无副作用帮助信息，不读取会话",
        advertised_flags: CLAUDE_FLAGS,
    },
    ToolSpec {
        id: "cursor_agent",
        label: "Cursor Agent",
        binary: "cursor-agent",
        description: "Cursor 的命令行 Agent 入口",
        scope: "只检查本机 CLI，不连接后台任务",
        advertised_flags: CURSOR_FLAGS,
    },
    ToolSpec {
        id: "copilot_cli",
        label: "GitHub Copilot CLI",
        binary: "copilot",
        description: "GitHub Copilot 的命令行入口",
        scope: "只检查本机 CLI，不读取远程会话",
        advertised_flags: COPILOT_FLAGS,
    },
    ToolSpec {
        id: "zed",
        label: "Zed",
        binary: "zed",
        description: "Zed 编辑器命令行入口",
        scope: "只检查命令行入口，不打开编辑器或线程",
        advertised_flags: COMMON_FLAGS,
    },
    ToolSpec {
        id: "zcode",
        label: "ZCode",
        binary: "zcode",
        description: "Z.AI ZCode 桌面入口",
        scope: "只检查命令行入口，不读取桌面会话",
        advertised_flags: COMMON_FLAGS,
    },
    ToolSpec {
        id: "vscode",
        label: "VS Code",
        binary: "code",
        description: "VS Code 命令行入口",
        scope: "只检查编辑器 CLI，不读取 Copilot 会话",
        advertised_flags: COMMON_FLAGS,
    },
    ToolSpec {
        id: "opencode",
        label: "OpenCode",
        binary: "opencode",
        description: "OpenCode 命令行与 ACP 样本",
        scope: "只检查 CLI，不启动 HTTP 或 ACP 服务",
        advertised_flags: &["acp", "serve", "session", "--format"],
    },
    ToolSpec {
        id: "cline",
        label: "Cline",
        binary: "cline",
        description: "Cline 命令行与 ACP 样本",
        scope: "只检查 CLI，不创建任务或 ACP 会话",
        advertised_flags: ACP_FLAGS,
    },
    ToolSpec {
        id: "aider",
        label: "Aider",
        binary: "aider",
        description: "Aider 命令行对照工具",
        scope: "只检查 CLI，不读取历史或修改 Git",
        advertised_flags: &["--message", "--stream", "--yes-always"],
    },
];

#[derive(Debug)]
enum ProbeOutcome {
    Completed { exit_code: i32, bytes: Vec<u8> },
    TimedOut,
    Failed(&'static str),
}

pub fn builtin_tool_catalog() -> Vec<ToolCatalogEntry> {
    ALLOWED_TOOL_SPECS
        .iter()
        .map(|spec| ToolCatalogEntry {
            id: spec.id.to_owned(),
            label: spec.label.to_owned(),
            binary: spec.binary.to_owned(),
            description: spec.description.to_owned(),
            scope: spec.scope.to_owned(),
        })
        .collect()
}

pub fn discover_local_tools(db_path: &Path) -> Result<ToolDiscoverySnapshot, CoreError> {
    discover_local_tools_from_path(db_path, env::var_os("PATH"))
}

fn discover_local_tools_from_path(
    db_path: &Path,
    path: Option<OsString>,
) -> Result<ToolDiscoverySnapshot, CoreError> {
    let observed_at = now_utc();
    let tools = ALLOWED_TOOL_SPECS
        .iter()
        .map(|spec| probe_tool(*spec, &observed_at, path.as_ref()))
        .collect::<Vec<_>>();
    let mut snapshot = ToolDiscoverySnapshot {
        contract: "jzmatrix.tool-discovery".to_owned(),
        version: "1.0.0".to_owned(),
        id: Uuid::now_v7().to_string(),
        observed_at,
        data_source: "real".to_owned(),
        invocation: "user_triggered".to_owned(),
        network_requested: false,
        existing_sessions_read: false,
        external_writes: false,
        tools,
        persisted: false,
    };

    ensure_schema(db_path)?;
    let connection = Connection::open(db_path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "busy_timeout", 3_000_i64)?;
    snapshot.persisted = true;
    let serialized = serde_json::to_string(&snapshot)?;
    connection.execute(
        "INSERT INTO tool_discovery_snapshots
            (id, observed_at, data_source, invocation, snapshot_json)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            snapshot.id,
            snapshot.observed_at,
            snapshot.data_source,
            snapshot.invocation,
            serialized
        ],
    )?;
    Ok(snapshot)
}

fn probe_tool(
    spec: ToolSpec,
    observed_at: &str,
    search_path: Option<&OsString>,
) -> ToolDiscoveryResult {
    let Some(path) = resolve_binary(spec.binary, search_path) else {
        return ToolDiscoveryResult {
            id: spec.id.to_owned(),
            label: spec.label.to_owned(),
            binary: spec.binary.to_owned(),
            status: "not_found".to_owned(),
            version: None,
            version_exit_code: None,
            help_status: "not_run".to_owned(),
            help_exit_code: None,
            advertised_flags: Vec::new(),
            output_sha256: None,
            observed_at: observed_at.to_owned(),
            reason: Some("binary_not_in_path".to_owned()),
        };
    };

    let version_probe = run_probe(&path, "--version");
    let (status, version, version_exit_code, version_bytes, reason) = match version_probe {
        ProbeOutcome::Completed { exit_code, bytes } if exit_code == 0 => (
            "available".to_owned(),
            extract_version(&bytes),
            Some(exit_code),
            bytes,
            None,
        ),
        ProbeOutcome::Completed { exit_code, .. } => (
            "failed".to_owned(),
            None,
            Some(exit_code),
            Vec::new(),
            Some("version_command_nonzero".to_owned()),
        ),
        ProbeOutcome::TimedOut => (
            "timed_out".to_owned(),
            None,
            None,
            Vec::new(),
            Some("version_command_timeout".to_owned()),
        ),
        ProbeOutcome::Failed(reason) => (
            "failed".to_owned(),
            None,
            None,
            Vec::new(),
            Some(reason.to_owned()),
        ),
    };

    if status != "available" {
        return ToolDiscoveryResult {
            id: spec.id.to_owned(),
            label: spec.label.to_owned(),
            binary: spec.binary.to_owned(),
            status,
            version,
            version_exit_code,
            help_status: "not_run".to_owned(),
            help_exit_code: None,
            advertised_flags: Vec::new(),
            output_sha256: None,
            observed_at: observed_at.to_owned(),
            reason,
        };
    }

    let help_probe = run_probe(&path, "--help");
    let (help_status, help_exit_code, help_bytes, help_reason) = match help_probe {
        ProbeOutcome::Completed { exit_code, bytes } if exit_code == 0 => {
            ("pass".to_owned(), Some(exit_code), bytes, None)
        }
        ProbeOutcome::Completed { exit_code, .. } => (
            "failed".to_owned(),
            Some(exit_code),
            Vec::new(),
            Some("help_command_nonzero".to_owned()),
        ),
        ProbeOutcome::TimedOut => (
            "timed_out".to_owned(),
            None,
            Vec::new(),
            Some("help_command_timeout".to_owned()),
        ),
        ProbeOutcome::Failed(reason) => (
            "failed".to_owned(),
            None,
            Vec::new(),
            Some(reason.to_owned()),
        ),
    };
    let advertised_flags = spec
        .advertised_flags
        .iter()
        .filter(|flag| contains_ascii(&help_bytes, flag))
        .map(|flag| (*flag).to_owned())
        .collect();
    ToolDiscoveryResult {
        id: spec.id.to_owned(),
        label: spec.label.to_owned(),
        binary: spec.binary.to_owned(),
        status: "available".to_owned(),
        version,
        version_exit_code,
        help_status,
        help_exit_code,
        advertised_flags,
        output_sha256: Some(sha256_hex(&[version_bytes, help_bytes].concat())),
        observed_at: observed_at.to_owned(),
        reason: help_reason.or(reason),
    }
}

fn resolve_binary(binary: &str, search_path: Option<&OsString>) -> Option<PathBuf> {
    let path = search_path?;
    env::split_paths(&path)
        .filter(|entry| entry.is_absolute())
        .map(|entry| entry.join(binary))
        .find(|candidate| candidate.is_file())
}

fn run_probe(path: &Path, argument: &str) -> ProbeOutcome {
    if !matches!(argument, "--version" | "--help") {
        return ProbeOutcome::Failed("argument_not_allowlisted");
    }
    let mut command = Command::new(path);
    command
        .arg(argument)
        .current_dir(env::temp_dir())
        .env_clear()
        .env(
            "PATH",
            format!(
                "{}:{SAFE_SYSTEM_PATH}",
                path.parent().unwrap_or_else(|| Path::new("/")).display()
            ),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            return ProbeOutcome::Failed(match error.kind() {
                std::io::ErrorKind::PermissionDenied => "permission_denied",
                std::io::ErrorKind::NotFound => "binary_not_found",
                _ => "spawn_failed",
            })
        }
    };
    let deadline = Instant::now() + DISCOVERY_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return ProbeOutcome::TimedOut;
            }
            Ok(None) => sleep(Duration::from_millis(10)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return ProbeOutcome::Failed("wait_failed");
            }
        }
    }
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(_) => return ProbeOutcome::Failed("output_read_failed"),
    };
    let mut bytes =
        Vec::with_capacity((output.stdout.len() + output.stderr.len()).min(OUTPUT_LIMIT));
    bytes.extend_from_slice(&output.stdout[..output.stdout.len().min(OUTPUT_LIMIT)]);
    if bytes.len() < OUTPUT_LIMIT {
        bytes.extend_from_slice(
            &output.stderr[..(OUTPUT_LIMIT - bytes.len()).min(output.stderr.len())],
        );
    }
    ProbeOutcome::Completed {
        exit_code: output.status.code().unwrap_or(1),
        bytes,
    }
}

fn extract_version(bytes: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(bytes);
    text.split_whitespace().find_map(|token| {
        let candidate = token.trim_matches(|character: char| {
            !character.is_ascii_alphanumeric()
                && character != '.'
                && character != '-'
                && character != '_'
        });
        let mut numeric_parts = candidate.split('.');
        let first = numeric_parts.next()?;
        let second = numeric_parts.next()?;
        let third = numeric_parts.next()?;
        if first.chars().all(|character| character.is_ascii_digit())
            && second.chars().all(|character| character.is_ascii_digit())
            && third
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_digit())
        {
            Some(candidate.to_owned())
        } else {
            None
        }
    })
}

fn contains_ascii(bytes: &[u8], needle: &str) -> bool {
    String::from_utf8_lossy(bytes).contains(needle)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use rusqlite::Connection;

    use super::{builtin_tool_catalog, discover_local_tools_from_path, ALLOWED_TOOL_SPECS};

    #[test]
    fn allowlist_is_fixed_and_does_not_accept_paths() {
        assert_eq!(ALLOWED_TOOL_SPECS.len(), 10);
        assert!(ALLOWED_TOOL_SPECS.iter().all(|spec| {
            !spec.binary.contains('/') && !spec.binary.contains('\\') && !spec.binary.contains(".")
        }));
        assert!(builtin_tool_catalog()
            .iter()
            .any(|tool| tool.id == "codex_cli"));
        assert!(builtin_tool_catalog()
            .iter()
            .any(|tool| tool.id == "claude_code_cli"));
    }

    #[test]
    fn empty_path_records_only_not_found_facts_without_raw_output() {
        let directory = tempfile::tempdir().expect("temporary app data directory");
        let db = directory.path().join("db/app.sqlite3");
        let snapshot = discover_local_tools_from_path(&db, Some(OsString::from("")))
            .expect("persist empty-path snapshot");
        assert!(snapshot.persisted);
        assert_eq!(snapshot.tools.len(), 10);
        assert!(snapshot.tools.iter().all(|tool| tool.status == "not_found"));
        let connection = Connection::open(db).expect("open discovery database");
        let stored: String = connection
            .query_row(
                "SELECT snapshot_json FROM tool_discovery_snapshots WHERE id = ?1",
                [&snapshot.id],
                |row| row.get(0),
            )
            .expect("stored snapshot");
        assert!(!stored.contains(&format!("/{}/", "Users")));
        assert!(!stored.contains("raw_stdout"));
    }
}
