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
