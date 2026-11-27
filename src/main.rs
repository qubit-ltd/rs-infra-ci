// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::io::Write;

use anyhow::Result;
use clap::Parser;
use qubit_infra_ci::Config;
use qubit_infra_ci::load_config;
use qubit_infra_ci::plan_github_matrix;
use qubit_infra_ci::plan_workflow;
use qubit_infra_ci::run_matrix_check;
use qubit_infra_ci::run_with_coverage_policy;

mod cli;
mod command;
mod matrix_command;

use crate::cli::Cli;
use crate::command::Command;
use crate::matrix_command::MatrixCommand;

/// Parses arguments and executes the requested operation.
fn main() {
    if let Err(error) = execute() {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}

/// Parses arguments, executes the requested operation, and reports its result.
fn execute() -> Result<()> {
    let cli = Cli::parse();
    let operation = match &cli.command {
        Command::Plan => "plan",
        Command::Check => "check",
        Command::Matrix {
            command: MatrixCommand::Plan { .. },
        } => "matrix plan",
        Command::Matrix {
            command: MatrixCommand::Run { .. },
        } => "matrix run",
    };
    let machine_output = matches!(
        cli.command,
        Command::Matrix {
            command: MatrixCommand::Plan { .. }
        }
    );

    let result = (|| match cli.command {
        Command::Plan | Command::Check => {
            let config: Config = load_config(&cli.project)?;
            let tasks = config.select(&cli.only)?;
            if matches!(cli.command, Command::Plan) {
                plan_workflow(&cli.project, &tasks)
            } else {
                run_with_coverage_policy(&cli.project, tasks, !cli.ignore_coverage_thresholds)
            }
        }
        Command::Matrix {
            command: MatrixCommand::Plan { output, runner_output },
        } => {
            let matrix = plan_github_matrix(&cli.project)?;
            let mut json = serde_json::to_vec(&matrix)?;
            json.push(b'\n');
            if let Some(path) = output {
                std::fs::write(path, json)?;
            } else {
                std::io::stdout().write_all(&json)?;
            }
            if let Some(path) = runner_output {
                if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(std::env::current_exe()?, path)?;
            }
            Ok(())
        }
        Command::Matrix {
            command: MatrixCommand::Run { check },
        } => run_matrix_check(&cli.project, &check),
    })();

    if result.is_ok() && !machine_output {
        println!("✅ rs-infra-ci: {operation} succeeded");
    }
    result.map_err(|error| anyhow::anyhow!("❌ rs-infra-ci: {operation} failed: {error:#}"))
}
