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
    /// Runs lock, build, test, documentation, and packaging checks.
    Verify,
    /// Runs strict workspace Clippy checks.
    Clippy,
    /// Optionally repeats Clippy with coverage configuration enabled.
    CoverageCfgClippy,
    /// Executes the project's Cargo compatibility matrix.
    FeatureMatrix,
    /// Executes an optional project-owned hook.
    ProjectHook,
    /// Runs configured Miri tests after installing the pinned Miri toolchain.
    Miri,
    /// Runs configured AddressSanitizer checks after installing rust-src.
    AddressSanitizer,
    /// Runs configured fuzz targets using the configured fuzz mode.
    Fuzz,
    /// Runs configured Loom model tests with the loom cfg enabled.
    Loom,
    /// Builds and verifies publishable packages through the verification tool.
    Package,
    /// Audits dependencies, with a configurable database-fetch fallback.
    Audit,
    /// Collects and checks the project's coverage.
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
            Self::Verify
            | Self::Package
            | Self::Miri
            | Self::AddressSanitizer
            | Self::Fuzz
            | Self::Loom => "rs-infra-verify",
            Self::Clippy | Self::CoverageCfgClippy | Self::FeatureMatrix | Self::Audit => "cargo",
            Self::ProjectHook => "./project-ci-check.sh",
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
            Self::Package => &[&["run", "--suite", "package"]],
            Self::Clippy | Self::CoverageCfgClippy => &[&[
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ]],
            Self::Audit => &[&["audit"]],
            Self::Miri => &[&["run", "--suite", "miri"]],
            Self::AddressSanitizer => &[&["run", "--suite", "address-sanitizer"]],
            Self::Fuzz => &[&["run", "--suite", "fuzz"]],
            Self::Loom => &[&["run", "--suite", "loom"]],
            Self::FeatureMatrix | Self::ProjectHook => &[],
            Self::Coverage => &[&["collect"]],
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
            Self::Clippy => "clippy",
            Self::CoverageCfgClippy => "coverage-cfg-clippy",
            Self::FeatureMatrix => "feature-matrix",
            Self::ProjectHook => "project-hook",
            Self::Miri => "miri",
            Self::AddressSanitizer => "address-sanitizer",
            Self::Fuzz => "fuzz",
            Self::Loom => "loom",
            Self::Package => "package",
            Self::Audit => "audit",
            Self::Coverage => "coverage",
            Self::Pages => "pages",
            Self::Dependency => "dependency",
        })
    }
}
