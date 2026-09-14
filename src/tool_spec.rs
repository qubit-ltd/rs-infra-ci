// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use serde::Deserialize;

/// Immutable source and package identity used to install one CI tool.
///
/// # Examples
///
/// ```
/// use qubit_infra_ci::ToolSpec;
///
/// let tool = ToolSpec {
///     source: "https://example.invalid/tool.git".into(),
///     revision: "0123456789abcdef0123456789abcdef01234567".into(),
///     binary: "tool".into(),
///     package: "package".into(),
/// };
/// assert_eq!(tool.install_args()[0], "install");
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct ToolSpec {
    /// The Git repository containing the tool.
    pub source: String,
    /// The full Git revision used to pin the tool.
    pub revision: String,
    /// The binary installed from the package.
    pub binary: String,
    /// The Cargo package passed to `cargo install`.
    pub package: String,
}

impl ToolSpec {
    /// Returns arguments for a locked `cargo install --git` invocation.
    ///
    /// The returned vector owns copies of all repository, revision, package,
    /// and binary strings so it can be passed directly to a process builder.
    ///
    /// # Returns
    ///
    /// The complete argument vector in the order expected by Cargo.
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
