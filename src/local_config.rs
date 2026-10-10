// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Project-specific CI options combined with installed shared tool versions.

use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde::Deserialize;

/// Local execution settings assembled from the project `[local]` table and
/// the installed `.infra/tools/defaults.toml`.
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

/// Shared tool versions installed from `rs-infra-tools/conf/defaults.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SharedDefaults {
    /// Cargo toolchain used for build commands and audit.
    build_toolchain: String,
    /// Cargo toolchain used for Clippy.
    clippy_toolchain: String,
    /// Nightly toolchain used for Miri, sanitizers, and fuzz verification.
    nightly_toolchain: String,
    /// Exact cargo-fuzz version.
    fuzz_version: String,
}

impl SharedDefaults {
    /// Reads shared settings from the installed project configuration.
    fn load(project: &Path) -> Result<Self> {
        let path = project.join(".infra/tools/defaults.toml");
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}; run ./update-infra.sh", path.display()))?;
        toml::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
    }
}

impl Default for LocalConfig {
    /// Uses project-specific defaults until shared versions are loaded.
    fn default() -> Self {
        Self {
            build_toolchain: None,
            clippy_toolchain: None,
            nightly_toolchain: String::new(),
            fuzz_mode: "smoke".into(),
            fuzz_seconds_per_target: 10,
            fuzz_max_len: 4096,
            fuzz_version: String::new(),
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
        let path = project.join(".infra/ci/ci.toml");
        let document: Option<toml::Value> = if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            Some(toml::from_str(&text)?)
        } else {
            None
        };
        let local = document.as_ref().and_then(|value| value.get("local"));
        let mut config: Self = local
            .cloned()
            .map(toml::Value::try_into)
            .transpose()
            .context("invalid [local] CI configuration")?
            .unwrap_or_default();
        let shared = SharedDefaults::load(project)?;
        for (key, expected) in [
            ("build_toolchain", shared.build_toolchain.as_str()),
            ("clippy_toolchain", shared.clippy_toolchain.as_str()),
            ("nightly_toolchain", shared.nightly_toolchain.as_str()),
            ("fuzz_version", shared.fuzz_version.as_str()),
        ] {
            if local
                .and_then(|value| value.get(key))
                .is_some_and(|value| value.as_str() != Some(expected))
            {
                bail!("{key} is managed by .infra/tools/defaults.toml; remove the project override");
            }
        }
        config.build_toolchain = Some(shared.build_toolchain);
        config.clippy_toolchain = Some(shared.clippy_toolchain);
        config.nightly_toolchain = shared.nightly_toolchain;
        config.fuzz_version = shared.fuzz_version;
        for path in [&config.matrix, &config.hook] {
            if path.is_empty()
                || Path::new(path)
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_) | std::path::Component::CurDir))
            {
                bail!("local CI paths must be project-relative: {path}");
            }
        }
        for value in [&config.build_toolchain, &config.clippy_toolchain]
            .into_iter()
            .flatten()
        {
            if value.is_empty() || value.starts_with('-') || value.chars().any(char::is_whitespace) {
                bail!("invalid Cargo toolchain: {value}");
            }
        }
        if config.nightly_toolchain.is_empty()
            || config.nightly_toolchain.starts_with(['+', '-'])
            || config.nightly_toolchain.chars().any(char::is_whitespace)
            || (config.nightly_toolchain.starts_with("nightly") && !is_pinned_nightly(&config.nightly_toolchain))
        {
            bail!("nightly_toolchain must be nightly-YYYY-MM-DD or a valid toolchain name");
        }
        if !matches!(config.fuzz_mode.as_str(), "smoke" | "build-only" | "disabled") {
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
