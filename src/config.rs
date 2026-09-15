// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

use crate::Task;

const DEFAULT_TASKS: &[Task] = &[
    Task::Style,
    Task::Clippy,
    Task::CoverageCfgClippy,
    Task::Verify,
    Task::FeatureMatrix,
    Task::ProjectHook,
    Task::Package,
    Task::Coverage,
    Task::Audit,
];

/// Project CI configuration containing the selected infrastructure tasks.
///
/// # Examples
///
/// ```
/// use qubit_infra_ci::Config;
///
/// let config = Config::default();
/// assert!(config.tasks.is_empty());
/// ```
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Tasks explicitly enabled by the project configuration.
    pub tasks: Vec<Task>,
}

impl Config {
    /// Resolves an optional command-line selection against configured tasks.
    ///
    /// # Parameters
    ///
    /// * `only` - The requested subset, or an empty slice for all configured tasks.
    ///
    /// # Returns
    ///
    /// The selected tasks in execution order.
    ///
    /// # Errors
    ///
    /// Returns an error when configured tasks contain a duplicate or a
    /// requested task is not enabled.
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
        validate_unique(only)?;
        Ok(only.to_vec())
    }
}

/// Rejects duplicate task entries while preserving their original order.
pub(crate) fn validate_unique(tasks: &[Task]) -> Result<()> {
    for (index, task) in tasks.iter().enumerate() {
        if tasks[..index].contains(task) {
            bail!("task '{task}' is configured more than once");
        }
    }
    Ok(())
}
