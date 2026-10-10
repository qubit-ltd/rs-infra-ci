// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::process::Output;

use qubit_infra_ci::Config;
use tempfile::TempDir;
use tempfile::tempdir;

/// Creates executable process fixtures without changing the test process
/// environment.
fn fixture(config: &str) -> TempDir {
    let dir = tempdir().expect("fixture");
    fs::create_dir_all(dir.path().join(".infra/ci")).expect("configuration directory");
    fs::create_dir_all(dir.path().join(".infra/tools")).expect("tools configuration directory");
    fs::write(dir.path().join(".infra/ci/ci.toml"), config).expect("configuration");
    fs::write(
        dir.path().join(".infra/tools/defaults.toml"),
        include_str!("../conf/defaults.toml"),
    )
    .expect("shared defaults");
    script(
        &dir,
        "cargo",
        r#"#!/bin/sh
printf '%s|%s|%s|%s\n' "$*" "$RUSTFLAGS" "$RUSTDOCFLAGS" "$CARGO_TARGET_DIR" >> "$PWD/raw-calls"
case "$1" in +*) shift;; esac
printf '%s|%s|%s|%s\n' "$*" "$RUSTFLAGS" "$RUSTDOCFLAGS" "$CARGO_TARGET_DIR" >> "$PWD/calls"
if [ "$1" = audit ] && [ -f audit-network ]; then
  case "$*" in *--no-fetch*) exit 0;; esac
  echo "couldn't fetch advisory database" >&2
  exit 1
fi
if [ "$1" = fuzz ] && [ "$2" = --version ]; then
  if [ -f fuzz-wrong-version ]; then echo 'cargo-fuzz 0.12.0'; else echo 'cargo-fuzz 0.13.2'; fi
  exit 0
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
fn run(dir: &TempDir, operation: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rs-infra-ci"))
        .args(["--project", dir.path().to_str().expect("path"), operation])
        .env(
            "PATH",
            format!("{}:{}", dir.path().display(), std::env::var("PATH").expect("PATH")),
        )
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .env("RUSTFLAGS", "original")
        .output()
        .expect("CLI")
}

/// Runs a matrix subcommand against the fixture with its fake Cargo executable.
fn run_matrix(dir: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_rs-infra-ci"))
        .args(["--project", dir.path().to_str().expect("UTF-8 path"), "matrix"])
        .args(args)
        .env(
            "PATH",
            format!("{}:{}", dir.path().display(), std::env::var("PATH").expect("PATH")),
        )
        .output()
        .expect("matrix CLI")
}

#[test]
fn test_matrix_plan_writes_validated_github_matrix() {
    let dir = fixture("tasks = ['feature-matrix']\n");
    fs::write(dir.path().join(".infra/ci/cargo-matrix.json"), r#"{"version":1,"checks":[{"name":"default","commands":["check"]},{"name":"lint","commands":["clippy","test"]}]}"#).expect("matrix");
    let output_path = dir.path().join("matrix-output.json");
    let output = run_matrix(&dir, &["plan", "--output", output_path.to_str().expect("UTF-8 path")]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(
        output.stdout.is_empty(),
        "machine-readable plan should not print a banner"
    );
    let matrix: serde_json::Value =
        serde_json::from_slice(&fs::read(output_path).expect("matrix output")).expect("valid JSON");
    assert_eq!(
        matrix,
        serde_json::json!({"include":[{"name":"default","needs_clippy":false},{"name":"lint","needs_clippy":true}]})
    );

    fs::write(
        dir.path().join(".infra/ci/cargo-matrix.json"),
        r#"{"version":1,"checks":[]}"#,
    )
    .expect("empty matrix");
    let empty = run_matrix(&dir, &["plan"]);
    assert!(empty.status.success(), "{}", String::from_utf8_lossy(&empty.stderr));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&empty.stdout).expect("empty JSON"),
        serde_json::json!({"include":[]})
    );

    fs::write(
        dir.path().join(".infra/ci/cargo-matrix.json"),
        r#"{"version":1,"checks":[{"name":"same","commands":["check"]},{"name":"same","commands":["test"]}]}"#,
    )
    .expect("invalid matrix");
    let invalid = run_matrix(&dir, &["plan"]);
    assert!(!invalid.status.success());
    assert!(String::from_utf8_lossy(&invalid.stderr).contains("duplicate matrix check name"));
}

