// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use clap::Subcommand;

use qubit_infra_ci::Config;
use qubit_infra_ci::Task;
use qubit_infra_ci::load_config;
use qubit_infra_ci::plan_workflow;
use qubit_infra_ci::run;

/// Command-line arguments for the CI task orchestrator.
#[derive(Debug, Parser)]
#[command(name = "rs-infra-ci", about = "Run the project's reusable CI tasks")]
struct Cli {
    /// Project directory whose CI configuration should be used.
    #[arg(long, default_value = ".")]
    project: PathBuf,
    /// Comma-separated tasks to run instead of all configured tasks.
    #[arg(long, value_delimiter = ',')]
    only: Vec<Task>,
    /// Operation to perform.
    #[command(subcommand)]
    command: Command,
}

/// Supported command-line operations.
#[derive(Debug, Subcommand)]
enum Command {
    /// Print the selected workflow plan.
    Plan,
    /// Execute the selected workflow.
    Check,
}

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
    };

    let result = (|| {
        let config: Config = load_config(&cli.project)?;
        let tasks = config.select(&cli.only)?;

        match cli.command {
            Command::Plan => plan_workflow(&cli.project, &tasks),
            Command::Check => run(&cli.project, tasks),
        }
    })();

    if result.is_ok() {
        println!("rs-infra-ci: {operation} succeeded");
    }
    result.map_err(|error| anyhow::anyhow!("rs-infra-ci: {operation} failed: {error:#}"))
}
