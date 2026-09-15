#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use tempfile::TempDir;

/// Creates executable process fixtures without changing the test process environment.
fn fixture(config: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("fixture");
    fs::create_dir_all(dir.path().join(".infra/ci")).expect("configuration directory");
    fs::write(dir.path().join(".infra/ci.toml"), config).expect("configuration");
    script(
        &dir,
        "cargo",
        r#"#!/bin/sh
printf '%s|%s|%s|%s\n' "$*" "$RUSTFLAGS" "$RUSTDOCFLAGS" "$CARGO_TARGET_DIR" >> "$PWD/calls"
if [ "$1" = audit ] && [ -f audit-network ]; then
  case "$*" in *--no-fetch*) exit 0;; esac
  echo "couldn't fetch advisory database" >&2
  exit 1
fi
if [ "$1" = audit ] && [ -f audit-vulnerability ]; then exit 1; fi
if [ "$1" = update ]; then printf changed > Cargo.lock; fi
if [ "$1" = metadata ]; then echo '{"packages":[{"name":"dep","version":"1.2.3"}]}'; fi
if [ -f fail ] && [ "$1" = test ]; then exit 9; fi
"#,
    );
    dir
}

/// Writes an executable fixture into its isolated project.
fn script(dir: &TempDir, name: &str, text: &str) {
    let path = dir.path().join(name);
    fs::write(&path, text).expect("script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("permissions");
}

/// Runs the real CLI with fixture executables and captures its result.
fn run(dir: &TempDir, operation: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rs-infra-ci"))
        .args(["--project", dir.path().to_str().expect("path"), operation])
        .env(
            "PATH",
            format!(
                "{}:{}",
                dir.path().display(),
                std::env::var("PATH").expect("PATH")
            ),
        )
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("RUSTFLAGS", "original")
        .output()
        .expect("CLI")
}

#[test]
fn test_clippy_and_coverage_cfg_are_strict_and_scoped() {
    let dir = fixture(
        "tasks = ['clippy', 'coverage-cfg-clippy', 'audit']\n[local]\ncoverage_cfg_clippy = true\n",
    );
    let output = run(&dir, "check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(
        calls.contains("clippy --workspace --all-targets --all-features -- -D warnings|original|")
    );
    assert!(calls.contains(
        "clippy --workspace --all-targets --all-features -- -D warnings|--cfg coverage|"
    ));
    assert!(calls.contains("audit|original|"));
}

#[test]
fn test_matrix_preserves_feature_package_doc_and_clippy_semantics() {
    let dir = fixture("tasks = ['feature-matrix']\n");
    fs::write(dir.path().join(".infra/ci/cargo-matrix.json"), r#"{"version":1,"checks":[{"name":"minimal","commands":["test","doc","doc-test","clippy"],"defaultFeatures":false,"features":["regex"],"packages":["one"]},{"name":"all","commands":["check"],"allFeatures":true}]}"#).expect("matrix");
    let output = run(&dir, "check");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("test --package one --no-default-features --features regex"));
    assert!(calls.contains("test --doc --package one"));
    assert!(calls.contains(
        "clippy --all-targets --package one --no-default-features --features regex -- -D warnings"
    ));
    assert!(calls.contains("|-D warnings|"));
    assert!(calls.contains("check --workspace --all-features"));
    assert!(calls.contains("target/infra-feature-matrix/minimal"));
}

#[test]
fn test_dependency_matrix_restores_lock_on_failure_and_stops_hook() {
    let dir = fixture("tasks = ['feature-matrix', 'project-hook']\n");
    fs::write(dir.path().join("Cargo.lock"), "baseline").expect("lock");
    fs::write(dir.path().join("fail"), "").expect("failure marker");
    fs::write(dir.path().join(".infra/ci/cargo-matrix.json"), r#"{"version":1,"checks":[{"name":"old","commands":["test"],"dependency":{"name":"dep","resolution":"precise","version":"1.2.3"}}]}"#).expect("matrix");
    script(&dir, "project-ci-check.sh", "#!/bin/sh\ntouch hook-ran\n");
    assert!(!run(&dir, "check").status.success());
    assert!(
        fs::read_to_string(dir.path().join("calls"))
            .expect("matrix executed")
            .contains("update --package dep --precise 1.2.3")
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("Cargo.lock")).expect("lock"),
        "baseline"
    );
    assert!(!dir.path().join("hook-ran").exists());
}

#[test]
fn test_hook_is_optional_but_must_be_executable_and_propagates_failure() {
    let dir = fixture("tasks = ['project-hook']\n");
    assert!(run(&dir, "check").status.success());
    fs::write(dir.path().join("project-ci-check.sh"), "exit 0").expect("hook");
    assert!(!run(&dir, "check").status.success());
    script(
        &dir,
        "project-ci-check.sh",
        "#!/bin/sh\nprintf '%s' \"$PWD\" > hook-root\nexit 7\n",
    );
    assert!(!run(&dir, "check").status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("hook-root")).expect("root"),
        dir.path().to_str().expect("path")
    );
}

#[test]
fn test_audit_only_retries_database_fetch_failures() {
    let dir = fixture("tasks = ['audit']\n");
    fs::write(dir.path().join("audit-network"), "").expect("network marker");
    assert!(run(&dir, "check").status.success());
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("audit --no-fetch --stale"));
    let vulnerable = fixture("tasks = ['audit']\n");
    fs::write(vulnerable.path().join("audit-vulnerability"), "").expect("vulnerability marker");
    assert!(!run(&vulnerable, "check").status.success());
    assert!(
        !fs::read_to_string(vulnerable.path().join("calls"))
            .expect("calls")
            .contains("--no-fetch")
    );
}