#[test]
fn test_matrix_run_selects_one_check_and_restores_lock_on_failure() {
    let dir = fixture("tasks = ['feature-matrix']\n");
    fs::write(dir.path().join("Cargo.lock"), "baseline").expect("lock");
    fs::write(dir.path().join(".infra/ci/cargo-matrix.json"), r#"{"version":1,"checks":[{"name":"plain","commands":["check"]},{"name":"dependency","commands":["test"],"dependency":{"name":"dep","resolution":"precise","version":"1.2.3"}}]}"#).expect("matrix");

    let plain = run_matrix(&dir, &["run", "--check", "plain"]);
    assert!(plain.status.success(), "{}", String::from_utf8_lossy(&plain.stderr));
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("check --workspace"));
    assert!(!calls.contains("update --package dep"));

    let unknown = run_matrix(&dir, &["run", "--check", "missing"]);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown matrix check"));
    assert_eq!(fs::read_to_string(dir.path().join("calls")).expect("calls"), calls);

    fs::write(dir.path().join("fail"), "").expect("failure marker");
    let failed = run_matrix(&dir, &["run", "--check", "dependency"]);
    assert!(!failed.status.success());
    assert_eq!(
        fs::read_to_string(dir.path().join("Cargo.lock")).expect("restored lock"),
        "baseline"
    );
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("update --package dep --precise 1.2.3"));
    assert!(calls.contains("test --workspace --locked"));
}

