// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Cargo compatibility matrices with isolated artifacts and restored lockfiles.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use serde_json::Value;
use serde_json::from_slice;

use crate::CommandSpec;
use crate::local;
use crate::local_config::LocalConfig;

/// Reads and validates the complete matrix before any command can mutate the project.
fn checks(project: &Path, config: &LocalConfig) -> Result<Vec<Value>> {
    let path = project.join(&config.matrix);
    if !path.try_exists()? {
        return Ok(Vec::new());
    }
    let document: Value = from_slice(&std::fs::read(&path)?)
        .with_context(|| format!("invalid matrix {}", path.display()))?;
    if document["version"] != 1 {
        bail!("matrix version must be 1");
    }
    let checks = document["checks"]
        .as_array()
        .context("matrix checks must be an array")?;
    if checks.is_empty() {
        bail!("matrix checks must not be empty");
    }
    let mut names = BTreeSet::new();
    for check in checks {
        let name = check["name"]
            .as_str()
            .context("matrix check name must be a string")?;
        if !identifier(name, true) || !names.insert(name) {
            bail!("invalid or duplicate matrix check name: {name}");
        }
        let commands = strings(check, "commands")?;
        if commands.is_empty()
            || commands.iter().any(|command| {
                !matches!(
                    command.as_str(),
                    "check" | "build" | "test" | "doc" | "doc-test" | "clippy"
                )
            })
        {
            bail!("invalid matrix commands for {name}");
        }
        let features = strings(check, "features")?;
        if features.iter().any(|feature| {
            feature.is_empty()
                || !feature
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_+./-".contains(&byte))
        }) {
            bail!("invalid matrix features for {name}");
        }
        let packages = strings(check, "packages")?;
        if (check.get("packages").is_some() && packages.is_empty())
            || packages.iter().any(|package| !identifier(package, false))
            || packages.iter().collect::<BTreeSet<_>>().len() != packages.len()
        {
            bail!("invalid matrix packages for {name}");
        }
        let defaults = boolean(check, "defaultFeatures", true)?;
        if boolean(check, "allFeatures", false)? && (!defaults || !features.is_empty()) {
            bail!("allFeatures conflicts with feature selection for {name}");
        }
        if let Some(dependency) = check.get("dependency") {
            let name = dependency["name"]
                .as_str()
                .context("dependency name must be a string")?;
            if !identifier(name, false) {
                bail!("invalid dependency name");
            }
            match dependency["resolution"].as_str() {
                Some("latest") if dependency.get("version").is_none() => (),
                Some("precise")
                    if dependency["version"]
                        .as_str()
                        .is_some_and(|value| identifier(value, true) && !value.contains('/')) => {}
                _ => bail!("dependency resolution must be latest or precise with a version"),
            }
        }
    }
    Ok(checks.clone())
}

/// Checks identifiers used as package names or isolated artifact directory names.
fn identifier(value: &str, dots: bool) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || b"_-".contains(&byte) || (dots && b".+".contains(&byte))
        })
}

/// Extracts a string array, rejecting null and non-string elements.
fn strings(value: &Value, key: &str) -> Result<Vec<String>> {
    match value.get(key) {
        None => Ok(Vec::new()),
        Some(value) => value
            .as_array()
            .with_context(|| format!("{key} must be an array"))?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .with_context(|| format!("{key} entries must be strings"))
            })
            .collect(),
    }
}

/// Reads a boolean option without confusing explicit false with absence.
fn boolean(value: &Value, key: &str, default: bool) -> Result<bool> {
    value
        .get(key)
        .map(|value| {
            value
                .as_bool()
                .with_context(|| format!("{key} must be boolean"))
        })
        .unwrap_or(Ok(default))
}

