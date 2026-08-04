use std::process::Command;

#[test]
fn doctor_json_is_parseable_and_offline_by_default() {
    let directory = tempfile::tempdir().expect("temporary app data directory");
    let output = Command::new(env!("CARGO_BIN_EXE_jzmatrix"))
        .env("HOME", directory.path())
        .env("APPDATA", directory.path())
        .env("LOCALAPPDATA", directory.path())
        .env("XDG_DATA_HOME", directory.path())
        .args(["doctor", "--json"])
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

#[test]
fn local_group_create_is_idempotent_and_does_not_start_tools() {
    let directory = tempfile::tempdir().expect("temporary app data directory");
    let args = [
        "group",
        "create",
        "--goal",
        "整理一份可复核的本地工作包",
        "--template",
        "research",
        "--idempotency-key",
        "test-group-create-001",
        "--json",
    ];
    let first = Command::new(env!("CARGO_BIN_EXE_jzmatrix"))
        .env("HOME", directory.path())
        .env("APPDATA", directory.path())
        .env("LOCALAPPDATA", directory.path())
        .env("XDG_DATA_HOME", directory.path())
        .args(args)
        .output()
        .expect("create local group");
    assert!(
        first.status.success(),
        "first create failed: {:?}",
        first.status
    );
    let first_json: serde_json::Value =
        serde_json::from_slice(&first.stdout).expect("first create output must be JSON");
    assert_eq!(first_json["command"], "group.create");
    assert_eq!(first_json["data"]["data_source"], "demo");
    assert_eq!(first_json["data"]["template_id"], "research");
    let local_fact = first_json["data"]["facts"]
        .as_array()
        .expect("group facts array")
        .iter()
        .find(|fact| fact["kind"] == "local_written")
        .expect("local_written fact");
    assert_eq!(local_fact["state"], "observed");
    assert_eq!(first_json["extensions"]["external_processes"], false);
    assert_eq!(first_json["extensions"]["replayed"], false);

    let second = Command::new(env!("CARGO_BIN_EXE_jzmatrix"))
        .env("HOME", directory.path())
        .env("APPDATA", directory.path())
        .env("LOCALAPPDATA", directory.path())
        .env("XDG_DATA_HOME", directory.path())
        .args(args)
        .output()
        .expect("replay local group");
    assert!(
        second.status.success(),
        "replay failed: {:?}",
        second.status
    );
    let second_json: serde_json::Value =
        serde_json::from_slice(&second.stdout).expect("replay output must be JSON");
    assert_eq!(second_json["extensions"]["replayed"], true);
    assert_eq!(second_json["data"]["id"], first_json["data"]["id"]);

    let conflict = Command::new(env!("CARGO_BIN_EXE_jzmatrix"))
        .env("HOME", directory.path())
        .env("APPDATA", directory.path())
        .env("LOCALAPPDATA", directory.path())
        .env("XDG_DATA_HOME", directory.path())
        .args([
            "group",
            "create",
            "--goal",
            "另一件事",
            "--template",
            "research",
            "--idempotency-key",
            "test-group-create-001",
            "--json",
        ])
        .output()
        .expect("run conflicting local group");
    assert_eq!(conflict.status.code(), Some(2));
    let conflict_json: serde_json::Value =
        serde_json::from_slice(&conflict.stdout).expect("conflict output must be JSON");
    assert_eq!(conflict_json["errors"][0]["code"], "idempotency_conflict");
    assert!(!String::from_utf8_lossy(&conflict.stdout).contains("另一件事"));
}
