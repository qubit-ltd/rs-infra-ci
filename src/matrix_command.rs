// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Operations for planning and executing a feature matrix.

use std::path::PathBuf;

use clap::Subcommand;

/// GitHub matrix planning and selected execution operations.
#[derive(Debug, Subcommand)]
pub(crate) enum MatrixCommand {
    /// Validate checks and emit a GitHub Actions matrix as JSON.
    Plan {
        /// Write the matrix JSON to a file instead of standard output.
        #[arg(long)]
        output: Option<PathBuf>,
        /// Copy the running, dynamically resolved binary to this path.
        #[arg(long)]
        runner_output: Option<PathBuf>,
    },
    /// Execute one named, fully validated feature check.
    Run {
        /// Name of the matrix check to execute.
        #[arg(long)]
        check: String,
    },
}
