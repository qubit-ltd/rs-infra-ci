// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;

use crate::CommandSpec;
use crate::Config;
use crate::JobSpec;
use crate::Task;
use crate::ToolConfig;
use crate::ToolSpec;
use crate::config::validate_unique;

/// Loads revision-pinned tool specifications from a project.
///
/// # Parameters
///
/// * `project` - The project root containing `.infra/ci/tools.toml`.
///
/// # Returns
///
/// The parsed tool configuration, or an empty configuration when the file is
/// absent.
///
/// # Errors
///
/// Returns an error when the file cannot be read, parsed, or contains a
/// revision that is not a full hexadecimal Git SHA.
pub fn load_tools(project: &Path) -> Result<ToolConfig> {
    let path = project.join(".infra/ci/tools.toml");
    if !path.is_file() {
        return Ok(ToolConfig {
            tools: BTreeMap::new(),
        });
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let tools: BTreeMap<String, ToolSpec> =
        ::toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))?;
    for (name, tool) in &tools {
        if tool.revision.len() != 40 || !tool.revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            bail!("tool '{name}' revision must be a 40-character hexadecimal Git SHA");
        }
    }
    Ok(ToolConfig { tools })
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
/// A job specification for each selected task.
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
            Ok(JobSpec {
                name: task.to_string(),
                task: *task,
                commands: task
                    .commands()
                    .iter()
                    .map(|args| CommandSpec {
                        executable: tool_name.to_owned(),
                        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
                    })
                    .collect(),
                tool,
            })
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
    jobs(tasks, &load_tools(project)?)
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
        for command in job.commands {
            println!("run: {} {}", command.executable, command.args.join(" "));
        }
    }
    Ok(())
}

/// Loads the project CI configuration from disk.
///
/// # Parameters
///
/// * `project` - The project root containing `.infra/ci.toml`.
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
    let path = project.join(".infra/ci.toml");
    if !path.is_file() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    ::toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

/// Prints the commands represented by selected tasks.
///
/// # Parameters
///
/// * `project` - The project root printed in the plan header.
/// * `tasks` - Tasks whose commands should be printed.
///
/// # Errors
///
/// This function currently performs no fallible operation and always returns
/// `Ok(())`.
pub fn plan(project: &Path, tasks: Vec<Task>) -> Result<()> {
    println!("project: {}", project.display());
    for task in tasks {
        for args in task.commands() {
            println!("{task}: {} {}", task.executable(), args.join(" "));
        }
    }
    Ok(())
}

/// Executes the selected infrastructure commands in the project directory.
///
/// # Parameters
///
/// * `project` - The working directory passed to every task process.
/// * `tasks` - Tasks to execute in order.
///
/// # Errors
///
/// Returns an error when a task cannot start or exits unsuccessfully.
pub fn run(project: &Path, tasks: Vec<Task>) -> Result<()> {
    let bin_dir = std::env::var_os("RS_INFRA_BIN_DIR").map(PathBuf::from);
    for task in tasks {
        let executable = bin_dir
            .as_ref()
            .map(|dir| dir.join(task.executable()))
            .unwrap_or_else(|| PathBuf::from(task.executable()));
        for args in task.commands() {
            let mut command = Command::new(&executable);
            command
                .current_dir(project)
                .arg("--project")
                .arg(project)
                .args(*args)
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit());
            let status = command.status().with_context(|| {
                format!("failed to start task '{task}' ({})", executable.display())
            })?;
            if !status.success() {
                bail!("task '{task}' failed with status {status}");
            }
        }
    }
    Ok(())
}
