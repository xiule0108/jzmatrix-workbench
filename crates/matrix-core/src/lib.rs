use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const INITIAL_MIGRATION_SQL: &str = include_str!("../migrations/0001_initial.sql");
const OFFLINE_DEMO_MANIFEST: &str = include_str!("../../../fixtures/offline-demo/manifest.json");
const OFFLINE_DEMO_JSON: &str = include_str!("../../../fixtures/offline-demo/offline-demo.json");

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("filesystem operation failed")]
    Io(#[from] std::io::Error),
    #[error("sqlite operation failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("fixture JSON is invalid")]
    Json(#[from] serde_json::Error),
    #[error("offline fixture integrity check failed")]
    FixtureIntegrity,
    #[error("database integrity check failed")]
    DatabaseIntegrity,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureManifest {
    pub contract: String,
    pub version: String,
    pub fixture: String,
    pub sha256: String,
    pub data_source: String,
    pub network_required: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseStatus {
    Pass,
    Degraded,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    NotStarted,
    Committed,
    NotCommitted,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Degraded,
    Blocked,
    Unknown,
    NotRun,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceRef {
    pub kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Remediation {
    pub action_id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorCheck {
    pub id: String,
    pub category: String,
    pub status: CheckStatus,
    pub required: bool,
    pub severity: String,
    pub observed_at: String,
    pub evidence: Vec<EvidenceRef>,
    pub remediation: Vec<Remediation>,
    pub exit_impact: u8,
    pub reason_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorSummary {
    pub pass: usize,
    pub degraded: usize,
    pub blocked: usize,
    pub unknown: usize,
    pub not_run: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DoctorData {
    pub doctor_version: String,
    pub observed_at: String,
    pub checks: Vec<DoctorCheck>,
    pub summary: DoctorSummary,
    pub overall_status: ResponseStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorItem {
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub outcome: Outcome,
    pub details_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WarningItem {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct NextAction {
    pub action_id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Redactions {
    pub profile: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CliResponse {
    pub contract: String,
    pub version: String,
    pub command: String,
    pub request_id: String,
    pub ok: bool,
    pub status: ResponseStatus,
    pub outcome: Outcome,
    pub data: Value,
    pub errors: Vec<ErrorItem>,
    pub warnings: Vec<WarningItem>,
    pub evidence: Vec<EvidenceRef>,
    pub next_actions: Vec<NextAction>,
    pub redactions: Redactions,
    pub extensions: Value,
}

impl CliResponse {
    pub fn exit_code(&self) -> i32 {
        match &self.status {
            ResponseStatus::Pass => 0,
            ResponseStatus::Degraded => 1,
            ResponseStatus::Blocked | ResponseStatus::Unknown => 2,
        }
    }
}

pub fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn offline_demo() -> Result<Value, CoreError> {
    let manifest: FixtureManifest = serde_json::from_str(OFFLINE_DEMO_MANIFEST)?;
    let actual = sha256_hex(OFFLINE_DEMO_JSON.as_bytes());
    if manifest.contract != "jzmatrix.offline-demo-manifest"
        || manifest.version != "1.0.0"
        || manifest.fixture != "offline-demo.json"
        || manifest.data_source != "demo"
        || manifest.network_required
        || manifest.sha256 != actual
    {
        return Err(CoreError::FixtureIntegrity);
    }
    Ok(serde_json::from_str(OFFLINE_DEMO_JSON)?)
}

pub fn ensure_schema(db_path: &Path) -> Result<(), CoreError> {
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut connection = Connection::open(db_path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "busy_timeout", 3_000_i64)?;
    connection.execute_batch("PRAGMA journal_mode=WAL;")?;

    let transaction = connection.transaction()?;
    transaction.execute_batch(INITIAL_MIGRATION_SQL)?;
    transaction.execute(
        "INSERT OR IGNORE INTO schema_migrations (version, name, checksum_sha256, applied_at, status) VALUES (?1, ?2, ?3, ?4, 'committed')",
        params![1_i64, "0001_initial", sha256_hex(INITIAL_MIGRATION_SQL.as_bytes()), now_utc()],
    )?;
    transaction.commit()?;

    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    let mut foreign_key_statement = connection.prepare("PRAGMA foreign_key_check")?;
    let mut foreign_key_rows = foreign_key_statement.query([])?;
    if integrity != "ok" || foreign_key_rows.next()?.is_some() {
        return Err(CoreError::DatabaseIntegrity);
    }
    Ok(())
}

pub fn default_app_data_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "JIEZIJIUWEI", "JZMatrix Workbench")
        .map(|dirs| dirs.data_dir().to_path_buf())
}

pub fn blocked_response(code: &str, message: &str) -> CliResponse {
    let observed_at = now_utc();
    CliResponse {
        contract: "jzmatrix.cli-response".to_owned(),
        version: "1.0.0".to_owned(),
        command: "doctor".to_owned(),
        request_id: Uuid::now_v7().to_string(),
        ok: false,
        status: ResponseStatus::Blocked,
        outcome: Outcome::NotCommitted,
        data: json!({
            "doctor_version": "1.0.0",
            "observed_at": observed_at,
            "checks": [],
            "summary": {"pass": 0, "degraded": 0, "blocked": 1, "unknown": 0, "not_run": 0},
            "overall_status": "blocked"
        }),
        errors: vec![ErrorItem {
            code: code.to_owned(),
            message: message.to_owned(),
            retryable: false,
            outcome: Outcome::NotCommitted,
            details_ref: None,
        }],
        warnings: Vec::new(),
        evidence: vec![EvidenceRef {
            kind: "local_check".to_owned(),
            reference: "evidence:doctor".to_owned(),
        }],
        next_actions: vec![NextAction {
            action_id: "inspect_environment".to_owned(),
            label: "检查产品运行环境".to_owned(),
        }],
        redactions: Redactions {
            profile: "v1".to_owned(),
            fields: Vec::new(),
        },
        extensions: json!({}),
    }
}

pub fn run_doctor(app_data_dir: &Path) -> CliResponse {
    let observed_at = now_utc();
    let mut checks = Vec::new();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    checks.push(check(
        "product.runtime",
        "product",
        CheckStatus::Pass,
        true,
        "info",
        &observed_at,
        "local_check",
        "product:runtime",
        Vec::new(),
        0,
        None,
    ));

    match offline_demo() {
        Ok(_) => checks.push(check(
            "offline_demo.resources",
            "offline_demo",
            CheckStatus::Pass,
            true,
            "info",
            &observed_at,
            "fixture",
            "fixture:offline-demo",
            Vec::new(),
            0,
            None,
        )),
        Err(_) => checks.push(check(
            "offline_demo.resources",
            "offline_demo",
            CheckStatus::Blocked,
            true,
            "error",
            &observed_at,
            "fixture",
            "fixture:offline-demo",
            vec![Remediation {
                action_id: "repair_resources".to_owned(),
                label: "重新安装产品资源".to_owned(),
            }],
            2,
            Some("resource_integrity_failed".to_owned()),
        )),
    }

    let data_dir_status = match writable_probe(app_data_dir) {
        Ok(()) => CheckStatus::Pass,
        Err(_) => CheckStatus::Blocked,
    };
    let data_dir_reason = if matches!(&data_dir_status, CheckStatus::Pass) {
        None
    } else {
        Some("app_data_unavailable".to_owned())
    };
    checks.push(check(
        "app_data.writable",
        "data_dir",
        data_dir_status,
        true,
        "error",
        &observed_at,
        "local_check",
        "path:app-data",
        vec![Remediation {
            action_id: "choose_app_data".to_owned(),
            label: "检查应用数据目录权限".to_owned(),
        }],
        2,
        data_dir_reason,
    ));

    let sqlite_status = match ensure_schema(&app_data_dir.join("db/app.sqlite3")) {
        Ok(()) => CheckStatus::Pass,
        Err(_) => CheckStatus::Blocked,
    };
    let sqlite_reason = if matches!(&sqlite_status, CheckStatus::Pass) {
        None
    } else {
        Some("db_integrity_failed".to_owned())
    };
    checks.push(check(
        "sqlite.integrity",
        "sqlite",
        sqlite_status,
        true,
        "error",
        &observed_at,
        "local_check",
        "evidence:sqlite-integrity",
        vec![Remediation {
            action_id: "restore_backup".to_owned(),
            label: "选择已验证的数据库备份恢复".to_owned(),
        }],
        2,
        sqlite_reason,
    ));

    checks.push(check(
        "agent.optional",
        "external_tool",
        CheckStatus::NotRun,
        false,
        "warning",
        &observed_at,
        "local_check",
        "capability:optional-agent",
        Vec::new(),
        0,
        Some("optional_capability_not_requested".to_owned()),
    ));
    warnings.push(WarningItem {
        code: "optional_agent_not_run".to_owned(),
        message: "未请求外部 Agent 探测，离线模式不受阻断".to_owned(),
    });

    checks.push(check(
        "secret_store.p1a",
        "secret_store",
        CheckStatus::NotRun,
        false,
        "warning",
        &observed_at,
        "local_check",
        "capability:secret-store",
        Vec::new(),
        0,
        Some("p1a_secret_store_probe_not_enabled".to_owned()),
    ));
    warnings.push(WarningItem {
        code: "secret_store_not_run".to_owned(),
        message: "P1-A 不读取或创建密钥".to_owned(),
    });

    let summary = summarize(&checks);
    let status = if summary.blocked > 0 {
        ResponseStatus::Blocked
    } else if summary.degraded > 0 {
        ResponseStatus::Degraded
    } else {
        ResponseStatus::Pass
    };
    if matches!(&status, ResponseStatus::Blocked) {
        for item in checks
            .iter()
            .filter(|item| matches!(item.status, CheckStatus::Blocked | CheckStatus::Unknown))
        {
            errors.push(ErrorItem {
                code: item
                    .reason_code
                    .clone()
                    .unwrap_or_else(|| "doctor_blocked".to_owned()),
                message: format!("检查未通过: {}", item.id),
                retryable: false,
                outcome: Outcome::NotCommitted,
                details_ref: None,
            });
        }
    }
    let ok = !matches!(&status, ResponseStatus::Blocked | ResponseStatus::Unknown);
    CliResponse {
        contract: "jzmatrix.cli-response".to_owned(),
        version: "1.0.0".to_owned(),
        command: "doctor".to_owned(),
        request_id: Uuid::now_v7().to_string(),
        ok,
        status: status.clone(),
        outcome: if ok {
            Outcome::Committed
        } else {
            Outcome::NotCommitted
        },
        data: serde_json::to_value(DoctorData {
            doctor_version: "1.0.0".to_owned(),
            observed_at: observed_at.clone(),
            checks,
            summary,
            overall_status: status.clone(),
        })
        .unwrap_or_else(|_| json!({})),
        errors,
        warnings,
        evidence: vec![EvidenceRef {
            kind: "local_check".to_owned(),
            reference: "evidence:doctor".to_owned(),
        }],
        next_actions: Vec::new(),
        redactions: Redactions {
            profile: "v1".to_owned(),
            fields: Vec::new(),
        },
        extensions: json!({"network": "disabled_by_default", "daemon_mode": false}),
    }
}

fn writable_probe(app_data_dir: &Path) -> Result<(), CoreError> {
    fs::create_dir_all(app_data_dir)?;
    let probe = app_data_dir.join(format!(".doctor-write-probe-{}", Uuid::now_v7()));
    fs::write(&probe, b"jzmatrix doctor")?;
    fs::remove_file(probe)?;
    Ok(())
}

fn check(
    id: &str,
    category: &str,
    status: CheckStatus,
    required: bool,
    severity: &str,
    observed_at: &str,
    evidence_kind: &str,
    evidence_ref: &str,
    remediation: Vec<Remediation>,
    exit_impact: u8,
    reason_code: Option<String>,
) -> DoctorCheck {
    DoctorCheck {
        id: id.to_owned(),
        category: category.to_owned(),
        status,
        required,
        severity: severity.to_owned(),
        observed_at: observed_at.to_owned(),
        evidence: vec![EvidenceRef {
            kind: evidence_kind.to_owned(),
            reference: evidence_ref.to_owned(),
        }],
        remediation,
        exit_impact,
        reason_code,
    }
}

fn summarize(checks: &[DoctorCheck]) -> DoctorSummary {
    let mut summary = DoctorSummary {
        pass: 0,
        degraded: 0,
        blocked: 0,
        unknown: 0,
        not_run: 0,
    };
    for item in checks {
        match item.status {
            CheckStatus::Pass => summary.pass += 1,
            CheckStatus::Degraded => summary.degraded += 1,
            CheckStatus::Blocked => summary.blocked += 1,
            CheckStatus::Unknown => summary.unknown += 1,
            CheckStatus::NotRun => summary.not_run += 1,
        }
    }
    summary
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_fixture_is_verified_without_network() {
        let demo = offline_demo().expect("fixture must be valid");
        assert_eq!(demo["data_source"], "demo");
        assert_eq!(demo["groups"][0]["events"][1]["status"], "not_run");
    }

    #[test]
    fn doctor_keeps_optional_capabilities_non_blocking() {
        let directory = tempfile::tempdir().expect("temporary app data directory");
        let response = run_doctor(directory.path());
        assert!(response.ok);
        assert_eq!(response.exit_code(), 0);
        let data = response.data["checks"].as_array().expect("checks array");
        let agent = data
            .iter()
            .find(|item| item["id"] == "agent.optional")
            .expect("agent check");
        assert_eq!(agent["status"], "not_run");
        assert_eq!(response.data["overall_status"], "pass");
    }

    #[test]
    fn sqlite_schema_is_idempotent() {
        let directory = tempfile::tempdir().expect("temporary app data directory");
        let db = directory.path().join("db/app.sqlite3");
        ensure_schema(&db).expect("first schema application");
        ensure_schema(&db).expect("second schema application");
        let connection = Connection::open(db).expect("open database");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count");
        assert_eq!(count, 1);
    }
}
