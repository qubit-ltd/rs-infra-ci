// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Project-owned options for local checks.

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;
use std::path::Path;

/// Local execution settings read from the `[local]` table in `.infra/ci.toml`.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LocalConfig {
    /// Optional Cargo toolchain for build commands and audit.
    pub build_toolchain: Option<String>,
    /// Optional Cargo toolchain for all Clippy invocations.
    pub clippy_toolchain: Option<String>,
    /// Whether to run the coverage-configured Clippy pass.
    pub coverage_cfg_clippy: bool,
    /// Whether network failures may fall back to cached RustSec data.
    pub audit_cached_fallback: bool,
    /// Project-relative matrix configuration path.
    pub matrix: String,
    /// Project-relative optional executable hook path.
    pub hook: String,
}

impl Default for LocalConfig {
    /// Uses current Cargo toolchains, the standard paths, and legacy audit fallback.
    fn default() -> Self {
        Self {
            build_toolchain: None,
            clippy_toolchain: None,
            coverage_cfg_clippy: false,
            audit_cached_fallback: true,
            matrix: ".infra/ci/cargo-matrix.json".into(),
            hook: "project-ci-check.sh".into(),
        }
    }
}

impl LocalConfig {
    /// Reads local options; rejects malformed values and paths outside the project.
    pub(crate) fn load(project: &Path) -> Result<Self> {
        let path = project.join(".infra/ci.toml");
        let config = if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let document: toml::Value = toml::from_str(&text)?;
            document
                .get("local")
                .cloned()
                .map(toml::Value::try_into)
                .transpose()
                .context("invalid [local] CI configuration")?
                .unwrap_or_default()
        } else {
            Self::default()
        };
        for path in [&config.matrix, &config.hook] {
            if path.is_empty()
                || Path::new(path).components().any(|part| {
                    !matches!(
                        part,
                        std::path::Component::Normal(_) | std::path::Component::CurDir
                    )
                })
            {
                bail!("local CI paths must be project-relative: {path}");
            }
        }
        for value in [&config.build_toolchain, &config.clippy_toolchain]
            .into_iter()
            .flatten()
        {
            if value.is_empty() || value.starts_with('-') || value.chars().any(char::is_whitespace)
            {
                bail!("invalid Cargo toolchain: {value}");
            }
        }
        Ok(config)
    }
}
