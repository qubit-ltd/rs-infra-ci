// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::fs;

use qubit_infra_ci::Config;
use qubit_infra_ci::Task;
use qubit_infra_ci::ToolSpec;
use qubit_infra_ci::jobs;
use qubit_infra_ci::load_config;
use qubit_infra_ci::load_tools;
use qubit_infra_ci::plan_workflow;
use qubit_infra_ci::workflow;
use tempfile::tempdir;

const SHARED_DEFAULTS: &str = r#"
build_toolchain = "1.94.0"
clippy_toolchain = "nightly-2026-06-05"
nightly_toolchain = "nightly-2026-06-05"
fuzz_version = "0.13.2"
"#;

#[test]
fn test_verify_job_contains_lock_and_all_verification_suites() {
    let workflow = jobs(&[Task::Verify], &Default::default()).expect("workflow jobs");
    let commands = &workflow[0].commands;

    assert_eq!(commands.len(), 5);
    assert_eq!(commands[0].args, ["lock", "check"]);
    assert_eq!(commands[1].args, ["run", "--suite", "build"]);
    assert_eq!(commands[2].args, ["run", "--suite", "test"]);
    assert_eq!(commands[3].args, ["run", "--suite", "doc"]);
    assert_eq!(commands[4].args, ["run", "--suite", "package"]);
}

#[test]
fn test_workflow_tasks_do_not_require_project_tool_pins() {
    let project = tempdir().expect("temporary project");
    fs::create_dir_all(project.path().join(".infra/dependency")).expect("dependency directory");
    fs::write(
        project.path().join(".infra/dependency/tool.toml"),
        r#"
source = "https://example.invalid/dependency.git"
revision = "0123456789abcdef0123456789abcdef01234567"
binary = "rs-infra-dependency"
package = "qubit-infra-dependency"
"#,
    )
    .expect("tool configuration");

    let tools = load_tools(project.path()).expect("tools load");
    let workflow = jobs(&[Task::Dependency], &tools).expect("workflow jobs");
    assert!(workflow[0].tool.is_none());
}

#[test]
fn test_invalid_tool_pin_is_ignored_by_dynamic_workflow_resolution() {
    let project = tempdir().expect("temporary project");
    fs::create_dir_all(project.path().join(".infra/style")).expect("style directory");
    fs::write(
        project.path().join(".infra/style/tool.toml"),
        "revision = \"not-a-sha\"\n",
    )
    .expect("tool configuration");

    assert!(
        load_tools(project.path())
            .expect("dynamic tool config")
            .tools
            .is_empty()
    );
}

#[test]
fn test_duplicate_jobs_are_rejected() {
    let error = jobs(&[Task::Style, Task::Style], &Default::default()).expect_err("duplicate tasks must be rejected");

    assert!(error.to_string().contains("configured more than once"));
}

#[test]
fn test_task_selection_includes_dependency() {
    let selected = Config {
        tasks: vec![Task::Style, Task::Verify, Task::Coverage, Task::Pages, Task::Dependency],
    }
    .select(&[])
    .expect("task selection");

    assert_eq!(selected.len(), 5);
}

#[test]
fn test_default_task_selection_includes_dependency() {
    let selected = Config::default().select(&[]).expect("default task selection");

    assert!(selected.contains(&Task::Dependency));
}

#[test]
fn test_dependency_job_runs_check_without_sync() {
    let project = tempdir().expect("temporary project");
    fs::create_dir_all(project.path().join(".infra/dependency")).expect("dependency directory");
    fs::write(
        project.path().join(".infra/dependency/tool.toml"),
        r#"
source = "https://example.invalid/dependency.git"
revision = "0123456789abcdef0123456789abcdef01234567"
binary = "rs-infra-dependency"
package = "qubit-infra-dependency"
"#,
    )
    .expect("tool configuration");

    let workflow = jobs(&[Task::Dependency], &load_tools(project.path()).expect("tools load")).expect("workflow jobs");
    let command = &workflow[0].commands[0];

    assert_eq!(command.executable, "rs-infra-dependency");
    assert_eq!(command.args, ["check"]);
    assert!(!command.args.iter().any(|arg| arg == "sync"));
}

#[test]
fn test_tool_spec_is_constructible_for_workflow_consumers() {
    let tool = ToolSpec {
        source: "source".into(),
        revision: "revision".into(),
        binary: "binary".into(),
        package: "package".into(),
    };
    assert_eq!(tool.binary, "binary");
}

#[test]
fn test_project_workflow_uses_ci_configuration_and_can_be_planned() {
    let project = tempdir().expect("temporary project");
    fs::create_dir_all(project.path().join(".infra/ci")).expect("CI directory");
    fs::create_dir_all(project.path().join(".infra/tools")).expect("tools configuration directory");
    fs::write(project.path().join(".infra/ci/ci.toml"), "tasks = ['style']\n").expect("CI configuration");
    fs::write(project.path().join(".infra/tools/defaults.toml"), SHARED_DEFAULTS).expect("shared defaults");

    let config = load_config(project.path()).expect("CI configuration loads");
    assert_eq!(config.tasks, [Task::Style]);

    let jobs = workflow(project.path(), &[Task::Style]).expect("project workflow");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].commands[0].executable, "rs-infra-style");
    assert_eq!(
        jobs[0].commands[0].args,
        [
            "--project".to_owned(),
            project.path().to_string_lossy().into_owned(),
            "check".to_owned(),
        ]
    );

    plan_workflow(project.path(), &[Task::Style]).expect("workflow plan");
}
