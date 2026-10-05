// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;

use crate::CommandSpec;
use crate::Config;
use crate::JobSpec;
use crate::Task;
use crate::ToolConfig;
use crate::config::validate_unique;
use crate::local;
use crate::local_config::LocalConfig;

/// Returns an empty tool specification set because tools are resolved by the
/// shared runtime before this workflow starts.
///
/// # Parameters
///
/// * `project` - The project root (retained for API compatibility).
///
/// # Returns
///
/// An empty configuration; no tool pins are stored in project repositories.
///
/// # Errors
///
/// This operation currently cannot fail.
pub fn load_tools(project: &Path) -> Result<ToolConfig> {
    let _ = project;
    Ok(ToolConfig::default())
}

/// Builds workflow jobs for the selected tasks and installed tools.
///
/// # Parameters
///
/// * `tasks` - Tasks to represent, in execution order.
/// * `tools` - Tool specifications keyed by executable name.
///
/// # Returns
///
/// A static job template for each selected task. Use `workflow` to expand
/// project-specific matrices, hook paths, optional checks, and environment
/// values.
///
/// # Errors
///
/// Returns an error when `tasks` contains a duplicate entry.
pub fn jobs(tasks: &[Task], tools: &ToolConfig) -> Result<Vec<JobSpec>> {
    validate_unique(tasks)?;
    tasks
        .iter()
        .map(|task| {
            let tool_name = task.executable();
            let tool = tools.tools.get(tool_name).cloned();
            let mut job = JobSpec {
                name: task.to_string(),
                task: *task,
                commands: task
                    .commands()
                    .iter()
                    .map(|args| CommandSpec {
                        executable: tool_name.to_owned(),
                        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
                        env: Default::default(),
                    })
                    .collect(),
                tool,
            };
            if *task == Task::Coverage {
                let encoded_flags = match std::env::var("CARGO_ENCODED_RUSTFLAGS") {
                    Ok(mut flags) => {
                        if !flags.is_empty() {
                            flags.push('\x1f');
                        }
                        flags.push_str("-Clink-dead-code");
                        Some(flags)
                    }
                    Err(std::env::VarError::NotPresent) => None,
                    Err(std::env::VarError::NotUnicode(_)) => {
                        bail!("CARGO_ENCODED_RUSTFLAGS must be valid Unicode for coverage")
                    }
                };
                let rust_flags = if let Some(flags) = encoded_flags {
                    ("CARGO_ENCODED_RUSTFLAGS", flags)
                } else {
                    let mut flags = match std::env::var("RUSTFLAGS") {
                        Ok(flags) => flags,
                        Err(std::env::VarError::NotPresent) => String::new(),
                        Err(std::env::VarError::NotUnicode(_)) => {
                            bail!("RUSTFLAGS must be valid Unicode for coverage")
                        }
                    };
                    if !flags.is_empty() {
                        flags.push(' ');
                    }
                    flags.push_str("-Clink-dead-code");
                    ("RUSTFLAGS", flags)
                };
                for command in &mut job.commands {
                    command.env.insert("CARGO_INCREMENTAL".into(), "1".into());
                    command.env.insert(rust_flags.0.into(), rust_flags.1.clone());
                }
            }
            Ok(job)
        })
        .collect()
}

/// Loads project tool pins and builds the selected workflow jobs.
///
/// # Parameters
///
/// * `project` - The project root from which tool pins are loaded.
/// * `tasks` - Tasks to include in the workflow.
///
/// # Returns
///
/// The generated jobs in task order.
///
/// # Errors
///
/// Propagates tool-file parsing, validation, and duplicate-task errors.
pub fn workflow(project: &Path, tasks: &[Task]) -> Result<Vec<JobSpec>> {
    let project = project.canonicalize().context("invalid project directory")?;
    let config = LocalConfig::load(&project)?;
    let mut jobs = jobs(tasks, &load_tools(&project)?)?;
    for job in &mut jobs {
        // An explicit package task determines its position; verify alone retains
        // packaging.
        if job.task == Task::Verify && tasks.contains(&Task::Package) {
            job.commands
                .retain(|command| command.args != ["run", "--suite", "package"]);
        }
        if job.task == Task::Verify && tasks.contains(&Task::StrictDoc) {
            job.commands.retain(|command| command.args != ["run", "--suite", "doc"]);
        }
        if let Some(commands) = local::commands(&project, job.task, &config)? {
            job.commands = commands;
        }
        for command in &mut job.commands {
            if command.executable.starts_with("rs-infra-") {
                command
                    .args
                    .splice(0..0, ["--project".into(), project.to_string_lossy().into_owned()]);
                if let Some(toolchain) = &config.build_toolchain {
                    command.env.insert("RUSTUP_TOOLCHAIN".into(), toolchain.clone());
                }
            }
        }
    }
    Ok(jobs)
}

