// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Installed versions of shared CI tools.

use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;

/// Shared tool versions installed from `rs-infra-tools/conf/defaults.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SharedDefaults {
    /// Cargo toolchain used for build commands and audit.
    pub(crate) build_toolchain: String,
    /// Cargo toolchain used for Clippy.
    pub(crate) clippy_toolchain: String,
    /// Nightly toolchain used for Miri, sanitizers, and fuzz verification.
    pub(crate) nightly_toolchain: String,
    /// Exact cargo-fuzz version.
    pub(crate) fuzz_version: String,
}

impl SharedDefaults {
    /// Reads shared settings from the installed project configuration.
    ///
    /// # Parameters
    ///
    /// * `project` - Project directory holding the installed shared settings.
    ///
    /// # Returns
    ///
    /// The configured infrastructure tool versions.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read or parsed.
    pub(crate) fn load(project: &Path) -> Result<Self> {
        let path = project.join(".infra/tools/defaults.toml");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}; run ./update-infra.sh", path.display()))?;
        toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
    }
}
