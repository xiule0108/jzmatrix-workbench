use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use chrono::{SecondsFormat, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

const INITIAL_MIGRATION_SQL: &str = include_str!("../migrations/0001_initial.sql");
const OFFLINE_DEMO_MANIFEST: &str = include_str!("../../../fixtures/offline-demo/manifest.json");
const OFFLINE_DEMO_JSON: &str = include_str!("../../../fixtures/offline-demo/offline-demo.json");
const PLATFORM_FIXTURE_MANIFEST: &str =
    include_str!("../../../fixtures/platform-events/manifest.json");
const CODEX_PLATFORM_FIXTURE: &str =
    include_str!("../../../fixtures/platform-events/codex-cli-v1.json");
const CLAUDE_PLATFORM_FIXTURE: &str =
    include_str!("../../../fixtures/platform-events/claude-code-v1.json");

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
    #[error("built-in platform fixture was not found")]
    FixtureNotFound,
    #[error("platform fixture was blocked by its safety contract")]
    FixtureBlocked(&'static str),
    #[error("platform fixture schema is unsupported")]
    FixtureSchemaUnsupported,
    #[error("platform fixture event sequence conflicts")]
    FixtureEventConflict,
    #[error("database integrity check failed")]
    DatabaseIntegrity,
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "io_error",
            Self::Sqlite(_) => "sqlite_error",
            Self::Json(_) => "invalid_json",
            Self::FixtureIntegrity => "fixture_integrity_failed",
            Self::FixtureNotFound => "fixture_not_found",
            Self::FixtureBlocked(code) => code,
            Self::FixtureSchemaUnsupported => "fixture_schema_unsupported",
            Self::FixtureEventConflict => "fixture_event_conflict",
            Self::DatabaseIntegrity => "database_integrity_failed",
        }
    }
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

pub fn new_request_id() -> String {
    Uuid::now_v7().to_string()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut hex, "{byte:02x}").expect("writing to a string cannot fail");
    }
    hex
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

const PLATFORM_FIXTURE_PARSER_VERSION: &str = "1.0.0";
const PLATFORM_FIXTURE_CONTRACT: &str = "jzmatrix.synthetic-platform-events";
const PLATFORM_FIXTURE_PROTOCOL_COVERAGE: &str = "observed_structured_event_categories_only";

#[derive(Debug, Clone, Deserialize)]
struct PlatformFixtureManifest {
    contract: String,
    version: String,
    parser_version: String,
    synthetic: bool,
    network_required: bool,
    source_observation: String,
    protocol_coverage: String,
    fixtures: Vec<PlatformFixtureManifestEntry>,
}

