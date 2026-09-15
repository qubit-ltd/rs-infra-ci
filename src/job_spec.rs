// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use crate::CommandSpec;
use crate::Task;
use crate::ToolSpec;

/// The complete information needed to materialize one workflow job.
///
/// # Examples
///
/// ```
/// use qubit_infra_ci::CommandSpec;
/// use qubit_infra_ci::JobSpec;
/// use qubit_infra_ci::Task;
///
/// let job = JobSpec {
///     name: "style".into(),
///     task: Task::Style,
///     commands: vec![CommandSpec {
///         executable: "rs-infra-style".into(),
///         env: Default::default(),
///         args: vec!["check".into()],
///     }],
///     tool: None,
/// };
/// assert_eq!(job.task, Task::Style);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobSpec {
    /// The human-readable task name used for the job.
    pub name: String,
    /// The infrastructure task represented by this job.
    pub task: Task,
    /// The commands executed by the job, in execution order.
    pub commands: Vec<CommandSpec>,
    /// The optional revision-pinned tool used by the job.
    pub tool: Option<ToolSpec>,
}
