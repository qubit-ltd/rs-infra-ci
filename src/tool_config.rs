// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::ToolSpec;

/// Revision-pinned tools keyed by their installed executable name.
///
/// # Examples
///
/// ```
/// use qubit_infra_ci::ToolConfig;
///
/// let config = ToolConfig::default();
/// assert!(config.tools.is_empty());
/// ```
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq)]
pub struct ToolConfig {
    /// Tool specifications keyed by installed executable name.
    pub tools: BTreeMap<String, ToolSpec>,
}
