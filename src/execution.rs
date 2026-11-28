// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Compiler-cache configuration shared by CI commands.

use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde_json::Value;
use serde_json::from_slice;

use crate::CommandSpec;

static EXECUTION_NONCE: AtomicU64 = AtomicU64::new(0);

/// Compiler cache settings shared by every command in one CI invocation.
pub(crate) struct Execution {
    /// Whether this invocation configured sccache.
    compiler_cache: bool,
    target_parent: PathBuf,
    target_directory: PathBuf,
    cleaned: bool,
}

impl Execution {
    /// Selects sccache when available and preserves caller-configured wrappers.
    ///
    /// # Parameters
    ///
    /// * `project` - Project directory used to resolve Cargo configuration.
    ///
    /// # Returns
    ///
    /// The compiler cache settings for this invocation.
    pub(crate) fn new(project: &Path) -> Result<Self> {
        let compiler_cache = if std::env::var_os("RS_INFRA_SCCACHE").is_some_and(|value| value == "0") {
            eprintln!("rs-infra-ci: sccache disabled");
            false
        } else if cargo_wrapper_configured(project) {
            eprintln!("rs-infra-ci: using caller-configured compiler wrapper");
            false
        } else {
            let available = Command::new("sccache")
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success());
            if available {
                eprintln!("rs-infra-ci: sccache enabled");
            } else {
                eprintln!("rs-infra-ci: sccache unavailable; using Cargo build cache");
            }
            available
        };
        let target_parent = cargo_target_directory(project)?.join("rs-infra-ci");
        std::fs::create_dir_all(&target_parent)
            .with_context(|| format!("failed to create Cargo target parent {}", target_parent.display()))?;
        let target_parent = target_parent.canonicalize()
            .with_context(|| format!("failed to resolve Cargo target parent {}", target_parent.display()))?;
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let nonce = EXECUTION_NONCE.fetch_add(1, Ordering::Relaxed);
        let target_directory = target_parent.join(format!("run-{}-{timestamp}-{nonce}", std::process::id()));
        std::fs::create_dir(&target_directory)
            .with_context(|| format!("failed to create Cargo target {}", target_directory.display()))?;
        Ok(Self { compiler_cache, target_parent, target_directory, cleaned: false })
    }

    /// Applies cache settings to the child and its descendants.
    ///
    /// # Parameters
    ///
    /// * `command` - Child process to configure.
    /// * `spec` - Per-command environment overrides.
    pub(crate) fn apply(&self, command: &mut Command, spec: &CommandSpec) {
        command.env("CARGO_TARGET_DIR", &self.target_directory);
        if self.compiler_cache {
            command.env("RUSTC_WRAPPER", "sccache");
            if std::env::var_os("CARGO_INCREMENTAL").is_none() && !spec.env.contains_key("CARGO_INCREMENTAL") {
                command.env("CARGO_INCREMENTAL", "0");
            }
            // Coverage uses incremental compilation. Let sccache pass those
            // invocations through to rustc while caching normal builds.
            for setting in ["SCCACHE_ALLOW_INCREMENTAL", "SCCACHE_IGNORE_SERVER_IO_ERROR"] {
                if std::env::var_os(setting).is_none() && !spec.env.contains_key(setting) {
                    command.env(setting, "1");
                }
            }
        }
    }

    /// Reports cache statistics without changing the CI task result.
    pub(crate) fn report(&self) {
        if self.compiler_cache
            && let Ok(output) = Command::new("sccache").arg("--show-stats").output()
            && output.status.success()
        {
            eprintln!("rs-infra-ci: sccache statistics (shared server counters)");
            eprint!("{}", String::from_utf8_lossy(&output.stdout));
        }
    }

    pub(crate) fn target_directory(&self) -> &Path {
        &self.target_directory
    }

    pub(crate) fn cleanup(&mut self) -> Result<()> {
        if self.target_directory.parent() != Some(self.target_parent.as_path())
            || !self.target_directory.file_name().is_some_and(|name| name.to_string_lossy().starts_with("run-"))
        {
            bail!("refusing to clean unexpected Cargo target {}", self.target_directory.display());
        }
        std::fs::remove_dir_all(&self.target_directory)
            .with_context(|| format!("unable to clean Cargo target {}", self.target_directory.display()))?;
        self.cleaned = true;
        Ok(())
    }
}

impl Drop for Execution {
    fn drop(&mut self) {
        if self.cleaned {
            return;
        }
        if self.target_directory.parent() != Some(self.target_parent.as_path())
            || !self.target_directory.file_name().is_some_and(|name| name.to_string_lossy().starts_with("run-"))
        {
            eprintln!("rs-infra-ci: refusing to clean unexpected Cargo target {}", self.target_directory.display());
            return;
        }
        if let Err(error) = std::fs::remove_dir_all(&self.target_directory)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("rs-infra-ci: unable to clean Cargo target {}: {error}", self.target_directory.display());
        }
    }
}

pub(crate) fn cargo_target_directory(project: &Path) -> Result<PathBuf> {
    let manifest = project.join("Cargo.toml");
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1", "--manifest-path"])
        .arg(&manifest)
        .current_dir(project)
        .output()
        .with_context(|| format!("failed to run cargo metadata for {}", manifest.display()))?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed for {}: {}",
            manifest.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let metadata: Value = from_slice(&output.stdout).context("invalid cargo metadata JSON")?;
    let target = metadata["target_directory"].as_str().context("cargo metadata omitted target_directory")?;
    let target = PathBuf::from(target);
    if !target.is_absolute() {
        bail!("cargo metadata returned a relative target_directory: {}", target.display());
    }
    Ok(target)
}

/// Checks Cargo's environment and project configuration layers for wrappers.
///
/// # Parameters
///
/// * `project` - Project directory at the start of the Cargo configuration
///   search.
///
/// # Returns
///
/// Whether Cargo has a wrapper override or a configuration that cannot be
/// interpreted here.
fn cargo_wrapper_configured(project: &Path) -> bool {
    const WRAPPERS: [&str; 4] = [
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    ];
    if WRAPPERS.into_iter().any(|key| std::env::var_os(key).is_some()) {
        return true;
    }
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(|path| project.join(path))
        .or_else(|| std::env::home_dir().map(|home| home.join(".cargo")));
    project
        .ancestors()
        .map(|dir| dir.join(".cargo"))
        .chain(cargo_home)
        .any(|dir: PathBuf| {
            let legacy = dir.join("config");
            let path = if legacy.exists() {
                legacy
            } else {
                dir.join("config.toml")
            };
            match std::fs::read_to_string(path) {
                Ok(text) => text.parse::<toml::Table>().map_or(true, |config| {
                    config.contains_key("include")
                        || config
                            .get("build")
                            .and_then(toml::Value::as_table)
                            .is_some_and(|build| {
                                build.contains_key("rustc-wrapper") || build.contains_key("rustc-workspace-wrapper")
                            })
                        || config
                            .get("env")
                            .and_then(toml::Value::as_table)
                            .is_some_and(|env| WRAPPERS.into_iter().any(|key| env.contains_key(key)))
                }),
                Err(error) => error.kind() != std::io::ErrorKind::NotFound,
            }
        })
}
