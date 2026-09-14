// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::fmt;

use clap::ValueEnum;
use serde::Deserialize;

/// A supported infrastructure operation in a project's CI workflow.
///
/// # Examples
///
/// ```
/// use qubit_infra_ci::Task;
///
/// let task = Task::Style;
/// assert_eq!(task.to_string(), "style");
/// ```
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum Task {
    /// Checks source formatting and project style rules.
    Style,
    /// Runs the project's build, test, documentation, and packaging checks.
    Verify,
    /// Checks the project's coverage configuration.
    Coverage,
    /// Builds the project's documentation pages.
    Pages,
    /// Checks dependency policy compliance.
    Dependency,
}

impl Task {
    /// Returns the executable associated with this task.
    ///
    /// # Returns
    ///
    /// The independent infrastructure binary that implements this task.
    pub(crate) fn executable(self) -> &'static str {
        match self {
            Self::Style => "rs-infra-style",
            Self::Verify => "rs-infra-verify",
            Self::Coverage => "rs-infra-coverage",
            Self::Pages => "rs-infra-pages",
            Self::Dependency => "rs-infra-dependency",
        }
    }

    /// Returns the command argument sequences associated with this task.
    ///
    /// # Returns
    ///
    /// The ordered argument sequences passed to the task's executable.
    pub(crate) fn commands(self) -> &'static [&'static [&'static str]] {
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
    /// Writes the configuration spelling of this task.
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