/// Prints a migration-friendly workflow plan, including tool installation data.
///
/// # Parameters
///
/// * `project` - The project root whose configuration and tools are used.
/// * `tasks` - Tasks to include in the printed plan.
///
/// # Errors
///
/// Propagates configuration, tool-file, and duplicate-task errors.
pub fn plan_workflow(project: &Path, tasks: &[Task]) -> Result<()> {
    println!("project: {}", project.display());
    for job in workflow(project, tasks)? {
        println!("job: {}", job.name);
        if let Some(tool) = job.tool {
            println!("install: cargo {}", tool.install_args().join(" "));
        }
        if job.commands.is_empty() {
            println!("skip: {} is not configured", job.name);
        }
        for command in job.commands {
            println!(
                "run: {} {} env={:?}",
                command.executable,
                command.args.join(" "),
                command.env
            );
        }
    }
    Ok(())
}

/// Loads the project CI configuration from disk.
///
/// # Parameters
///
/// * `project` - The project root containing `.infra/ci/ci.toml`.
///
/// # Returns
///
/// The parsed configuration, or the default configuration when the file is
/// absent.
///
/// # Errors
///
/// Returns an error when the file cannot be read or parsed.
pub fn load_config(project: &Path) -> Result<Config> {
    let path = project.join(".infra/ci/ci.toml");
    if !path.is_file() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    ::toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

/// Prints the commands represented by selected tasks.
///
/// # Parameters
///
/// * `project` - The project root printed in the plan header.
/// * `tasks` - Tasks whose commands should be printed; the vector is consumed
///   in task order.
///
/// # Errors
///
/// Returns an error for an invalid project path, malformed configuration, or
/// invalid selected tasks. This is the same validated plan used by `run`.
pub fn plan(project: &Path, tasks: Vec<Task>) -> Result<()> {
    plan_workflow(project, &tasks)
}

/// Executes the selected infrastructure commands in the project directory.
///
/// # Parameters
///
/// * `project` - The working directory passed to every task process.
/// * `tasks` - Tasks to execute in order; the vector is consumed as commands
///   are launched.
///
/// Each command inherits the caller's standard input, output, and error
/// streams. When `RS_INFRA_BIN_DIR` is set, task executables are resolved from
/// that directory; otherwise they are resolved through the process `PATH`.
/// Cargo and hooks use their own executable paths. Matrix dependency checks
/// temporarily update Cargo.lock and restore it on normal success or failure.
/// Audit may retry with cached data according to the local configuration.
///
/// # Errors
///
/// Returns an error when a task cannot start or exits unsuccessfully.
pub fn run(project: &Path, tasks: Vec<Task>) -> Result<()> {
    run_with_coverage_policy(project, tasks, true)
}

/// Executes CI tasks and optionally allows coverage percentage shortfalls.
///
/// Coverage collection and validation still run when `enforce_thresholds` is
/// false. Other failed commands remain errors.
///
/// # Errors
///
/// Returns an error when planning or a task fails, including a coverage
/// threshold failure when `enforce_thresholds` is true.
pub fn run_with_coverage_policy(project: &Path, tasks: Vec<Task>, enforce_thresholds: bool) -> Result<()> {
    let project = project.canonicalize().context("invalid project directory")?;
    let config = LocalConfig::load(&project)?;
    let bin_dir = std::env::var_os("RS_INFRA_BIN_DIR").map(PathBuf::from);
    let jobs = workflow(&project, &tasks)?;
    for job in jobs {
        if job.commands.is_empty() {
            println!("{}: not configured; skipping", job.name);
            continue;
        }
        if job.task == Task::FeatureMatrix {
            crate::matrix::run(&project, &config)?;
            continue;
        }
        if job.task == Task::Fuzz {
            local::ensure_fuzz(&project, &config)?;
        }
        for mut spec in job.commands {
            if job.task == Task::Coverage && !enforce_thresholds {
                spec.args.push("--ignore-thresholds".into());
            }
            if spec.executable.starts_with("rs-infra-")
                && let Some(dir) = &bin_dir
            {
                spec.executable = dir.join(&spec.executable).to_string_lossy().into_owned();
            }
            if job.task == Task::Audit {
                local::audit(&project, &spec, &config)?;
            } else {
                local::execute(&project, &spec)?;
            }
        }
    }
    Ok(())
}