#[test]
fn test_disabled_coverage_cfg_and_missing_matrix_skip_without_commands() {
    let dir = fixture("tasks = ['coverage-cfg-clippy', 'feature-matrix']\n");
    assert!(run(&dir, "check").status.success());
    assert!(!dir.path().join("calls").exists());
}

#[test]
fn test_invalid_matrix_fails_before_any_command() {
    for invalid in [
        r#"{"version":1,"checks":[{"name":"bad","commands":["clean"]}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":["test"],"allFeatures":true,"defaultFeatures":false}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":["test"],"packages":[]}]}"#,
        r#"{"version":1,"checks":[{"name":"same","commands":["test"]},{"name":"same","commands":["test"]}]}"#,
    ] {
        let dir = fixture("tasks = ['feature-matrix']\n");
        fs::write(dir.path().join(".infra/ci/cargo-matrix.json"), invalid).expect("invalid matrix");
        assert!(!run(&dir, "check").status.success());
        assert!(!dir.path().join("calls").exists());
    }
}

#[test]
fn test_audit_cache_fallback_can_be_disabled() {
    let dir = fixture("tasks = ['audit']\n[local]\naudit_cached_fallback = false\n");
    fs::write(dir.path().join("audit-network"), "").expect("marker");
    assert!(!run(&dir, "check").status.success());
    assert!(
        !fs::read_to_string(dir.path().join("calls"))
            .expect("calls")
            .contains("--no-fetch")
    );
}

#[test]
fn test_relative_project_and_selected_toolchain_are_used() {
    let dir = fixture(
        "tasks = ['clippy', 'audit']\n[local]\nbuild_toolchain = '1.94.0'\nclippy_toolchain = 'nightly-2026-06-05'\n",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_rs-infra-ci"))
        .current_dir(dir.path().parent().expect("parent"))
        .args([
            "--project",
            dir.path()
                .file_name()
                .expect("name")
                .to_str()
                .expect("name"),
            "check",
        ])
        .env(
            "PATH",
            format!(
                "{}:{}",
                dir.path().display(),
                std::env::var("PATH").expect("PATH")
            ),
        )
        .output()
        .expect("CLI");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("+nightly-2026-06-05 clippy"));
    assert!(calls.contains("+1.94.0 audit"));
}

#[test]
fn test_dependency_matrix_checks_resolution_and_restores_missing_lock() {
    let dir = fixture("tasks = ['feature-matrix']\n");
    fs::write(dir.path().join(".infra/ci/cargo-matrix.json"), r#"{"version":1,"checks":[{"name":"wrong-version","commands":["test"],"dependency":{"name":"dep","resolution":"precise","version":"9.9.9"}}]}"#).expect("matrix");
    let output = run(&dir, "check");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected version"));
    assert!(!dir.path().join("Cargo.lock").exists());
    assert!(
        !fs::read_to_string(dir.path().join("calls"))
            .expect("calls")
            .contains("test --")
    );
}

#[test]
fn test_plan_and_check_include_the_same_matrix_commands_without_plan_side_effects() {
    let dir = fixture("tasks = ['feature-matrix', 'project-hook', 'audit']\n");
    fs::write(
        dir.path().join(".infra/ci/cargo-matrix.json"),
        r#"{"version":1,"checks":[{"name":"all","commands":["clippy"],"allFeatures":true}]}"#,
    )
    .expect("matrix");
    script(
        &dir,
        "project-ci-check.sh",
        "#!/bin/sh\necho hook >> calls\n",
    );
    let plan = run(&dir, "plan");
    assert!(plan.status.success());
    assert!(!dir.path().join("calls").exists());
    let plan = String::from_utf8_lossy(&plan.stdout);
    assert!(plan.contains("cargo clippy --all-targets --workspace --all-features -- -D warnings"));
    assert!(run(&dir, "check").status.success());
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    let lines: Vec<_> = calls.lines().collect();
    assert!(lines[0].starts_with("clippy "));
    assert_eq!(lines[1], "hook");
    assert!(lines[2].starts_with("audit|"));
}

#[test]
fn test_default_tasks_place_matrix_and_hook_before_package_and_audit_last() {
    let tasks = qubit_infra_ci::Config::default()
        .select(&[])
        .expect("default tasks");
    let names: Vec<_> = tasks.iter().map(ToString::to_string).collect();
    assert_eq!(
        names,
        [
            "style",
            "clippy",
            "coverage-cfg-clippy",
            "verify",
            "feature-matrix",
            "project-hook",
            "package",
            "coverage",
            "audit"
        ]
    );
}

#[test]
fn test_dependency_named_metadata_still_runs_update() {
    let dir = fixture("tasks = ['feature-matrix']\n");
    fs::write(dir.path().join(".infra/ci/cargo-matrix.json"), r#"{"version":1,"checks":[{"name":"metadata-dep","commands":["check"],"dependency":{"name":"metadata","resolution":"latest"}}]}"#).expect("matrix");
    let output = run(&dir, "check");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected version"));
}

#[test]
fn test_explicit_package_runs_once_after_hook_and_infra_receives_project() {
    let dir = fixture("tasks = ['verify', 'project-hook', 'package']\n");
    script(
        &dir,
        "rs-infra-verify",
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> calls\n",
    );
    script(
        &dir,
        "project-ci-check.sh",
        "#!/bin/sh\necho hook >> calls\n",
    );
    assert!(run(&dir, "check").status.success());
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert_eq!(calls.matches("--suite package").count(), 1);
    assert!(calls.find("hook").expect("hook") < calls.find("--suite package").expect("package"));
    assert!(calls.contains(&format!("--project {} lock check", dir.path().display())));
}
