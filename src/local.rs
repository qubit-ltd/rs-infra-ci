// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Planning and execution shared by local and CI callers.

use crate::CommandSpec;
use crate::Task;
use crate::local_config::LocalConfig;
use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use std::path::Path;
use std::process::Command;

/// Builds a Cargo invocation using the configured build or Clippy toolchain.
pub(crate) fn cargo(config: &LocalConfig, args: Vec<String>) -> CommandSpec {
    let toolchain = if args.first().is_some_and(|arg| arg == "clippy") {
        config
            .clippy_toolchain
            .as_ref()
            .or(config.build_toolchain.as_ref())
    } else {
        config.build_toolchain.as_ref()
    };
    let mut full = Vec::new();
    if let Some(toolchain) = toolchain {
        full.push(format!("+{toolchain}"));
    }
    full.extend(args);
    CommandSpec {
        executable: "cargo".into(),
        args: full,
        env: Default::default(),
    }
}

/// Resolves a project-owned hook, skipping absence and rejecting invalid files.
pub(crate) fn hook(project: &Path, config: &LocalConfig) -> Result<Vec<CommandSpec>> {
    let path = project.join(&config.hook);
    if !path.try_exists()? {
        if path.is_symlink() {
            bail!("project hook is a broken symlink: {}", path.display());
        }
        return Ok(Vec::new());
    }
    let metadata = path.metadata()?;
    if !metadata.is_file() {
        bail!("project hook is not a regular file: {}", path.display());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            bail!("project hook is not executable: {}", path.display());
        }
    }
    let path = path.canonicalize()?;
    #[cfg(windows)]
    let (executable, args) = ("bash".into(), vec![path.to_string_lossy().into_owned()]);
    #[cfg(not(windows))]
    let (executable, args) = (path.to_string_lossy().into_owned(), Vec::new());
    Ok(vec![CommandSpec {
        executable,
        args,
        env: Default::default(),
    }])
}

/// Expands local tasks; returns `None` for tasks implemented by infrastructure tools.
pub(crate) fn commands(
    project: &Path,
    task: Task,
    config: &LocalConfig,
) -> Result<Option<Vec<CommandSpec>>> {
    let commands = match task {
        Task::Clippy | Task::CoverageCfgClippy => {
            if task == Task::CoverageCfgClippy && !config.coverage_cfg_clippy {
                return Ok(Some(Vec::new()));
            }
            let mut command = cargo(
                config,
                task.commands()[0].iter().map(|arg| (*arg).into()).collect(),
            );
            if task == Task::CoverageCfgClippy {
                command
                    .env
                    .insert("RUSTFLAGS".into(), "--cfg coverage".into());
            }
            vec![command]
        }
        Task::Audit => vec![cargo(config, vec!["audit".into()])],
        Task::ProjectHook => hook(project, config)?,
        Task::FeatureMatrix => crate::matrix::plan(project, config)?,
        Task::Miri => vec![
            rustup_toolchain(config, &["miri", "rust-src"]),
            cargo(
                config,
                vec![
                    format!("+{}", config.nightly_toolchain),
                    "miri".into(),
                    "setup".into(),
                ],
            ),
            verify_suite("miri", config),
        ],
        Task::AddressSanitizer => vec![
            rustup_toolchain(config, &["rust-src"]),
            verify_suite("address-sanitizer", config),
        ],
        Task::Fuzz => vec![verify_fuzz(config)],
        Task::Loom => vec![verify_loom(config)],
        Task::StrictDoc => {
            let mut command = verify_suite("doc", config);
            command
                .env
                .insert("RUSTDOCFLAGS".into(), "-D warnings -D missing-docs".into());
            vec![command]
        }
        Task::Readme => vec![verify_suite("readme", config)],
        Task::ReleaseBuild => vec![cargo(
            config,
            vec!["build".into(), "--release".into(), "--verbose".into()],
        )],
        _ => return Ok(None),
    };
    Ok(Some(commands))
}

/// Creates a rustup installation command for configured nightly components.
///
/// # Parameters
///
/// * `config` - The project-local nightly toolchain setting.
/// * `components` - Rustup component names to install.
///
/// # Returns
///
/// A command specification for installing the selected toolchain components.
fn rustup_toolchain(config: &LocalConfig, components: &[&str]) -> CommandSpec {
    let mut args = vec![
        "toolchain".into(),
        "install".into(),
        config.nightly_toolchain.clone(),
        "--profile".into(),
        "minimal".into(),
    ];
    for component in components {
        args.extend(["--component".into(), (*component).into()]);
    }
    CommandSpec {
        executable: "rustup".into(),
        args,
        env: Default::default(),
    }
}

