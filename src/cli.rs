// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Arguments for the CI binary.

use std::path::PathBuf;

use clap::Parser;
use qubit_infra_ci::Task;

use crate::command::Command;

/// Command-line arguments for the CI task orchestrator.
#[derive(Debug, Parser)]
#[command(name = "rs-infra-ci", about = "Run the project's reusable CI tasks")]
pub(crate) struct Cli {
    /// Project directory whose CI configuration should be used.
    #[arg(long, default_value = ".")]
    pub(crate) project: PathBuf,
    /// Comma-separated tasks to run instead of all configured tasks.
    #[arg(long, value_delimiter = ',')]
    pub(crate) only: Vec<Task>,
    /// Allow coverage threshold shortfalls while still running coverage.
    #[arg(long)]
    pub(crate) ignore_coverage_thresholds: bool,
    /// Operation to perform.
    #[command(subcommand)]
    pub(crate) command: Command,
}
