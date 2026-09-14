use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use serde::Deserialize;

const DEFAULT_TASKS: &[Task] = &[Task::Style, Task::Verify];

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Task {
    Style,
    Verify,
    Coverage,
    Pages,
    Dependency,
}

impl Task {
    fn executable(self) -> &'static str {
        match self {
            Self::Style => "rs-infra-style",
            Self::Verify => "rs-infra-verify",
            Self::Coverage => "rs-infra-coverage",
            Self::Pages => "rs-infra-pages",
            Self::Dependency => "rs-infra-dependency",
        }
    }

    fn commands(self) -> &'static [&'static [&'static str]] {
        match self {
            Self::Style => &[&["check"]],
            Self::Verify => &[
                &["lock", "check"],
                &["run", "--suite", "build"],
                &["run", "--suite", "test"],
                &["run", "--suite", "doc"],
                &["run", "--suite", "package"],
            ],
            Self::Coverage => &[&["check"]],
            Self::Pages => &[&["build"]],
            Self::Dependency => &[&["check"]],
        }
    }
}

impl fmt::Display for Task {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Style => "style",
            Self::Verify => "verify",
            Self::Coverage => "coverage",
            Self::Pages => "pages",
            Self::Dependency => "dependency",
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// One executable invocation in a generated CI job.
pub struct CommandSpec {
    pub executable: String,
    pub args: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
/// Immutable source and package identity used to install one CI tool.
pub struct ToolSpec {
    pub source: String,
    pub revision: String,
    pub binary: String,
    pub package: String,
}

impl ToolSpec {
    /// Returns the arguments for a locked `cargo install --git` invocation.
    pub fn install_args(&self) -> Vec<String> {
        vec![
            "install".into(),
            "--git".into(),
            self.source.clone(),
            "--rev".into(),
            self.revision.clone(),
            "--locked".into(),
            self.package.clone(),
            "--bin".into(),
            self.binary.clone(),
        ]
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
/// Revision-pinned tools keyed by their installed executable name.
pub struct ToolConfig {
    pub tools: BTreeMap<String, ToolSpec>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// The complete information needed to materialize one workflow job.
pub struct JobSpec {
    pub name: String,
    pub task: Task,
    pub commands: Vec<CommandSpec>,
    pub tool: Option<ToolSpec>,
}

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
        toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))?;
    for (name, tool) in &tools {
        if tool.revision.len() != 40 || !tool.revision.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            bail!("tool '{name}' revision must be a 40-character hexadecimal Git SHA");
        }
    }
    Ok(ToolConfig { tools })
}

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
pub fn workflow(project: &Path, tasks: &[Task]) -> Result<Vec<JobSpec>> {
    jobs(tasks, &load_tools(project)?)
}

/// Prints a migration-friendly workflow plan, including tool installation data.
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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub tasks: Vec<Task>,
}

impl Config {
    pub fn select(&self, only: &[Task]) -> Result<Vec<Task>> {
        let configured = if self.tasks.is_empty() {
            DEFAULT_TASKS.to_vec()
        } else {
            self.tasks.clone()
        };
        validate_unique(&configured)?;
        if only.is_empty() {
            return Ok(configured);
        }
        for task in only {
            if !configured.contains(task) {
                bail!("task '{task}' is not enabled in .infra/ci.toml");
            }
        }
        Ok(only.to_vec())
    }
}

pub fn load_config(project: &Path) -> Result<Config> {
    let path = project.join(".infra/ci.toml");
    if !path.is_file() {
        return Ok(Config::default());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

pub fn plan(project: &Path, tasks: Vec<Task>) -> Result<()> {
    println!("project: {}", project.display());
    for task in tasks {
        for args in task.commands() {
            println!("{task}: {} {}", task.executable(), args.join(" "));
        }
    }
    Ok(())
}

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

fn validate_unique(tasks: &[Task]) -> Result<()> {
    for (index, task) in tasks.iter().enumerate() {
        if tasks[..index].contains(task) {
            bail!("task '{task}' is configured more than once");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Config, Task, load_config};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn default_tasks_are_stable() {
        let selected = Config::default().select(&[]).expect("defaults are valid");
        assert_eq!(selected, vec![Task::Style, Task::Verify]);
    }

    #[test]
    fn config_selects_enabled_task() {
        let directory = tempdir().expect("temp directory");
        fs::create_dir(directory.path().join(".infra")).expect("infra directory");
        fs::write(
            directory.path().join(".infra/ci.toml"),
            "tasks = [\"pages\"]\n",
        )
        .expect("config");
        let config = load_config(directory.path()).expect("config loads");
        assert_eq!(config.select(&[]).expect("selection"), vec![Task::Pages]);
        assert!(config.select(&[Task::Style]).is_err());
    }

    #[test]
    fn duplicate_tasks_are_rejected() {
        let config = Config {
            tasks: vec![Task::Style, Task::Style],
        };
        assert!(config.select(&[]).is_err());
    }
}