#[test]
fn test_matrix_plan_rejects_more_than_github_job_limit() {
    let dir = fixture("tasks = ['feature-matrix']\n");
    let checks: Vec<_> = (0..257)
        .map(|index| serde_json::json!({"name": format!("check-{index}"), "commands": ["check"]}))
        .collect();
    fs::write(
        dir.path().join(".infra/ci/cargo-matrix.json"),
        serde_json::to_vec(&serde_json::json!({"version": 1, "checks": checks})).expect("matrix JSON"),
    )
    .expect("matrix");

    let output = run_matrix(&dir, &["plan"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("at most 256"));
}

#[test]
fn test_matrix_run_validates_all_checks_before_executing_selected_one() {
    let dir = fixture("tasks = ['feature-matrix']\n");
    fs::write(
        dir.path().join(".infra/ci/cargo-matrix.json"),
        r#"{"version":1,"checks":[{"name":"first","commands":["check"]},{"name":"broken","commands":["unknown"]}]}"#,
    )
    .expect("matrix");
    let output = run_matrix(&dir, &["run", "--check", "first"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid matrix commands"));
    assert!(!dir.path().join("calls").exists());
}

/// Verifies the opt-in coverage flag reaches only the coverage subprocess.
#[test]
fn test_coverage_threshold_override_reaches_coverage_collector() {
    let dir = fixture("tasks = ['coverage']\n");
    script(
        &dir,
        "rs-infra-coverage",
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> calls\nprintf '%s|%s|%s\\n' \"${CARGO_INCREMENTAL:-}\" \"${RUSTFLAGS:-}\" \"${CARGO_ENCODED_RUSTFLAGS:-}\" >> coverage-env\ncase \"$*\" in *--ignore-thresholds) exit 0;; *) exit 7;; esac\n",
    );
    let strict = run(&dir, "check");
    assert!(!strict.status.success(), "the default must enforce coverage thresholds");

    let ignored = Command::new(env!("CARGO_BIN_EXE_rs-infra-ci"))
        .args([
            "--project",
            dir.path().to_str().expect("UTF-8 path"),
            "--ignore-coverage-thresholds",
            "check",
        ])
        .env(
            "PATH",
            format!("{}:{}", dir.path().display(), std::env::var("PATH").expect("PATH")),
        )
        .output()
        .expect("run CI with ignored coverage thresholds");
    assert!(ignored.status.success(), "{}", String::from_utf8_lossy(&ignored.stderr));
    let encoded = Command::new(env!("CARGO_BIN_EXE_rs-infra-ci"))
        .args([
            "--project",
            dir.path().to_str().expect("UTF-8 path"),
            "--ignore-coverage-thresholds",
            "check",
        ])
        .env(
            "PATH",
            format!("{}:{}", dir.path().display(), std::env::var("PATH").expect("PATH")),
        )
        .env("CARGO_ENCODED_RUSTFLAGS", "--cfg inherited")
        .env_remove("RUSTFLAGS")
        .output()
        .expect("run CI with encoded Rust flags");
    assert!(encoded.status.success(), "{}", String::from_utf8_lossy(&encoded.stderr));
    let calls = fs::read_to_string(dir.path().join("calls")).expect("coverage calls");
    assert!(calls.contains("collect --ignore-thresholds"));
    let coverage_env = fs::read_to_string(dir.path().join("coverage-env")).expect("coverage environment");
    assert!(
        coverage_env
            .lines()
            .next()
            .is_some_and(|line| line.starts_with("1|original -Clink-dead-code|")),
        "coverage must enable incremental compilation and preserve inherited Rust flags: {coverage_env}"
    );
    assert!(
        coverage_env
            .lines()
            .nth(2)
            .is_some_and(|line| line == "1||--cfg inherited\x1f-Clink-dead-code"),
        "coverage must append to CARGO_ENCODED_RUSTFLAGS: {coverage_env}"
    );
}

#[test]
fn test_clippy_and_coverage_cfg_are_strict_and_scoped() {
    let dir = fixture("tasks = ['clippy', 'coverage-cfg-clippy', 'audit']\n[local]\ncoverage_cfg_clippy = true\n");
    let output = run(&dir, "check");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(String::from_utf8_lossy(&output.stdout).contains("✅ rs-infra-ci: check succeeded"));
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("clippy --workspace --all-targets --all-features -- -D warnings|original|"));
    assert!(calls.contains("clippy --workspace --all-targets --all-features -- -D warnings|--cfg coverage|"));
    assert!(calls.contains("audit|original|"));
}

#[test]
fn test_matrix_preserves_feature_package_doc_and_clippy_semantics() {
    let dir = fixture("tasks = ['feature-matrix']\n");
    fs::write(dir.path().join(".infra/ci/cargo-matrix.json"), r#"{"version":1,"checks":[{"name":"minimal","commands":["test","doc","doc-test","clippy"],"defaultFeatures":false,"features":["regex"],"packages":["one"]},{"name":"all","commands":["check"],"allFeatures":true}]}"#).expect("matrix");
    let output = run(&dir, "check");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert!(calls.contains("test --package one --no-default-features --features regex"));
    assert!(calls.contains("test --doc --package one"));
    assert!(calls.contains("clippy --all-targets --package one --no-default-features --features regex -- -D warnings"));
    assert!(calls.contains("|-D warnings|"));
    assert!(calls.contains("check --workspace --all-features"));
    assert!(calls.contains("target/infra-feature-matrix/minimal"));
}

#[test]
fn test_empty_matrix_skips_compatibility_checks() {
    let dir = fixture("tasks = ['feature-matrix']\n");
    fs::write(
        dir.path().join(".infra/ci/cargo-matrix.json"),
        r#"{"version":1,"checks":[]}"#,
    )
    .expect("empty matrix");
    let output = run(&dir, "check");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!dir.path().join("calls").exists());
}

#[test]
fn test_dependency_matrix_restores_lock_on_failure_and_stops_hook() {
    let dir = fixture("tasks = ['feature-matrix', 'project-hook']\n");
    fs::write(dir.path().join("Cargo.lock"), "baseline").expect("lock");
    fs::write(dir.path().join("fail"), "").expect("failure marker");
    fs::write(dir.path().join(".infra/ci/cargo-matrix.json"), r#"{"version":1,"checks":[{"name":"old","commands":["test"],"dependency":{"name":"dep","resolution":"precise","version":"1.2.3"}}]}"#).expect("matrix");
    script(&dir, "project-ci-check.sh", "#!/bin/sh\ntouch hook-ran\n");
    let output = run(&dir, "check");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("❌ rs-infra-ci: check failed:"));
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
fn test_advanced_suites_prepare_tools_and_forward_suite_configuration() {
    let dir = fixture(
        "tasks = ['miri', 'address-sanitizer', 'fuzz', 'loom']\n[local]\nbuild_toolchain = '1.94.0'\nnightly_toolchain = 'nightly-2026-06-05'\nfuzz_mode = 'build-only'\nfuzz_seconds_per_target = 17\nfuzz_max_len = 16384\nfuzz_version = '0.13.2'\n",
    );
    script(
        &dir,
        "rustup",
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$PWD/rustup-calls\"\n",
    );
    script(
        &dir,
        "rs-infra-verify",
        "#!/bin/sh\nprintf '%s|%s|%s|%s|%s|%s\\n' \"$*\" \"$RS_INFRA_NIGHTLY_TOOLCHAIN\" \"$RS_INFRA_FUZZ_MODE\" \"$RS_INFRA_FUZZ_SECONDS_PER_TARGET\" \"$RS_INFRA_FUZZ_MAX_LEN\" \"$RUSTFLAGS\" >> \"$PWD/verify-calls\"\n",
    );
    let output = run(&dir, "check");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let calls = fs::read_to_string(dir.path().join("calls")).expect("Cargo calls");
    assert!(calls.contains("fuzz --version"));
    let raw_calls = fs::read_to_string(dir.path().join("raw-calls")).expect("raw Cargo calls");
    assert!(raw_calls.contains("+nightly-2026-06-05 miri setup"));
    assert!(!raw_calls.contains("+1.94.0 +nightly-2026-06-05 miri setup"));
    let rustup_calls = fs::read_to_string(dir.path().join("rustup-calls")).expect("rustup calls");
    assert!(rustup_calls.contains("toolchain install nightly-2026-06-05"));
    let verify_calls = fs::read_to_string(dir.path().join("verify-calls")).expect("suite calls");
    assert!(verify_calls.contains("--suite miri|nightly-2026-06-05||||"));
    assert!(verify_calls.contains("--suite address-sanitizer|nightly-2026-06-05||||"));
    assert!(verify_calls.contains("--suite fuzz|nightly-2026-06-05|build-only|17|16384"));
    assert!(verify_calls.contains("--suite loom|nightly-2026-06-05||||--cfg loom"));
    assert!(!calls.contains("install cargo-fuzz"));
}

#[test]
fn test_strict_docs_readme_and_release_build_keep_legacy_local_checks() {
    let dir =
        fixture("tasks = ['verify', 'strict-doc', 'readme', 'release-build']\n[local]\nbuild_toolchain = '1.94.0'\n");
    script(
        &dir,
        "rs-infra-verify",
        "#!/bin/sh\nprintf '%s|%s|%s\\n' \"$*\" \"$RUSTUP_TOOLCHAIN\" \"$RUSTDOCFLAGS\" >> \"$PWD/verify-calls\"\n",
    );

    let output = run(&dir, "check");

    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let calls = fs::read_to_string(dir.path().join("raw-calls")).expect("Cargo calls");
    assert_eq!(
        calls
            .lines()
            .filter(|line| line.starts_with("+1.94.0 build --release --verbose"))
            .count(),
        1
    );
    let verify_calls = fs::read_to_string(dir.path().join("verify-calls")).expect("verify calls");
    assert!(verify_calls.contains("run --suite doc|1.94.0|-D warnings -D missing-docs"));
    assert!(verify_calls.contains("run --suite readme|1.94.0|"));
    assert_eq!(
        verify_calls
            .lines()
            .filter(|line| line.contains("run --suite doc|"))
            .count(),
        1
    );
}

#[test]
fn test_fuzz_installs_configured_version_when_existing_version_differs() {
    let dir = fixture("tasks = ['fuzz']\n[local]\nfuzz_version = '0.13.2'\n");
    fs::write(dir.path().join("fuzz-wrong-version"), "").expect("old version marker");
    script(&dir, "rs-infra-verify", "#!/bin/sh\nexit 0\n");
    let output = run(&dir, "check");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let calls = fs::read_to_string(dir.path().join("calls")).expect("Cargo calls");
    assert!(calls.contains("install cargo-fuzz --locked --version 0.13.2"));
}

#[test]
fn test_disabled_fuzz_does_not_install_cargo_fuzz() {
    let dir = fixture("tasks = ['fuzz']\n[local]\nfuzz_mode = 'disabled'\n");
    script(
        &dir,
        "rs-infra-verify",
        "#!/bin/sh\nprintf '%s|%s\\n' \"$*\" \"$RS_INFRA_FUZZ_MODE\" >> \"$PWD/verify-calls\"\n",
    );
    let output = run(&dir, "check");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!dir.path().join("calls").exists());
    let verify_calls = fs::read_to_string(dir.path().join("verify-calls")).expect("suite calls");
    assert!(verify_calls.contains("run --suite fuzz|disabled"));
}

#[test]
fn test_invalid_matrix_fails_before_any_command() {
    for invalid in [
        "{",
        r#"{"version":2,"checks":[]}"#,
        r#"{"version":1,"checks":null}"#,
        r#"{"version":1,"checks":[{"name":3,"commands":["test"]}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":"test"}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":["clean"]}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":["test"],"features":[3]}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":["test"],"features":["bad feature"]}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":["test"],"packages":["bad/name"]}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":["test"],"defaultFeatures":"yes"}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":["test"],"allFeatures":true,"defaultFeatures":false}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":["test"],"packages":[]}]}"#,
        r#"{"version":1,"checks":[{"name":"same","commands":["test"]},{"name":"same","commands":["test"]}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":["test"],"dependency":{"name":"bad/name","resolution":"latest"}}]}"#,
        r#"{"version":1,"checks":[{"name":"bad","commands":["test"],"dependency":{"name":"dep","resolution":"precise","version":"bad/version"}}]}"#,
    ] {
        let dir = fixture("tasks = ['feature-matrix']\n");
        fs::write(dir.path().join(".infra/ci/cargo-matrix.json"), invalid).expect("invalid matrix");
        assert!(!run(&dir, "check").status.success());
        assert!(!dir.path().join("calls").exists());
    }
}

#[test]
fn test_malformed_project_tool_pin_is_ignored_by_dynamic_resolution() {
    let dir = fixture("tasks = ['feature-matrix']\n");
    fs::create_dir_all(dir.path().join(".infra/ci")).expect("tool directory");
    fs::write(
        dir.path().join(".infra/ci/cargo-matrix.json"),
        r#"{"version":1,"checks":[]}"#,
    )
    .expect("empty matrix");
    fs::write(dir.path().join(".infra/ci/tool.toml"), "revision = [\n").expect("tool configuration");

    let output = run(&dir, "check");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!dir.path().join("calls").exists());
}

#[test]
fn test_invalid_local_configuration_fails_before_any_command() {
    for config in [
        "tasks = ['verify']\n[local]\nmatrix = '../Cargo.toml'\n",
        "tasks = ['verify']\n[local]\nnightly_toolchain = 'nightly'\n",
        "tasks = ['verify']\n[local]\nfuzz_mode = 'unknown'\n",
        "tasks = ['verify']\n[local]\nfuzz_seconds_per_target = 0\n",
    ] {
        let dir = fixture(config);
        let output = run(&dir, "check");

        assert!(!output.status.success());
        assert!(!dir.path().join("calls").exists());
    }
}

#[test]
fn test_missing_shared_defaults_fails_before_any_command() {
    let dir = fixture("tasks = ['clippy']\n");
    fs::remove_file(dir.path().join(".infra/tools/defaults.toml"))
        .expect("remove shared defaults");
    assert!(dir.path().join(".infra/ci/ci.toml").is_file());
    assert!(!dir.path().join(".infra/ci/defaults.toml").exists());

    let output = run(&dir, "check");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains(".infra/ci/defaults.toml"), "{stderr}");
    assert!(stderr.contains("run ./update-infra.sh"), "{stderr}");
    assert!(!dir.path().join("calls").exists());
}

