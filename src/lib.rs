// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Plans and runs project-local Rust infrastructure CI tasks.

mod command_spec;
mod config;
mod job_spec;
mod task;
mod tool_config;
mod tool_spec;
mod workflow;

pub use command_spec::CommandSpec;
pub use config::Config;
pub use job_spec::JobSpec;
pub use task::Task;
pub use tool_config::ToolConfig;
pub use tool_spec::ToolSpec;
pub use workflow::jobs;
pub use workflow::load_config;
pub use workflow::load_tools;
pub use workflow::plan;
pub use workflow::plan_workflow;
pub use workflow::run;
pub use workflow::workflow;