/// Creates a verifier invocation with the selected nightly toolchain.
///
/// # Parameters
///
/// * `suite` - The verification suite name.
/// * `config` - The project-local nightly toolchain setting.
///
/// # Returns
///
/// A verifier command with the nightly selection in its child environment.
fn verify_suite(suite: &str, config: &LocalConfig) -> CommandSpec {
    CommandSpec {
        executable: "rs-infra-verify".into(),
        args: vec!["run".into(), "--suite".into(), suite.into()],
        env: [(
            "RS_INFRA_NIGHTLY_TOOLCHAIN".into(),
            config.nightly_toolchain.clone(),
        )]
        .into(),
    }
}

/// Creates a fuzz verifier invocation with its mode and smoke limits.
///
/// # Parameters
///
/// * `config` - The project-local fuzz behavior and resource limits.
///
/// # Returns
///
/// A fuzz-suite command with all configured values in its child environment.
fn verify_fuzz(config: &LocalConfig) -> CommandSpec {
    let mut command = verify_suite("fuzz", config);
    command.env.extend([
        ("RS_INFRA_FUZZ_MODE".into(), config.fuzz_mode.clone()),
        (
            "RS_INFRA_FUZZ_SECONDS_PER_TARGET".into(),
            config.fuzz_seconds_per_target.to_string(),
        ),
        (
            "RS_INFRA_FUZZ_MAX_LEN".into(),
            config.fuzz_max_len.to_string(),
        ),
    ]);
    command
}

/// Creates a Loom invocation with the model-checking cfg enabled.
///
/// # Parameters
///
/// * `config` - The project-local nightly toolchain setting.
///
/// # Returns
///
/// A Loom-suite command with `RUSTFLAGS=--cfg loom` for its child process.
fn verify_loom(config: &LocalConfig) -> CommandSpec {
    let mut command = verify_suite("loom", config);
    command.env.insert("RUSTFLAGS".into(), "--cfg loom".into());
    command
}

/// Installs cargo-fuzz only when the exact configured version is unavailable.
///
/// # Errors
///
/// Returns an error if Cargo cannot inspect or install the requested version.
///
/// # Parameters
///
/// * `project` - The project directory used for the Cargo version probe.
/// * `config` - The exact cargo-fuzz version and mode configuration.
pub(crate) fn ensure_fuzz(project: &Path, config: &LocalConfig) -> Result<()> {
    if config.fuzz_mode == "disabled" {
        println!("fuzz: disabled; skipping cargo-fuzz installation");
        return Ok(());
    }
    let version = Command::new("cargo")
        .args(["fuzz", "--version"])
        .current_dir(project)
        .output();
    if version.is_ok_and(|output| {
        output.status.success()
            && String::from_utf8_lossy(&output.stdout)
                .trim_end()
                .ends_with(&config.fuzz_version)
    }) {
        return Ok(());
    }
    let install = CommandSpec {
        executable: "cargo".into(),
        args: vec![
            "install".into(),
            "cargo-fuzz".into(),
            "--locked".into(),
            "--version".into(),
            config.fuzz_version.clone(),
        ],
        env: Default::default(),
    };
    execute(project, &install)
}

/// Creates a child process in the canonical project root without changing global state.
pub(crate) fn process(project: &Path, spec: &CommandSpec) -> Command {
    let mut command = Command::new(&spec.executable);
    command
        .current_dir(project)
        .args(&spec.args)
        .envs(&spec.env);
    if spec.env.contains_key("RUSTFLAGS") {
        command.env_remove("CARGO_ENCODED_RUSTFLAGS");
    }
    command
}

/// Executes one planned command, forwarding output and failing on any nonzero exit.
pub(crate) fn execute(project: &Path, spec: &CommandSpec) -> Result<()> {
    let status = process(project, spec)
        .status()
        .with_context(|| format!("failed to start {} {:?}", spec.executable, spec.args))?;
    if !status.success() {
        bail!("{} {:?} failed with {status}", spec.executable, spec.args);
    }
    Ok(())
}

/// Audits dependencies and retries only recognized advisory-database fetch failures.
/// Captures and forwards both streams; vulnerability and retry failures remain fatal.
pub(crate) fn audit(project: &Path, spec: &CommandSpec, config: &LocalConfig) -> Result<()> {
    let output = process(project, spec)
        .output()
        .context("failed to start cargo audit")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    print!("{stdout}");
    eprint!("{stderr}");
    if output.status.success() {
        return Ok(());
    }
    let log = format!("{stdout}\n{stderr}").to_lowercase();
    if config.audit_cached_fallback
        && [
            "couldn't fetch advisory database",
            "failed to fetch advisory database",
            "failed to prepare fetch",
            "error sending request",
        ]
        .iter()
        .any(|message| log.contains(message))
    {
        eprintln!(
            "warning: audit database fetch failed; retrying cached data. CI must also audit current data."
        );
        let mut retry = spec.clone();
        retry.args.extend(["--no-fetch".into(), "--stale".into()]);
        return execute(project, &retry);
    }
    bail!("cargo audit failed with {}", output.status)
}