#[test]
fn test_shared_defaults_falls_back_to_legacy_path_when_new_path_is_absent() {
    let dir = fixture("tasks = ['clippy']\n");
    fs::remove_file(dir.path().join(".infra/tools/defaults.toml"))
        .expect("remove new shared defaults");
    fs::create_dir_all(dir.path().join(".infra/ci")).expect("legacy configuration directory");
    fs::write(
        dir.path().join(".infra/ci/defaults.toml"),
        include_str!("../conf/defaults.toml"),
    )
    .expect("legacy shared defaults");

    let output = run(&dir, "check");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_invalid_new_shared_defaults_does_not_fall_back_to_legacy_path() {
    let dir = fixture("tasks = ['clippy']\n");
    fs::create_dir_all(dir.path().join(".infra/ci")).expect("legacy configuration directory");
    fs::write(
        dir.path().join(".infra/ci/defaults.toml"),
        include_str!("../conf/defaults.toml"),
    )
    .expect("legacy shared defaults");
    fs::write(
        dir.path().join(".infra/tools/defaults.toml"),
        "build_toolchain = [\n",
    )
    .expect("invalid new shared defaults");

    let output = run(&dir, "check");
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success());
    assert!(stderr.contains(".infra/tools/defaults.toml"), "{stderr}");
    assert!(!dir.path().join("calls").exists());
}

