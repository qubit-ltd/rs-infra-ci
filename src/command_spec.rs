// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::collections::BTreeMap;

/// One executable invocation in a generated CI job.
///
/// # Examples
///
/// ```
/// use qubit_infra_ci::CommandSpec;
///
/// let command = CommandSpec {
///     executable: "cargo".into(),
///     args: vec!["test".into()],
///     env: Default::default(),
/// };
/// assert_eq!(command.args, ["test"]);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandSpec {
    /// The executable to invoke.
    pub executable: String,
    /// The command-line arguments passed to the executable, in order.
    pub args: Vec<String>,
    /// Environment overrides scoped to this child process.
    pub env: BTreeMap<String, String>,
}
