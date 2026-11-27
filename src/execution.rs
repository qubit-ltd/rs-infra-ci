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

use crate::CommandSpec;

/// Compiler cache settings shared by every command in one CI invocation.
pub(crate) struct Execution {
    /// Whether this invocation configured sccache.
    compiler_cache: bool,
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
    pub(crate) fn new(project: &Path) -> Self {
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
        Self { compiler_cache }
    }

    /// Applies cache settings to the child and its descendants.
    ///
    /// # Parameters
    ///
    /// * `command` - Child process to configure.
    /// * `spec` - Per-command environment overrides.
    pub(crate) fn apply(&self, command: &mut Command, spec: &CommandSpec) {
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
