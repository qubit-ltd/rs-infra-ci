// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Top-level operations of the CI binary.

use clap::Subcommand;

use crate::matrix_command::MatrixCommand;

/// Supported command-line operations.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Print the selected workflow plan.
    Plan,
    /// Execute the selected workflow.
    Check,
    /// Plan or run one configured Cargo feature check.
    Matrix {
        /// Matrix operation to perform.
        #[command(subcommand)]
        command: MatrixCommand,
    },
}
