# rs-infra-ci

[![Rust CI](https://github.com/qubit-ltd/rs-infra-ci/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-infra-ci/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-infra-ci/coverage-badge.json)](https://qubit-ltd.github.io/rs-infra-ci/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-infra-ci.svg?color=blue)](https://crates.io/crates/qubit-infra-ci)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![中文文档](https://img.shields.io/badge/文档-中文版-blue.svg)](README.zh_CN.md)

Orchestrate independent Rust infrastructure tools from a project-local CI configuration.

## Installation

```bash
cargo install --git https://github.com/qubit-ltd/rs-infra-ci.git --locked qubit-infra-ci
```

## Quick Start

From a Rust project root:

```bash
cargo run --manifest-path /path/to/rs-infra-ci/Cargo.toml -- --help
```

The project's `.infra` configuration remains the source of truth; this tool does not copy project configuration into the tool repository.

## Workflow contract

For a crate that must test minimal features and run its own integration hook,
use the same command locally and in GitHub Actions:

```bash
rs-infra-ci --project /path/to/project plan
rs-infra-ci --project /path/to/project check
```

Without an explicit task list, checks run in this order:
`style`, `clippy`, `coverage-cfg-clippy`, `verify`, `feature-matrix`,
`project-hook`, `package`, `coverage`, `audit`. A failure stops subsequent tasks.
Missing matrices/hooks and disabled coverage-configured Clippy are reported as
skipped. An explicit list replaces the defaults; `--only` selects enabled tasks
in the requested order.

Configure the consumer project's `.infra/ci.toml`:

```toml
tasks = ["style", "clippy", "coverage-cfg-clippy", "verify", "feature-matrix",
         "project-hook", "package", "coverage", "audit"]

[local]
build_toolchain = "1.94.0"
clippy_toolchain = "nightly-2026-06-05"
coverage_cfg_clippy = false
# Set false when CI must use current advisory data without a cached fallback.
audit_cached_fallback = true
matrix = ".infra/ci/cargo-matrix.json"
hook = "project-ci-check.sh"
# Add configured suites to `tasks` when the project opts in to them.
nightly_toolchain = "nightly-2026-06-05"
fuzz_mode = "smoke"
fuzz_seconds_per_target = 10
fuzz_max_len = 4096
fuzz_version = "0.13.2"
```

The toolchain keys are optional: omission uses the active Cargo toolchain.
Clippy falls back to `build_toolchain` when only that key is specified.
Paths are relative to the project root. Unknown `[local]` keys and invalid
configuration fail before execution. Options come from `.infra`, not legacy
`RS_CI_*` or `RUN_COVERAGE_CFG_CLIPPY` variables.

| Task | Executed behavior |
| --- | --- |
| `style` | `rs-infra-style check` |
| `clippy` | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| `coverage-cfg-clippy` | The same Clippy invocation with child-only `RUSTFLAGS="--cfg coverage"`; disabled by default; inherited `CARGO_ENCODED_RUSTFLAGS` is removed for that child |
| `verify` | `rs-infra-verify lock check`, then build, test, doc and package suites; when `package` is explicitly selected, packaging runs only at that task's position |
| `feature-matrix` | Execute each configured check and each command in file order |
| `project-hook` | Run the configured regular, executable file from the project root; absence skips, invalid files or nonzero exit fail; Windows uses Bash |
| `miri` | Install the configured nightly Miri/rust-src components, run `miri setup`, then execute the configured Miri suite |
| `address-sanitizer` | Install the configured nightly rust-src component, then execute the configured AddressSanitizer suite |
| `fuzz` | Ensure the pinned cargo-fuzz version, then run the configured fuzz suite with the selected mode and smoke limits; `disabled` skips installation and execution |
| `loom` | Run configured release Loom models with `RUSTFLAGS="--cfg loom"` |
| `package` | `rs-infra-verify run --suite package` |
| `coverage` | `rs-infra-coverage collect`, rather than configuration-only `check` |
| `audit` | `cargo audit`; only recognized database-fetch failures may retry once with `--no-fetch --stale`; vulnerability and retry failures stop CI |
| `pages` / `dependency` | Explicit opt-ins: `rs-infra-pages build` / `rs-infra-dependency check` |

Use `.infra/ci/cargo-matrix.json` for feature and dependency compatibility:

```json
{
  "version": 1,
  "checks": [
    {"name": "minimal", "commands": ["check", "test", "clippy"], "defaultFeatures": false},
    {"name": "all", "commands": ["test", "doc", "doc-test"], "allFeatures": true}
  ]
}
```

Supported commands are `check`, `build`, `test`, `doc`, `doc-test`, and `clippy`.
`features` selects named features; `packages` selects workspace packages and
must be nonempty if present. Without `packages`, checks use `--workspace`.
`allFeatures` cannot be combined with named features or `defaultFeatures: false`.
Matrix Clippy uses `--all-targets -- -D warnings`; matrix docs use
`RUSTDOCFLAGS="-D warnings"`; doctests use `cargo test --doc`.

A check may set `"dependency": {"name": "example", "resolution": "precise",
"version": "1.2.3"}` or `"dependency": {"name": "example", "resolution": "latest"}`.
The runner updates that dependency, verifies exactly one resolved version
(and an exact match for `precise`), then runs the check with `--locked`.
It restores the original Cargo.lock after each dependency check, including
command failure, and removes a newly generated lockfile if none existed.
Each check uses `target/infra-feature-matrix/<name>` for isolated artifacts.
Abrupt process termination can prevent restoration; do not run concurrent
lockfile writers in the same project.

Migration scripts can run `rs-infra-ci --project . plan` to inspect the complete
job plan. When `.infra/ci/tools.toml` is present, the plan also includes the
revision-pinned installation command for every selected tool:

```toml
[rs-infra-style]
source = "https://github.com/qubit-ltd/rs-infra-style.git"
revision = "0123456789abcdef0123456789abcdef01234567"
binary = "rs-infra-style"
package = "qubit-infra-style"
```

Tool revisions must be full Git SHAs. Installation uses `cargo install --git`
with `--rev` and `--locked`; rs-infra-ci invokes independent rs-infra-* binaries, Cargo, and project-owned
hooks without loading the legacy rs-ci runtime. `RS_INFRA_BIN_DIR` locates only
infrastructure binaries; Cargo is resolved through PATH. Install Cargo,
Clippy, cargo-audit, the selected infrastructure binaries, and the coverage
tool's prerequisites before running the full default pipeline. This command
prints installation plans but does not install tools automatically.

The Rust API `jobs()` returns static templates; `workflow()` expands the
project configuration into the same commands used by `plan` and `check`.
`CommandSpec.env` contains child-process environment overrides.

## Capabilities and limitations

The audited legacy script at revision `ac84c7b4a439f703a6dee46faec805105ab5d714`
ran lock synchronization, formatting/Clippy (optional
coverage cfg), style, debug/release builds, default/all-feature tests,
conditional Miri/sanitizer/fuzz/Loom, strict docs, README version checks,
feature matrix, project hook, package, coverage, and audit, in that order.

This change restores real matrix execution, strict Clippy, coverage cfg,
project hooks, audit, and conditional advanced suite execution inside the generic orchestrator. Packaging remains
available to existing explicit `verify` callers. The new default sequence
puts style first and keeps matrix/hook before the explicit package task.
Lock validation remains read-only instead of the old automatic lock sync.
Build/test/doc/package semantics depend on the installed `rs-infra-verify`
revision: select one that supplies the required release-build, documentation,
and actual-package-build guarantees. Conditional Miri, AddressSanitizer, fuzz,
and Loom checks are opt-in tasks and must be included in `.infra/ci.toml` when
the project has corresponding configuration. The orchestrator installs the
required nightly components and pinned cargo-fuzz version. README version
checks, Cargo home management, and build-artifact cleanup remain outside this
tool; configure them in the appropriate independent tools/workflows. A passing
orchestrator run alone does not establish full legacy CI parity unless the
project enables all of its required tasks.

This repository bootstraps `./ci-check.sh` through its own binary and checked-in
`.infra` configuration, retaining its existing all-feature tests and strict
Clippy gates without requiring separately installed infrastructure tools.

## Learn More

See the command help and source tests for the supported interface. Switch to [中文文档](README.zh_CN.md).

## Testing

```bash
# Run tests with the default feature set
cargo test

# Run tests with all declared features
cargo test --all-features

# Project CI checks
./ci-check.sh

# Check code coverage
./coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the
full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public API documentation and tests current, and run `./align-ci.sh` to format code and `./ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-infra-ci](https://github.com/qubit-ltd/rs-infra-ci)