#[test]
fn test_project_cannot_override_shared_toolchain() {
    let dir = fixture("tasks = ['clippy']\n[local]\nclippy_toolchain = 'nightly-2026-10-01'\n");

    let output = run(&dir, "check");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("managed by .infra/tools/defaults.toml")
    );
    assert!(!dir.path().join("calls").exists());
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
            dir.path().file_name().expect("name").to_str().expect("name"),
            "check",
        ])
        .env(
            "PATH",
            format!("{}:{}", dir.path().display(), std::env::var("PATH").expect("PATH")),
        )
        .output()
        .expect("CLI");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let calls = fs::read_to_string(dir.path().join("raw-calls")).expect("raw calls");
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
    script(&dir, "project-ci-check.sh", "#!/bin/sh\necho hook >> calls\n");
    let plan = run(&dir, "plan");
    assert!(plan.status.success());
    assert!(!dir.path().join("calls").exists());
    let plan = String::from_utf8_lossy(&plan.stdout);
    assert!(plan.contains("cargo +nightly-2026-06-05 clippy --all-targets --workspace --all-features -- -D warnings"));
    assert!(run(&dir, "check").status.success());
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    let lines: Vec<_> = calls.lines().collect();
    assert!(lines[0].starts_with("clippy "));
    assert_eq!(lines[1], "hook");
    assert!(lines[2].starts_with("audit|"));
}

#[test]
fn test_default_tasks_place_matrix_and_hook_before_package_and_audit_last() {
    let tasks = Config::default().select(&[]).expect("default tasks");
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
            "dependency",
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
    script(&dir, "rs-infra-verify", "#!/bin/sh\nprintf '%s\\n' \"$*\" >> calls\n");
    script(&dir, "project-ci-check.sh", "#!/bin/sh\necho hook >> calls\n");
    assert!(run(&dir, "check").status.success());
    let calls = fs::read_to_string(dir.path().join("calls")).expect("calls");
    assert_eq!(calls.matches("--suite package").count(), 1);
    assert!(calls.find("hook").expect("hook") < calls.find("--suite package").expect("package"));
    assert!(calls.contains(&format!("--project {} lock check", dir.path().display())));
}
