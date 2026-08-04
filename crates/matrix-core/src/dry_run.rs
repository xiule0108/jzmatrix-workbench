use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::{ensure_schema, load_group_tx, now_utc, open_app_database, CoreError};
use std::path::Path;

pub const MIGRATION_SQL: &str = include_str!("../migrations/0004_dry_run_plans.sql");

const PLAN_CONTRACT: &str = "jzmatrix.dry-run-plan";
const PLAN_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunPermissionSnapshot {
    pub external_processes: bool,
    pub network: bool,
    pub external_writes: bool,
    pub configuration_reads: bool,
    pub session_reads: bool,
    pub secrets_reads: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunStep {
    pub id: String,
    pub operation: String,
    pub label: String,
    pub status: String,
    pub requires_confirmation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DryRunPlan {
    pub contract: String,
    pub version: String,
    pub id: String,
    pub group_id: String,
    pub observed_at: String,
    pub data_source: String,
    pub execution: String,
    pub adapter_status: String,
    pub target_tools: Vec<String>,
    pub permissions: DryRunPermissionSnapshot,
    pub steps: Vec<DryRunStep>,
    pub persisted: bool,
}

pub fn create_dry_run_plan(db_path: &Path, group_id: &str) -> Result<DryRunPlan, CoreError> {
    if !group_id.is_empty()
        && group_id.chars().count() <= 128
        && !group_id.chars().any(|character| character.is_control())
        && !group_id.contains('/')
        && !group_id.contains('\\')
    {
        // Continue into the same schema and group-loading path used by the local group commands.
    } else {
        return Err(CoreError::InvalidGroupInput);
    }

    ensure_schema(db_path)?;
    let mut connection = open_app_database(db_path)?;
    let transaction = connection.transaction()?;
    let group = load_group_tx(&transaction, group_id)?;
    let observed_at = now_utc();
    let plan = DryRunPlan {
        contract: PLAN_CONTRACT.to_owned(),
        version: PLAN_VERSION.to_owned(),
        id: Uuid::now_v7().to_string(),
        group_id: group.id,
        observed_at,
        data_source: group.data_source,
        execution: "not_authorized".to_owned(),
        adapter_status: "not_implemented".to_owned(),
        target_tools: Vec::new(),
        permissions: DryRunPermissionSnapshot {
            external_processes: false,
            network: false,
            external_writes: false,
            configuration_reads: false,
            session_reads: false,
            secrets_reads: false,
        },
        steps: vec![
            DryRunStep {
                id: "create".to_owned(),
                operation: "create".to_owned(),
                label: "创建工具工作窗口".to_owned(),
                status: "not_authorized".to_owned(),
                requires_confirmation: true,
            },
            DryRunStep {
                id: "send".to_owned(),
                operation: "send".to_owned(),
                label: "发送协作消息".to_owned(),
                status: "not_authorized".to_owned(),
                requires_confirmation: true,
            },
            DryRunStep {
                id: "resume".to_owned(),
                operation: "resume".to_owned(),
                label: "恢复既有任务".to_owned(),
                status: "not_authorized".to_owned(),
                requires_confirmation: true,
            },
            DryRunStep {
                id: "cancel".to_owned(),
                operation: "cancel".to_owned(),
                label: "取消外部任务".to_owned(),
                status: "not_authorized".to_owned(),
                requires_confirmation: true,
            },
        ],
        persisted: false,
    };

    let mut persisted_plan = plan;
    persisted_plan.persisted = true;
    let plan_json = serde_json::to_string(&persisted_plan)?;
    transaction.execute(
        "INSERT INTO dry_run_plans
            (id, group_id, observed_at, data_source, execution, plan_json)
         VALUES (?1, ?2, ?3, ?4, 'not_authorized', ?5)",
        params![
            &persisted_plan.id,
            &persisted_plan.group_id,
            &persisted_plan.observed_at,
            &persisted_plan.data_source,
            &plan_json
        ],
    )?;
    transaction.commit()?;
    Ok(persisted_plan)
}

pub fn dry_run_plan_response_data(plan: &DryRunPlan) -> serde_json::Value {
    json!(plan)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{create_dry_run_plan, DryRunPlan};
    use crate::{create_collaboration_group, CreateGroupInput};

    #[test]
    fn plan_is_persisted_and_never_authorized() {
        let directory = tempfile::tempdir().expect("temporary app data directory");
        let db = directory.path().join("db/app.sqlite3");
        let group = create_collaboration_group(
            &db,
            CreateGroupInput {
                goal: "生成只读计划",
                template_id: "compact",
                idempotency_key: "dry-run-test-001",
            },
        )
        .expect("create local group");

        let plan = create_dry_run_plan(&db, &group.id).expect("create dry-run plan");
        assert!(plan.persisted);
        assert_eq!(plan.execution, "not_authorized");
        assert_eq!(plan.adapter_status, "not_implemented");
        assert_eq!(plan.steps.len(), 4);
        assert!(plan
            .steps
            .iter()
            .all(|step| { step.status == "not_authorized" && step.requires_confirmation }));
        assert!(!plan.permissions.external_processes);
        assert!(!plan.permissions.network);

        let connection = Connection::open(db).expect("open database");
        let stored: String = connection
            .query_row(
                "SELECT plan_json FROM dry_run_plans WHERE id = ?1",
                [&plan.id],
                |row| row.get(0),
            )
            .expect("stored plan");
        let stored_plan: DryRunPlan = serde_json::from_str(&stored).expect("stored plan JSON");
        assert_eq!(stored_plan.execution, "not_authorized");
        assert!(stored_plan.persisted);
        assert!(!stored.contains("raw_stdout"));
        assert!(stored.contains("session_reads"));
    }

    #[test]
    fn unknown_group_is_closed_without_a_plan() {
        let directory = tempfile::tempdir().expect("temporary app data directory");
        let db = directory.path().join("db/app.sqlite3");
        let result = create_dry_run_plan(&db, "missing-group");
        assert!(matches!(result, Err(crate::CoreError::GroupNotFound)));
        let connection = Connection::open(db).expect("open database");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM dry_run_plans", [], |row| row.get(0))
            .expect("plan count");
        assert_eq!(count, 0);
    }
}
