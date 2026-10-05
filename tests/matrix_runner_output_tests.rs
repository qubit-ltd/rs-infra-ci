// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::fs;
use std::process::Command;

use tempfile::tempdir;

#[test]
fn matrix_plan_exports_its_dynamic_runner_binary() {
    let project = tempdir().expect("project directory");
    let output = tempdir().expect("output directory");
    let matrix = output.path().join("feature-matrix.json");
    let runner = output.path().join("bin/rs-infra-ci");

    fs::create_dir_all(project.path().join(".infra/ci")).expect("CI directory");
    fs::write(
        project.path().join(".infra/ci/ci.toml"),
        "tasks = ['feature-matrix']\n",
    )
    .expect("CI configuration");

    let result = Command::new(env!("CARGO_BIN_EXE_rs-infra-ci"))
        .args([
            "--project",
            project.path().to_str().expect("project path"),
            "matrix",
            "plan",
            "--output",
            matrix.to_str().expect("matrix output path"),
            "--runner-output",
            runner.to_str().expect("runner output path"),
        ])
        .output()
        .expect("run matrix planning");

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(fs::read_to_string(matrix).unwrap(), "{\"include\":[]}\n");
    assert!(runner.is_file(), "matrix runner binary should be exported");
    assert_eq!(
        fs::read(runner).unwrap(),
        fs::read(env!("CARGO_BIN_EXE_rs-infra-ci")).unwrap()
    );
}
