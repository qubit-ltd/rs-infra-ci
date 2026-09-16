// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Project-owned options for local checks.

use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

/// Local execution settings read from the `[local]` table in `.infra/ci.toml`.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LocalConfig {
    /// Optional Cargo toolchain for build commands and audit.
    pub build_toolchain: Option<String>,
    /// Optional Cargo toolchain for all Clippy invocations.
    pub clippy_toolchain: Option<String>,
    /// Pinned nightly used by Miri, AddressSanitizer, and fuzz verification.
    pub nightly_toolchain: String,
    /// Fuzz execution mode: `smoke`, `build-only`, or `disabled`.
    pub fuzz_mode: String,
    /// Maximum smoke duration for each fuzz target, in seconds.
    pub fuzz_seconds_per_target: u32,
    /// Maximum input size passed to each fuzz target.
    pub fuzz_max_len: u32,
    /// Exact cargo-fuzz version installed before configured fuzz checks.
    pub fuzz_version: String,
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
    /// Uses current Cargo toolchains, the standard paths, and legacy audit
    /// fallback.
    fn default() -> Self {
        Self {
            build_toolchain: None,
            clippy_toolchain: None,
            nightly_toolchain: "nightly-2026-06-05".into(),
            fuzz_mode: "smoke".into(),
            fuzz_seconds_per_target: 10,
            fuzz_max_len: 4096,
            fuzz_version: "0.13.2".into(),
            coverage_cfg_clippy: false,
            audit_cached_fallback: true,
            matrix: ".infra/ci/cargo-matrix.json".into(),
            hook: "project-ci-check.sh".into(),
        }
    }
}

impl LocalConfig {
    /// Reads local options; rejects malformed values and paths outside the
    /// project.
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
        if config.nightly_toolchain.is_empty()
            || config.nightly_toolchain.starts_with(['+', '-'])
            || config.nightly_toolchain.chars().any(char::is_whitespace)
            || (config.nightly_toolchain.starts_with("nightly")
                && !is_pinned_nightly(&config.nightly_toolchain))
        {
            bail!("nightly_toolchain must be nightly-YYYY-MM-DD or a valid toolchain name");
        }
        if !matches!(
            config.fuzz_mode.as_str(),
            "smoke" | "build-only" | "disabled"
        ) {
            bail!("fuzz_mode must be smoke, build-only, or disabled");
        }
        if config.fuzz_seconds_per_target == 0 || config.fuzz_max_len == 0 {
            bail!("fuzz_seconds_per_target and fuzz_max_len must be positive");
        }
        if config.fuzz_version.is_empty()
            || config.fuzz_version.starts_with(['+', '-'])
            || config.fuzz_version.chars().any(char::is_whitespace)
        {
            bail!("fuzz_version must be a nonempty version without whitespace");
        }
        Ok(config)
    }
}

/// Checks the required pinned nightly toolchain spelling.
///
/// # Parameters
///
/// * `value` - The toolchain name being checked.
///
/// # Returns
///
/// `true` when `value` matches `nightly-YYYY-MM-DD` exactly.
fn is_pinned_nightly(value: &str) -> bool {
    let Some(date) = value.strip_prefix("nightly-") else {
        return false;
    };
    let bytes = date.as_bytes();
    bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}
