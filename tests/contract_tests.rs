use std::fs;

use qubit_infra_ci::{Config, Task, ToolSpec, jobs, load_tools};
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
fn tool_spec_is_constructible_for_workflow_consumers() {
    let tool = ToolSpec {
        source: "source".into(),
        revision: "revision".into(),
        binary: "binary".into(),
        package: "package".into(),
    };
    assert_eq!(tool.binary, "binary");
}
