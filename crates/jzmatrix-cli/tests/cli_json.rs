use std::process::Command;

#[test]
fn ephemeral_doctor_json_is_parseable_and_offline() {
    let directory = tempfile::tempdir().expect("temporary runtime root");
    let output = Command::new(env!("CARGO_BIN_EXE_jzmatrix"))
        .env("TMPDIR", directory.path())
        .env("TEMP", directory.path())
        .env("TMP", directory.path())
        .args(["doctor", "--ephemeral", "--json"])
        .output()
        .expect("run jzmatrix doctor");

    assert!(
        output.status.success(),
        "doctor failed: {:?}",
        output.status
    );
    let response: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("doctor output must be JSON");
    assert_eq!(response["contract"], "jzmatrix.cli-response");
    assert_eq!(response["status"], "pass");
    assert_eq!(response["data"]["overall_status"], "pass");
    assert_eq!(response["extensions"]["network"], "disabled_by_default");
    let checks = response["data"]["checks"]
        .as_array()
        .expect("doctor checks must be an array");
    let optional_agent = checks
        .iter()
        .find(|check| check["id"] == "agent.optional")
        .expect("optional agent check must be present");
    assert_eq!(optional_agent["status"], "not_run");
    assert_eq!(response["extensions"]["database"]["integrity"], "ok");
    assert_eq!(
        response["extensions"]["database"]["foreign_key_violations"],
        0
    );
    assert_eq!(response["extensions"]["database"]["migration_count"], 1);
}

#[test]
fn ephemeral_doctor_cleans_its_product_created_database() {
    let directory = tempfile::tempdir().expect("temporary runtime root");
    let output = Command::new(env!("CARGO_BIN_EXE_jzmatrix"))
        .env("TMPDIR", directory.path())
        .env("TEMP", directory.path())
        .env("TMP", directory.path())
        .args(["doctor", "--ephemeral", "--json"])
        .output()
        .expect("run ephemeral doctor");

    assert!(
        output.status.success(),
        "ephemeral doctor failed: {:?}",
        output.status
    );
    let response: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("doctor output must be JSON");
    assert_eq!(
        response["extensions"]["storage"]["mode"],
        "ephemeral_product_temp"
    );
    assert_eq!(response["extensions"]["storage"]["path_redacted"], true);
    assert_eq!(response["extensions"]["storage"]["cleanup_succeeded"], true);
    assert_eq!(
        std::fs::read_dir(directory.path())
            .expect("read temporary root")
            .count(),
        0
    );
}

#[test]
fn offline_demo_json_is_read_only_and_uses_the_cli_contract() {
    let directory = tempfile::tempdir().expect("temporary isolated environment");
    let output = Command::new(env!("CARGO_BIN_EXE_jzmatrix"))
        .env("HOME", directory.path())
        .env("APPDATA", directory.path())
        .env("LOCALAPPDATA", directory.path())
        .env("XDG_DATA_HOME", directory.path())
        .args(["offline-demo", "--json"])
        .output()
        .expect("run offline demo");

    assert!(
        output.status.success(),
        "offline demo failed: {:?}",
        output.status
    );
    let response: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("offline demo output must be JSON");
    assert_eq!(response["contract"], "jzmatrix.cli-response");
    assert_eq!(response["command"], "offline-demo");
    assert_eq!(response["status"], "pass");
    assert_eq!(response["outcome"], "not_committed");
    assert_eq!(response["data"]["data_source"], "demo");
    assert_eq!(
        response["data"]["groups"][0]["events"][1]["status"],
        "not_run"
    );
    assert_eq!(response["extensions"]["offline"], true);
    assert_eq!(response["extensions"]["external_processes"], false);
}

#[test]
fn fixture_inspect_is_built_in_read_only_json() {
    let output = Command::new(env!("CARGO_BIN_EXE_jzmatrix"))
        .args([
            "fixture",
            "inspect",
            "--id",
            "claude-code-synthetic-v1",
            "--json",
        ])
        .output()
        .expect("run fixture inspect");

    assert!(
        output.status.success(),
        "fixture inspect failed: {:?}",
        output.status
    );
    let response: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("fixture output must be JSON");
    assert_eq!(response["command"], "fixture.inspect");
    assert_eq!(response["status"], "pass");
    assert_eq!(response["outcome"], "not_committed");
    assert_eq!(response["data"]["fixture_id"], "claude-code-synthetic-v1");
    assert_eq!(response["data"]["parse_status"], "parsed");
    assert_eq!(response["data"]["facts"]["completed"]["state"], "observed");
    assert_eq!(response["data"]["facts"]["delivered"]["state"], "unknown");
    assert_eq!(
        response["data"]["evidence_summary"]["persisted_transcript"],
        false
    );
    assert_eq!(response["extensions"]["arbitrary_paths"], false);
}

#[test]
fn fixture_inspect_rejects_unknown_or_path_like_ids_without_echoing_them() {
    let requested_id = "/private/should-not-be-read";
    let output = Command::new(env!("CARGO_BIN_EXE_jzmatrix"))
        .args(["fixture", "inspect", "--id", requested_id, "--json"])
        .output()
        .expect("run blocked fixture inspect");

    assert_eq!(output.status.code(), Some(2));
    let response: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("blocked fixture output must be JSON");
    assert_eq!(response["status"], "blocked");
    assert_eq!(response["errors"][0]["code"], "fixture_not_found");
    let output_text = String::from_utf8_lossy(&output.stdout);
    assert!(!output_text.contains(requested_id));
}