#[derive(Debug, Clone, Deserialize)]
struct PlatformFixtureManifestEntry {
    id: String,
    platform: String,
    file: String,
    sha256: String,
    observed_event_kinds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureParseStatus {
    Parsed,
    Partial,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FixtureFactState {
    Observed,
    NotObserved,
    Unknown,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureFactObservation {
    pub state: FixtureFactState,
    pub evidence_event_ids: Vec<String>,
}

impl Default for FixtureFactObservation {
    fn default() -> Self {
        Self {
            state: FixtureFactState::Unknown,
            evidence_event_ids: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct FixtureFacts {
    pub activity: FixtureFactObservation,
    pub local_written: FixtureFactObservation,
    pub sent_not_confirmed: FixtureFactObservation,
    pub delivered: FixtureFactObservation,
    pub accepted: FixtureFactObservation,
    pub completed: FixtureFactObservation,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureEventCursor {
    pub first_sequence: Option<u64>,
    pub last_sequence: Option<u64>,
    pub next_expected_sequence: u64,
    pub has_gap: bool,
    pub out_of_order: bool,
    pub duplicate_events: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureEventSummary {
    pub event_id: String,
    pub run_id: String,
    pub sequence: u64,
    pub observed_at: String,
    pub event_kind: String,
    pub payload_sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureEvidenceSummary {
    pub source: String,
    pub event_count: usize,
    pub unique_event_count: usize,
    pub event_kinds: Vec<String>,
    pub persisted_raw_payload: bool,
    pub persisted_transcript: bool,
    pub persisted_secrets: bool,
    pub persisted_absolute_paths: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixtureInspection {
    pub contract: String,
    pub version: String,
    pub parser_version: String,
    pub fixture_id: String,
    pub platform: String,
    pub synthetic: bool,
    pub source_observation: String,
    pub protocol_coverage: String,
    pub parse_status: FixtureParseStatus,
    pub manifest_sha256: String,
    pub fixture_sha256: String,
    pub event_cursor: FixtureEventCursor,
    pub events: Vec<FixtureEventSummary>,
    pub facts: FixtureFacts,
    pub evidence_summary: FixtureEvidenceSummary,
    pub extensions: Value,
}

pub fn builtin_platform_fixture_ids() -> &'static [&'static str] {
    &["codex-cli-synthetic-v1", "claude-code-synthetic-v1"]
}

pub fn inspect_builtin_platform_fixture(fixture_id: &str) -> Result<FixtureInspection, CoreError> {
    let (fixture_file, fixture_source) = match fixture_id {
        "codex-cli-synthetic-v1" => ("codex-cli-v1.json", CODEX_PLATFORM_FIXTURE),
        "claude-code-synthetic-v1" => ("claude-code-v1.json", CLAUDE_PLATFORM_FIXTURE),
        _ => return Err(CoreError::FixtureNotFound),
    };
    let manifest: PlatformFixtureManifest = serde_json::from_str(PLATFORM_FIXTURE_MANIFEST)?;
    if manifest.contract != "jzmatrix.synthetic-platform-fixture-manifest"
        || manifest.version != "1.0.0"
        || manifest.parser_version != PLATFORM_FIXTURE_PARSER_VERSION
        || !manifest.synthetic
        || manifest.network_required
        || manifest.source_observation != "10A_P0_platform_probe"
        || manifest.protocol_coverage != PLATFORM_FIXTURE_PROTOCOL_COVERAGE
    {
        return Err(CoreError::FixtureIntegrity);
    }
    let entry = manifest
        .fixtures
        .iter()
        .find(|entry| entry.id == fixture_id && entry.file == fixture_file)
        .ok_or(CoreError::FixtureIntegrity)?;
    if entry.platform.is_empty() || entry.observed_event_kinds.is_empty() {
        return Err(CoreError::FixtureIntegrity);
    }
    let fixture_sha256 = sha256_hex(fixture_source.as_bytes());
    if entry.sha256 != fixture_sha256 {
        return Err(CoreError::FixtureIntegrity);
    }
    let mut parsed = parse_platform_fixture(fixture_id, fixture_source.as_bytes())?;
    if parsed.platform != entry.platform
        || parsed
            .events
            .iter()
            .any(|event| !entry.observed_event_kinds.contains(&event.event_kind))
    {
        return Err(CoreError::FixtureIntegrity);
    }
    parsed.manifest_sha256 = sha256_hex(PLATFORM_FIXTURE_MANIFEST.as_bytes());
    parsed.fixture_sha256 = fixture_sha256;
    Ok(parsed)
}

pub fn parse_platform_fixture(
    requested_fixture_id: &str,
    bytes: &[u8],
) -> Result<FixtureInspection, CoreError> {
    let value: Value = serde_json::from_slice(bytes)?;
    inspect_value_safety(&value)?;
    let root = value
        .as_object()
        .ok_or(CoreError::FixtureSchemaUnsupported)?;
    let contract = required_string(root, "contract")?;
    let version = required_string(root, "version")?;
    let fixture_id = required_string(root, "fixture_id")?;
    let platform = required_string(root, "platform")?;
    let synthetic = root
        .get("synthetic")
        .and_then(Value::as_bool)
        .ok_or(CoreError::FixtureSchemaUnsupported)?;
    let source_observation = required_string(root, "source_observation")?;
    let protocol_coverage = required_string(root, "protocol_coverage")?;
    let events = root
        .get("events")
        .and_then(Value::as_array)
        .ok_or(CoreError::FixtureSchemaUnsupported)?;

    if requested_fixture_id != fixture_id
        || contract != PLATFORM_FIXTURE_CONTRACT
        || version != "1.0.0"
        || !synthetic
        || !matches!(platform.as_str(), "codex_cli" | "claude_code")
        || source_observation != "10A_P0_platform_probe"
        || protocol_coverage != PLATFORM_FIXTURE_PROTOCOL_COVERAGE
        || events.is_empty()
    {
        return Err(CoreError::FixtureSchemaUnsupported);
    }

    let root_known = [
        "contract",
        "version",
        "fixture_id",
        "platform",
        "synthetic",
        "source_observation",
        "protocol_coverage",
        "events",
        "extensions",
    ];
    let mut root_unknown = Map::new();
    for (key, value) in root {
        if !root_known.contains(&key.as_str()) {
            root_unknown.insert(key.clone(), value.clone());
        }
    }
    let input_extensions = match root.get("extensions") {
        Some(Value::Object(extensions)) => Value::Object(extensions.clone()),
        Some(_) => return Err(CoreError::FixtureSchemaUnsupported),
        None => json!({}),
    };

    let mut event_summaries = Vec::new();
    let mut facts = FixtureFacts::default();
    let mut event_kinds = BTreeSet::new();
    let mut unknown_event_kinds = BTreeSet::new();
    let mut event_unknowns = Map::new();
    let mut seen_event_ids = BTreeMap::new();
    let mut seen_sequences = BTreeMap::new();
    let mut first_sequence = None;
    let mut last_sequence = None;
    let mut previous_sequence = None;
    let mut next_expected_sequence = 1_u64;
    let mut has_gap = false;
    let mut out_of_order = false;
    let mut duplicate_events = 0_usize;

    for event_value in events {
        let event = event_value
            .as_object()
            .ok_or(CoreError::FixtureSchemaUnsupported)?;
        let event_id = required_string(event, "event_id")?;
        let run_id = required_string(event, "run_id")?;
        let sequence = event
            .get("sequence")
            .and_then(Value::as_u64)
            .ok_or(CoreError::FixtureSchemaUnsupported)?;
        let observed_at = required_string(event, "observed_at")?;
        let event_kind = required_string(event, "event_kind")?;
        let payload = event
            .get("payload")
            .and_then(Value::as_object)
            .ok_or(CoreError::FixtureSchemaUnsupported)?;
        let event_digest = sha256_hex(&serde_json::to_vec(event)?);

        if let Some(previous_digest) = seen_event_ids.get(&event_id) {
            if previous_digest == &event_digest {
                duplicate_events += 1;
                continue;
            }
            return Err(CoreError::FixtureEventConflict);
        }
        let sequence_key = (run_id.clone(), sequence);
        if let Some(previous_digest) = seen_sequences.get(&sequence_key) {
            if previous_digest == &event_digest {
                duplicate_events += 1;
                continue;
            }
            return Err(CoreError::FixtureEventConflict);
        }
        seen_event_ids.insert(event_id.clone(), event_digest);
        seen_sequences.insert(sequence_key, sha256_hex(&serde_json::to_vec(event)?));

        let event_known = [
            "event_id",
            "run_id",
            "sequence",
            "observed_at",
            "event_kind",
            "payload",
        ];
        let mut event_unknown = Map::new();
        for (key, value) in event {
            if !event_known.contains(&key.as_str()) {
                event_unknown.insert(key.clone(), value.clone());
            }
        }
        let known_payload_fields = known_payload_fields(&event_kind);
        let mut payload_unknown = Map::new();
        for (key, value) in payload {
            if !known_payload_fields.contains(&key.as_str()) {
                payload_unknown.insert(key.clone(), value.clone());
            }
        }
        if !payload_unknown.is_empty() {
            event_unknown.insert(
                "payload_unknown_fields".to_owned(),
                Value::Object(payload_unknown),
            );
        }
        if !event_unknown.is_empty() {
            event_unknowns.insert(event_id.clone(), Value::Object(event_unknown));
        }

        event_kinds.insert(event_kind.clone());
        if !is_known_event_kind(&event_kind) {
            unknown_event_kinds.insert(event_kind.clone());
        }

        if first_sequence.is_none() {
            first_sequence = Some(sequence);
            if sequence > 1 {
                has_gap = true;
            }
        }
        if let Some(previous) = previous_sequence {
            if sequence < previous {
                out_of_order = true;
            }
        }
        match sequence.cmp(&next_expected_sequence) {
            Ordering::Greater => {
                has_gap = true;
                next_expected_sequence = sequence.saturating_add(1);
            }
            Ordering::Equal => {
                next_expected_sequence = next_expected_sequence.saturating_add(1);
            }
            Ordering::Less => {}
        }
        previous_sequence = Some(sequence);
        last_sequence = Some(sequence);

        event_summaries.push(FixtureEventSummary {
            event_id: event_id.clone(),
            run_id,
            sequence,
            observed_at,
            event_kind: event_kind.clone(),
            payload_sha256: sha256_hex(&serde_json::to_vec(payload)?),
        });
        project_event_facts(&mut facts, &event_kind, payload, &event_id);
    }

    let parse_status = if has_gap || out_of_order {
        facts = FixtureFacts::default();
        FixtureParseStatus::Partial
    } else if !unknown_event_kinds.is_empty() {
        facts = FixtureFacts::default();
        FixtureParseStatus::Unknown
    } else {
        FixtureParseStatus::Parsed
    };

    let extensions = json!({
        "unknown_fields": {
            "root": Value::Object(root_unknown),
            "events": Value::Object(event_unknowns),
        },
        "unknown_event_kinds": unknown_event_kinds.into_iter().collect::<Vec<_>>(),
        "input_extensions": input_extensions,
    });
    Ok(FixtureInspection {
        contract,
        version,
        parser_version: PLATFORM_FIXTURE_PARSER_VERSION.to_owned(),
        fixture_id,
        platform,
        synthetic,
        source_observation,
        protocol_coverage,
        parse_status,
        manifest_sha256: String::new(),
        fixture_sha256: sha256_hex(bytes),
        event_cursor: FixtureEventCursor {
            first_sequence,
            last_sequence,
            next_expected_sequence,
            has_gap,
            out_of_order,
            duplicate_events,
        },
        events: event_summaries,
        facts,
        evidence_summary: FixtureEvidenceSummary {
            source: "synthetic_fixture".to_owned(),
            event_count: events.len(),
            unique_event_count: seen_event_ids.len(),
            event_kinds: event_kinds.into_iter().collect(),
            persisted_raw_payload: false,
            persisted_transcript: false,
            persisted_secrets: false,
            persisted_absolute_paths: false,
        },
        extensions,
    })
}

fn required_string(object: &Map<String, Value>, field: &str) -> Result<String, CoreError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .ok_or(CoreError::FixtureSchemaUnsupported)
}

fn known_payload_fields(event_kind: &str) -> &'static [&'static str] {
    match event_kind {
        "thread.started" => &["platform_ref"],
        "turn.started" | "turn.completed" => &["status"],
        "item.completed" => &["item_kind", "status"],
        "system.init" => &["status", "session_ref"],
        "assistant" => &["status"],
        "result" => &["status", "complete"],
        "exit" => &["code", "complete"],
        "message" => &["status"],
        "artifact_observed" => &["receipt_kind", "artifact_kind"],
        _ => &[],
    }
}

fn is_known_event_kind(event_kind: &str) -> bool {
    matches!(
        event_kind,
        "thread.started"
            | "turn.started"
            | "item.completed"
            | "turn.completed"
            | "system.init"
            | "assistant"
            | "result"
            | "exit"
            | "message"
            | "artifact_observed"
            | "progress"
            | "handshake"
            | "session_created"
            | "tool_call"
            | "permission_request"
            | "cancel_requested"
            | "cancelled"
            | "error"
    )
}

fn project_event_facts(
    facts: &mut FixtureFacts,
    event_kind: &str,
    payload: &Map<String, Value>,
    event_id: &str,
) {
    if matches!(
        event_kind,
        "thread.started" | "turn.started" | "system.init" | "assistant" | "progress"
    ) {
        observe_fact(&mut facts.activity, event_id);
    }

    let is_completed = matches!(event_kind, "item.completed" | "turn.completed")
        || (event_kind == "result"
            && (payload.get("complete").and_then(Value::as_bool) == Some(true)
                || payload.get("status").and_then(Value::as_str) == Some("completed")))
        || (event_kind == "exit"
            && payload.get("code").and_then(Value::as_i64) == Some(0)
            && payload.get("complete").and_then(Value::as_bool) == Some(true));
    if is_completed {
        observe_fact(&mut facts.completed, event_id);
    }

    if event_kind == "message"
        && payload.get("status").and_then(Value::as_str) == Some("sent_not_confirmed")
    {
        observe_fact(&mut facts.sent_not_confirmed, event_id);
    }
    if event_kind == "artifact_observed" {
        match payload.get("receipt_kind").and_then(Value::as_str) {
            Some("local_written") => observe_fact(&mut facts.local_written, event_id),
            Some("sent_not_confirmed") => observe_fact(&mut facts.sent_not_confirmed, event_id),
            Some("delivered") => observe_fact(&mut facts.delivered, event_id),
            Some("accepted") => observe_fact(&mut facts.accepted, event_id),
            _ => {}
        }
    }
}

fn observe_fact(observation: &mut FixtureFactObservation, event_id: &str) {
    observation.state = FixtureFactState::Observed;
    if !observation
        .evidence_event_ids
        .iter()
        .any(|id| id == event_id)
    {
        observation.evidence_event_ids.push(event_id.to_owned());
    }
}

fn inspect_value_safety(value: &Value) -> Result<(), CoreError> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if is_sensitive_key(key) {
                    return Err(CoreError::FixtureBlocked("fixture_sensitive_field"));
                }
                inspect_value_safety(child)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                inspect_value_safety(child)?;
            }
        }
        Value::String(string) => {
            if looks_like_private_value(string) {
                return Err(CoreError::FixtureBlocked("fixture_private_value"));
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
    Ok(())
}

fn is_sensitive_key(key: &str) -> bool {
    let normalized = key.to_ascii_lowercase().replace(['-', ' '], "_");
    [
        "api_key",
        "access_token",
        "refresh_token",
        "authorization",
        "cookie",
        "password",
        "private_key",
        "client_secret",
        "secret",
        "token",
        "prompt",
        "response",
        "transcript",
        "tool_input",
        "tool_output",
        "message_body",
        "content",
    ]
    .iter()
    .any(|marker| normalized == *marker || normalized.contains(marker))
}

fn looks_like_private_value(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    value.starts_with('/')
        || value.starts_with("\\\\")
        || value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("file://")
        || (value.len() >= 3
            && value.as_bytes()[1] == b':'
            && matches!(value.as_bytes()[2], b'/' | b'\\'))
        || lower.contains("-----begin ")
        || lower.contains("bearer ")
        || lower.contains("sk-")
        || lower.contains("ghp_")
        || lower.contains("xoxb-")
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

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseInspection {
    pub contract: String,
    pub version: String,
    pub integrity: String,
    pub foreign_key_violations: usize,
    pub migration_count: usize,
    pub tables: Vec<String>,
}

pub fn inspect_product_database(db_path: &Path) -> Result<DatabaseInspection, CoreError> {
    let connection = Connection::open(db_path)?;
    let integrity: String = connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;

    let mut foreign_key_statement = connection.prepare("PRAGMA foreign_key_check")?;
    let mut foreign_key_rows = foreign_key_statement.query([])?;
    let mut foreign_key_violations = 0_usize;
    while foreign_key_rows.next()?.is_some() {
        foreign_key_violations += 1;
    }
    let migration_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM schema_migrations WHERE status = 'committed'",
        [],
        |row| row.get(0),
    )?;
    let mut table_statement = connection.prepare(
        "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )?;
    let tables = table_statement
        .query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>, _>>()?;

    if integrity != "ok" || foreign_key_violations > 0 || migration_count < 0 {
        return Err(CoreError::DatabaseIntegrity);
    }

    Ok(DatabaseInspection {
        contract: "jzmatrix.sqlite-inspection".to_owned(),
        version: "1.0.0".to_owned(),
        integrity,
        foreign_key_violations,
        migration_count: migration_count as usize,
        tables,
    })
}

pub fn default_app_data_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("com", "JIEZIJIUWEI", "JZMatrix Workbench")
        .map(|dirs| dirs.data_dir().to_path_buf())
}

pub fn blocked_response(code: &str, message: &str) -> CliResponse {
    blocked_command_response("doctor", code, message)
}

pub fn blocked_command_response(command: &str, code: &str, message: &str) -> CliResponse {
    let observed_at = now_utc();
    let data = if command == "doctor" {
        json!({
            "doctor_version": "1.0.0",
            "observed_at": observed_at,
            "checks": [],
            "summary": {"pass": 0, "degraded": 0, "blocked": 1, "unknown": 0, "not_run": 0},
            "overall_status": "blocked"
        })
    } else {
        json!({
            "command": command,
            "parse_status": "unknown",
            "read_only": true
        })
    };
    let next_actions = if command == "doctor" {
        vec![NextAction {
            action_id: "inspect_environment".to_owned(),
            label: "检查产品运行环境".to_owned(),
        }]
    } else {
        Vec::new()
    };
    CliResponse {
        contract: "jzmatrix.cli-response".to_owned(),
        version: "1.0.0".to_owned(),
        command: command.to_owned(),
        request_id: Uuid::now_v7().to_string(),
        ok: false,
        status: ResponseStatus::Blocked,
        outcome: Outcome::NotCommitted,
        data,
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
            reference: format!("evidence:{command}"),
        }],
        next_actions,
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

    let database_path = app_data_dir.join("db/app.sqlite3");
    let database_inspection =
        ensure_schema(&database_path).and_then(|()| inspect_product_database(&database_path));
    let sqlite_status = match &database_inspection {
        Ok(_) => CheckStatus::Pass,
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
        extensions: json!({
            "network": "disabled_by_default",
            "daemon_mode": false,
            "database": database_inspection.ok(),
        }),
    }
}

pub fn run_ephemeral_doctor() -> CliResponse {
    let app_data_dir = std::env::temp_dir().join(format!("jzmatrix-doctor-{}", new_request_id()));
    let response = run_doctor(&app_data_dir);
    let cleanup_succeeded = fs::remove_dir_all(&app_data_dir).is_ok();
    finish_ephemeral_doctor(response, cleanup_succeeded)
}

fn finish_ephemeral_doctor(mut response: CliResponse, cleanup_succeeded: bool) -> CliResponse {
    if !cleanup_succeeded {
        response = blocked_response(
            "ephemeral_cleanup_failed",
            "临时产品数据目录未能清理，检查结果已阻断",
        );
    }
    if let Some(extensions) = response.extensions.as_object_mut() {
        extensions.insert(
            "storage".to_owned(),
            json!({
                "mode": "ephemeral_product_temp",
                "path_redacted": true,
                "cleanup_succeeded": cleanup_succeeded,
            }),
        );
    }
    response
}

fn writable_probe(app_data_dir: &Path) -> Result<(), CoreError> {
    fs::create_dir_all(app_data_dir)?;
    let probe = app_data_dir.join(format!(".doctor-write-probe-{}", Uuid::now_v7()));
    fs::write(&probe, b"jzmatrix doctor")?;
    fs::remove_file(probe)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
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

    fn fixture_value(fixture_id: &str) -> Value {
        let source = match fixture_id {
            "codex-cli-synthetic-v1" => CODEX_PLATFORM_FIXTURE,
            "claude-code-synthetic-v1" => CLAUDE_PLATFORM_FIXTURE,
            _ => panic!("unknown test fixture"),
        };
        serde_json::from_str(source).expect("test fixture JSON")
    }

    fn parse_value(fixture_id: &str, value: &Value) -> Result<FixtureInspection, CoreError> {
        let bytes = serde_json::to_vec(value).expect("serialize test fixture");
        parse_platform_fixture(fixture_id, &bytes)
    }

    fn append_receipt(value: &mut Value, sequence: u64, receipt_kind: &str) {
        value["events"]
            .as_array_mut()
            .expect("events array")
            .push(json!({
                "event_id": format!("codex-fixture-receipt-{sequence:03}"),
                "run_id": "run:codex-synthetic-001",
                "sequence": sequence,
                "observed_at": format!("2026-08-03T00:00:{sequence:02}Z"),
                "event_kind": "artifact_observed",
                "payload": {
                    "receipt_kind": receipt_kind,
                    "artifact_kind": "synthetic_artifact"
                }
            }));
    }

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

    #[test]
    fn sqlite_inspection_reports_expected_schema_without_exposing_a_path() {
        let directory = tempfile::tempdir().expect("temporary app data directory");
        let db = directory.path().join("db/app.sqlite3");
        ensure_schema(&db).expect("schema application");
        let inspection = inspect_product_database(&db).expect("database inspection");

        assert_eq!(inspection.integrity, "ok");
        assert_eq!(inspection.foreign_key_violations, 0);
        assert_eq!(inspection.migration_count, 1);
        assert_eq!(
            inspection.tables,
            vec![
                "app_meta".to_owned(),
                "collaboration_groups".to_owned(),
                "schema_migrations".to_owned(),
            ]
        );
    }

    #[test]
    fn ephemeral_cleanup_failure_is_blocked_without_a_path() {
        let directory = tempfile::tempdir().expect("temporary app data directory");
        let response = finish_ephemeral_doctor(run_doctor(directory.path()), false);
        let serialized = serde_json::to_string(&response).expect("serialize blocked response");

        assert!(!response.ok);
        assert_eq!(response.exit_code(), 2);
        assert_eq!(response.errors[0].code, "ephemeral_cleanup_failed");
        assert_eq!(response.extensions["storage"]["cleanup_succeeded"], false);
        assert!(!serialized.contains(&directory.path().display().to_string()));
    }

    #[test]
    fn built_in_platform_fixtures_are_manifest_verified_and_parseable() {
        assert_eq!(builtin_platform_fixture_ids().len(), 2);
        for fixture_id in builtin_platform_fixture_ids() {
            let inspection = inspect_builtin_platform_fixture(fixture_id).expect("fixture parses");
            assert_eq!(inspection.parse_status, FixtureParseStatus::Parsed);
            assert!(inspection.synthetic);
            assert!(!inspection.manifest_sha256.is_empty());
            assert!(!inspection.fixture_sha256.is_empty());
            assert!(!inspection.evidence_summary.persisted_raw_payload);
            assert!(!inspection.evidence_summary.persisted_transcript);
        }
    }

    #[test]
    fn completion_does_not_upgrade_delivery_acceptance_or_local_receipts() {
        let codex = inspect_builtin_platform_fixture("codex-cli-synthetic-v1").expect("codex");
        assert_eq!(codex.facts.activity.state, FixtureFactState::Observed);
        assert_eq!(codex.facts.completed.state, FixtureFactState::Observed);
        assert_eq!(codex.facts.local_written.state, FixtureFactState::Unknown);
        assert_eq!(
            codex.facts.sent_not_confirmed.state,
            FixtureFactState::Unknown
        );
        assert_eq!(codex.facts.delivered.state, FixtureFactState::Unknown);
        assert_eq!(codex.facts.accepted.state, FixtureFactState::Unknown);

        let mut value = fixture_value("codex-cli-synthetic-v1");
        append_receipt(&mut value, 6, "local_written");
        append_receipt(&mut value, 7, "sent_not_confirmed");
        append_receipt(&mut value, 8, "delivered");
        append_receipt(&mut value, 9, "accepted");
        let with_receipts = parse_value("codex-cli-synthetic-v1", &value).expect("receipts parse");
        assert_eq!(
            with_receipts.facts.local_written.state,
            FixtureFactState::Observed
        );
        assert_eq!(
            with_receipts.facts.sent_not_confirmed.state,
            FixtureFactState::Observed
        );
        assert_eq!(
            with_receipts.facts.delivered.state,
            FixtureFactState::Observed
        );
        assert_eq!(
            with_receipts.facts.accepted.state,
            FixtureFactState::Observed
        );
        assert!(!with_receipts.facts.completed.evidence_event_ids.is_empty());
    }

    #[test]
    fn unknown_fields_are_preserved_without_persisting_payload_as_evidence() {
        let mut value = fixture_value("codex-cli-synthetic-v1");
        value["future_root"] = json!({"enabled": true});
        value["events"][0]["future_event_field"] = json!("preserve-me");
        value["events"][0]["payload"]["future_payload_field"] = json!(42);
        let inspection =
            parse_value("codex-cli-synthetic-v1", &value).expect("unknown fields parse");
        assert_eq!(inspection.parse_status, FixtureParseStatus::Parsed);
        assert_eq!(
            inspection.extensions["unknown_fields"]["root"]["future_root"]["enabled"],
            true
        );
        assert_eq!(
            inspection.extensions["unknown_fields"]["events"]["codex-fixture-event-001"]
                ["future_event_field"],
            "preserve-me"
        );
        assert_eq!(
            inspection.extensions["unknown_fields"]["events"]["codex-fixture-event-001"]
                ["payload_unknown_fields"]["future_payload_field"],
            42
        );
        assert!(!inspection.evidence_summary.persisted_raw_payload);
    }

    #[test]
    fn identical_duplicate_events_are_idempotent() {
        let mut value = fixture_value("codex-cli-synthetic-v1");
        let duplicate = value["events"][0].clone();
        value["events"]
            .as_array_mut()
            .expect("events array")
            .push(duplicate);
        let inspection = parse_value("codex-cli-synthetic-v1", &value).expect("duplicate parses");
        assert_eq!(inspection.parse_status, FixtureParseStatus::Parsed);
        assert_eq!(inspection.event_cursor.duplicate_events, 1);
        assert_eq!(inspection.evidence_summary.event_count, 6);
        assert_eq!(inspection.evidence_summary.unique_event_count, 5);
        assert_eq!(inspection.facts.activity.evidence_event_ids.len(), 2);
    }

    #[test]
    fn sequence_gap_fails_closed_and_preserves_partial_evidence() {
        let mut value = fixture_value("codex-cli-synthetic-v1");
        let events = value["events"].as_array_mut().expect("events array");
        events.remove(2);
        events[1]["sequence"] = json!(3);
        let inspection = parse_value("codex-cli-synthetic-v1", &value).expect("gap is recoverable");
        assert_eq!(inspection.parse_status, FixtureParseStatus::Partial);
        assert!(inspection.event_cursor.has_gap);
        assert_eq!(inspection.facts.activity.state, FixtureFactState::Unknown);
        assert_eq!(inspection.facts.completed.state, FixtureFactState::Unknown);
    }

    #[test]
    fn sequence_conflict_is_rejected() {
        let mut value = fixture_value("codex-cli-synthetic-v1");
        value["events"]
            .as_array_mut()
            .expect("events array")
            .push(json!({
                "event_id": "codex-fixture-conflict-006",
                "run_id": "run:codex-synthetic-001",
                "sequence": 3,
                "observed_at": "2026-08-03T00:00:06Z",
                "event_kind": "message",
                "payload": {"status": "sent_not_confirmed"}
            }));
        assert!(matches!(
            parse_value("codex-cli-synthetic-v1", &value),
            Err(CoreError::FixtureEventConflict)
        ));
    }

    #[test]
    fn unknown_event_kind_is_unknown_and_does_not_project_business_facts() {
        let mut value = fixture_value("codex-cli-synthetic-v1");
        value["events"][0]["event_kind"] = json!("vendor.future_event");
        let inspection =
            parse_value("codex-cli-synthetic-v1", &value).expect("unknown event parses");
        assert_eq!(inspection.parse_status, FixtureParseStatus::Unknown);
        assert_eq!(inspection.facts.activity.state, FixtureFactState::Unknown);
        assert_eq!(inspection.facts.completed.state, FixtureFactState::Unknown);
        assert_eq!(
            inspection.extensions["unknown_event_kinds"][0],
            "vendor.future_event"
        );
    }

    #[test]
    fn secret_fields_and_absolute_paths_are_blocked_without_echoing_values() {
        let mut secret = fixture_value("codex-cli-synthetic-v1");
        secret["api_key"] = json!("synthetic-secret-value");
        assert!(matches!(
            parse_value("codex-cli-synthetic-v1", &secret),
            Err(CoreError::FixtureBlocked("fixture_sensitive_field"))
        ));

        let mut path = fixture_value("codex-cli-synthetic-v1");
        path["future_path"] = json!("/private/synthetic-project");
        assert!(matches!(
            parse_value("codex-cli-synthetic-v1", &path),
            Err(CoreError::FixtureBlocked("fixture_private_value"))
        ));
    }

    #[test]
    fn unsupported_fixture_ids_and_schema_are_closed() {
        assert!(matches!(
            inspect_builtin_platform_fixture("/private/not-a-fixture"),
            Err(CoreError::FixtureNotFound)
        ));
        let mut value = fixture_value("codex-cli-synthetic-v1");
        value["synthetic"] = json!(false);
        assert!(matches!(
            parse_value("codex-cli-synthetic-v1", &value),
            Err(CoreError::FixtureSchemaUnsupported)
        ));
    }
}
