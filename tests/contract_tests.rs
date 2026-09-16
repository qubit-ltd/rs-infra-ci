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
use qubit_infra_ci::load_tools;
use tempfile::tempdir;

#[test]
fn verify_job_contains_lock_and_all_verification_suites() {
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
fn all_migration_tasks_have_pinned_install_information() {
    let project = tempdir().expect("temporary project");
    fs::create_dir_all(project.path().join(".infra/ci")).expect("ci directory");
    fs::write(
        project.path().join(".infra/ci/tools.toml"),
        r#"
[rs-infra-dependency]
source = "https://example.invalid/dependency.git"
revision = "0123456789abcdef0123456789abcdef01234567"
binary = "rs-infra-dependency"
package = "qubit-infra-dependency"
"#,
    )
    .expect("tool configuration");

    let tools = load_tools(project.path()).expect("tools load");
    let workflow = jobs(&[Task::Dependency], &tools).expect("workflow jobs");
    let tool = workflow[0].tool.as_ref().expect("dependency tool");

    assert_eq!(
        tool.install_args(),
        vec![
            "install",
            "--git",
            "https://example.invalid/dependency.git",
            "--rev",
            "0123456789abcdef0123456789abcdef01234567",
            "--locked",
            "qubit-infra-dependency",
            "--bin",
            "rs-infra-dependency",
        ]
    );
}

#[test]
fn invalid_tool_revision_is_rejected() {
    let project = tempdir().expect("temporary project");
    fs::create_dir_all(project.path().join(".infra/ci")).expect("ci directory");
    fs::write(
        project.path().join(".infra/ci/tools.toml"),
        "[rs-infra-style]\nrevision = \"not-a-sha\"\n",
    )
    .expect("tool configuration");

    assert!(load_tools(project.path()).is_err());
}

#[test]
fn duplicate_jobs_are_rejected() {
    let error = jobs(&[Task::Style, Task::Style], &Default::default())
        .expect_err("duplicate tasks must be rejected");

    assert!(error.to_string().contains("configured more than once"));
}

#[test]
fn task_selection_includes_dependency() {
    let selected = Config {
        tasks: vec![
            Task::Style,
            Task::Verify,
            Task::Coverage,
            Task::Pages,
            Task::Dependency,
        ],
    }
    .select(&[])
    .expect("task selection");

    assert_eq!(selected.len(), 5);
}

#[test]
fn default_task_selection_includes_dependency() {
    let selected = Config::default()
        .select(&[])
        .expect("default task selection");

    assert!(selected.contains(&Task::Dependency));
}

#[test]
fn dependency_job_runs_check_without_sync() {
    let project = tempdir().expect("temporary project");
    fs::create_dir_all(project.path().join(".infra/ci")).expect("ci directory");
    fs::write(
        project.path().join(".infra/ci/tools.toml"),
        r#"
[rs-infra-dependency]
source = "https://example.invalid/dependency.git"
revision = "0123456789abcdef0123456789abcdef01234567"
binary = "rs-infra-dependency"
package = "qubit-infra-dependency"
"#,
    )
    .expect("tool configuration");

    let workflow = jobs(
        &[Task::Dependency],
        &load_tools(project.path()).expect("tools load"),
    )
    .expect("workflow jobs");
    let command = &workflow[0].commands[0];

    assert_eq!(command.executable, "rs-infra-dependency");
    assert_eq!(command.args, ["check"]);
    assert!(!command.args.iter().any(|arg| arg == "sync"));
}

#[test]
fn tool_spec_is_constructible_for_workflow_consumers() {
    let tool = ToolSpec {
        source: "source".into(),
        revision: "revision".into(),
        binary: "binary".into(),
        package: "package".into(),
    };
    assert_eq!(tool.binary, "binary");
}
