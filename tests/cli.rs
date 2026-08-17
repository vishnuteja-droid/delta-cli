//! Integration tests covering init -> new -> status and the
//! stale-detection path, driving the real `dlt` binary as a black box.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;

use assert_cmd::Command;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

fn dlt() -> Command {
    Command::cargo_bin("dlt").expect("dlt binary built by cargo test harness")
}

fn stdout_of(cmd: &mut Command) -> String {
    let output = cmd.output().expect("subprocess spawns");
    assert!(
        output.status.success(),
        "expected success, got {:?}",
        output.status
    );
    String::from_utf8(output.stdout).expect("utf8 stdout")
}

#[test]
fn init_new_status_flow() {
    let dir = TempDir::new().expect("tempdir");

    dlt().current_dir(dir.path()).arg("init").assert().success();

    dlt()
        .current_dir(dir.path())
        .args(["change", "new", "add-widgets"])
        .assert()
        .success();

    let stdout = stdout_of(dlt().current_dir(dir.path()).arg("status"));
    assert!(stdout.contains("add-widgets"), "stdout was: {stdout}");
    assert!(stdout.contains("proposal"), "stdout was: {stdout}");
    assert!(stdout.contains("pending"), "stdout was: {stdout}");
}

#[test]
fn init_twice_fails_with_validation_exit_code() {
    let dir = TempDir::new().expect("tempdir");
    dlt().current_dir(dir.path()).arg("init").assert().success();
    dlt().current_dir(dir.path()).arg("init").assert().code(2);
}

#[test]
fn status_without_init_fails_with_validation_exit_code() {
    let dir = TempDir::new().expect("tempdir");
    dlt().current_dir(dir.path()).arg("status").assert().code(2);
}

#[test]
fn change_new_rejects_invalid_slug() {
    let dir = TempDir::new().expect("tempdir");
    dlt().current_dir(dir.path()).arg("init").assert().success();
    dlt()
        .current_dir(dir.path())
        .args(["change", "new", "Not A Slug"])
        .assert()
        .code(2);
}

#[test]
fn init_notes_existing_openspec_directory() {
    let dir = TempDir::new().expect("tempdir");
    fs::create_dir_all(dir.path().join("openspec")).expect("mkdir openspec");

    let stdout = stdout_of(dlt().current_dir(dir.path()).arg("init"));
    assert!(stdout.contains("openspec"), "stdout was: {stdout}");
    assert!(stdout.contains("not yet supported"), "stdout was: {stdout}");
}

/// A design.md whose stored source_hash no longer matches its input
/// (proposal.md) after the proposal is edited must show up as `stale`
/// in `status`, and must block `archive` with exit code 4.
#[test]
fn stale_design_blocks_archive_with_exit_code_4() {
    let dir = TempDir::new().expect("tempdir");
    dlt().current_dir(dir.path()).arg("init").assert().success();
    dlt()
        .current_dir(dir.path())
        .args(["change", "new", "rework-auth"])
        .assert()
        .success();

    let proposal_path = dir.path().join(".delta/changes/rework-auth/proposal.md");
    let proposal_text = fs::read_to_string(&proposal_path).expect("read proposal");
    let proposal_body = proposal_text
        .split_once("\n---\n")
        .expect("proposal has frontmatter delimiter")
        .1;

    let mut hasher = Sha256::new();
    hasher.update(proposal_body.as_bytes());
    let hash: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();

    let design_path = dir.path().join(".delta/changes/rework-auth/design.md");
    let design_text = format!(
        "---\nstage: design\ncreated: 2026-01-01T00:00:00Z\nupdated: 2026-01-01T00:00:00Z\nsource_hash: \"{hash}\"\nstatus: valid\n---\nSome design body.\n"
    );
    fs::write(&design_path, design_text).expect("write design.md");

    let stdout = stdout_of(dlt().current_dir(dir.path()).arg("status"));
    assert!(stdout.contains("design"), "stdout was: {stdout}");
    assert!(stdout.contains("valid"), "stdout was: {stdout}");

    fs::write(&proposal_path, proposal_text.replace("TODO", "CHANGED")).expect("edit proposal");

    let stdout = stdout_of(dlt().current_dir(dir.path()).arg("status"));
    assert!(stdout.contains("stale"), "stdout was: {stdout}");

    dlt()
        .current_dir(dir.path())
        .args(["archive", "rework-auth"])
        .assert()
        .code(4);
}

/// The end-to-end answer to "how do I tell dlt what feature to
/// change": `--description` seeds the placeholder proposal, and
/// `dlt run proposal --dry-run` (no provider call, no API key needed)
/// must actually include it in the assembled prompt — using the real
/// seeded `stages/proposal.yaml`, not a test fixture.
#[test]
fn change_new_description_reaches_the_dry_run_prompt() {
    let dir = TempDir::new().expect("tempdir");
    dlt().current_dir(dir.path()).arg("init").assert().success();

    fs::write(
        dir.path().join(".delta/config.toml"),
        "[providers.default]\nkind = \"gemini\"\nbase_url = \"https://example.invalid\"\nmodel = \"test-model\"\napi_key_env = \"TEST_KEY_NOT_SET\"\n",
    )
    .expect("write provider config");

    dlt()
        .current_dir(dir.path())
        .args([
            "change",
            "new",
            "add-healthcheck",
            "--description",
            "Add a health check endpoint at /healthz.",
        ])
        .assert()
        .success();

    let stdout = stdout_of(dlt().current_dir(dir.path()).args([
        "run",
        "proposal",
        "--change",
        "add-healthcheck",
        "--dry-run",
    ]));
    assert!(
        stdout.contains("Add a health check endpoint at /healthz."),
        "stdout was: {stdout}"
    );
}

#[test]
fn archive_moves_change_and_applies_deltas_to_truth() {
    let dir = TempDir::new().expect("tempdir");
    dlt().current_dir(dir.path()).arg("init").assert().success();
    dlt()
        .current_dir(dir.path())
        .args(["change", "new", "add-widgets"])
        .assert()
        .success();

    fs::create_dir_all(dir.path().join(".delta/changes/add-widgets/deltas")).expect("mkdir deltas");
    fs::write(
        dir.path()
            .join(".delta/changes/add-widgets/deltas/widgets.md"),
        "Widgets can now be created.\n",
    )
    .expect("write delta");

    dlt()
        .current_dir(dir.path())
        .args(["archive", "add-widgets"])
        .assert()
        .success();

    assert!(!dir.path().join(".delta/changes/add-widgets").exists());
    assert!(dir.path().join(".delta/archive/add-widgets").exists());
    let truth = fs::read_to_string(dir.path().join(".delta/truth/widgets.md")).expect("read truth");
    assert_eq!(truth, "Widgets can now be created.\n");
}