/// Produces the Cargo commands for one validated check, including dependency preparation.
fn commands(project: &Path, config: &LocalConfig, check: &Value) -> Result<Vec<CommandSpec>> {
    let mut result = Vec::new();
    if let Some(dependency) = check.get("dependency") {
        let mut args = vec![
            "update".into(),
            "--package".into(),
            dependency["name"]
                .as_str()
                .context("dependency name")?
                .into(),
        ];
        if dependency["resolution"] == "precise" {
            args.extend([
                "--precise".into(),
                dependency["version"]
                    .as_str()
                    .context("dependency version")?
                    .into(),
            ]);
        }
        result.push(local::cargo(config, args));
        result.push(local::cargo(
            config,
            vec![
                "metadata".into(),
                "--locked".into(),
                "--format-version".into(),
                "1".into(),
            ],
        ));
    }
    let mut selection = Vec::new();
    let packages = strings(check, "packages")?;
    if packages.is_empty() {
        selection.push("--workspace".into());
    }
    for package in packages {
        selection.extend(["--package".into(), package]);
    }
    if boolean(check, "allFeatures", false)? {
        selection.push("--all-features".into());
    } else {
        if !boolean(check, "defaultFeatures", true)? {
            selection.push("--no-default-features".into());
        }
        let features = strings(check, "features")?;
        if !features.is_empty() {
            selection.extend(["--features".into(), features.join(",")]);
        }
    }
    if check.get("dependency").is_some() {
        selection.push("--locked".into());
    }
    for command in strings(check, "commands")? {
        let mut args = match command.as_str() {
            "doc" => vec!["doc".into(), "--no-deps".into()],
            "doc-test" => vec!["test".into(), "--doc".into()],
            "clippy" => vec!["clippy".into(), "--all-targets".into()],
            _ => vec![command.clone()],
        };
        args.extend(selection.clone());
        if command == "clippy" {
            args.extend(["--".into(), "-D".into(), "warnings".into()]);
        }
        let mut spec = local::cargo(config, args);
        if command == "doc" {
            spec.env.insert("RUSTDOCFLAGS".into(), "-D warnings".into());
        }
        result.push(spec);
    }
    let target = project
        .join("target/infra-feature-matrix")
        .join(check["name"].as_str().context("check name")?);
    for command in &mut result {
        command.env.insert(
            "CARGO_TARGET_DIR".into(),
            target.to_string_lossy().into_owned(),
        );
    }
    Ok(result)
}

/// Expands every validated matrix command for the public workflow plan.
pub(crate) fn plan(project: &Path, config: &LocalConfig) -> Result<Vec<CommandSpec>> {
    checks(project, config)?
        .iter()
        .map(|check| commands(project, config, check))
        .collect::<Result<Vec<_>>>()
        .map(|groups| groups.into_iter().flatten().collect())
}

/// Runs matrix checks sequentially, restoring the entry lockfile even on command failure.
/// Dependency metadata must resolve exactly one matching version before tests start.
pub(crate) fn run(project: &Path, config: &LocalConfig) -> Result<()> {
    let checks = checks(project, config)?;
    let lock = project.join("Cargo.lock");
    let baseline = match std::fs::read(&lock) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    for check in checks {
        let dependency = check.get("dependency");
        let result = (|| {
            for command in commands(project, config, &check)? {
                if dependency.is_some()
                    && command
                        .args
                        .iter()
                        .find(|arg| !arg.starts_with('+'))
                        .is_some_and(|arg| arg == "metadata")
                {
                    let output = local::process(project, &command)
                        .output()
                        .context("matrix dependency metadata")?;
                    if !output.status.success() {
                        bail!(
                            "matrix metadata failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                    let metadata: Value = from_slice(&output.stdout)?;
                    let dependency = dependency.context("dependency configuration")?;
                    let versions: BTreeSet<_> = metadata["packages"]
                        .as_array()
                        .context("metadata packages")?
                        .iter()
                        .filter(|package| package["name"] == dependency["name"])
                        .filter_map(|package| package["version"].as_str())
                        .collect();
                    if versions.len() != 1
                        || (dependency["resolution"] == "precise"
                            && !versions.contains(
                                dependency["version"]
                                    .as_str()
                                    .context("dependency version")?,
                            ))
                    {
                        bail!("matrix dependency resolved an unexpected version: {versions:?}");
                    }
                } else {
                    local::execute(project, &command)?;
                }
            }
            Ok(())
        })();
        if dependency.is_some() {
            let restored = match &baseline {
                Some(bytes) => std::fs::write(&lock, bytes),
                None => match std::fs::remove_file(&lock) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    result => result,
                },
            };
            restored.with_context(|| {
                format!(
                    "failed to restore {}; matrix result: {result:?}",
                    lock.display()
                )
            })?;
        }
        result?;
    }
    Ok(())
}
