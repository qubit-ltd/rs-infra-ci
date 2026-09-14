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
cargo install --git https://github.com/qubit-ltd/rs-infra-ci.git --tag v0.1.0 qubit-infra-ci
```

## Quick Start

From a Rust project root:

```bash
cargo run --manifest-path /path/to/rs-infra-ci/Cargo.toml -- --help
```

The project's `.infra` configuration remains the source of truth; this tool does not copy project configuration into the tool repository.

## Workflow contract

`.infra/ci.toml` selects the jobs that a migration script should run:

```toml
tasks = ["style", "verify", "coverage", "pages", "dependency"]
```

`verify` expands to lock validation followed by the build, test, documentation,
and package suites. The other task names map to `rs-infra-style check`,
`rs-infra-coverage check`, `rs-infra-pages build`, and
`rs-infra-dependency check`.

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
with `--rev` and `--locked`; rs-infra-ci invokes only the independent
rs-infra-* binaries and has no dependency on the legacy rs-ci runtime.

## Capabilities and limitations

This first release provides the focused behavior described above. It is intentionally a small building block: project-specific policy belongs in `.infra`, and orchestration belongs in `rs-infra-ci`. It does not promise compatibility with the legacy `rs-ci` scripts beyond the commands currently covered by tests.

## Learn More

See the command help and source tests for the supported interface. Switch to [中文文档](README.zh_CN.md).

## Testing

```bash
cargo test
cargo test --all-features
./ci-check.sh
./coverage.sh
```

## License

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the full license text.

## Contributing

Contributions are welcome. Please follow the Rust API guidelines, keep public API documentation and tests current, and run `./align-ci.sh` to format code and `./ci-check.sh` to satisfy CI requirements before submitting a pull request.

## Author

**Haixing Hu** - *Qubit Co. Ltd.*

Repository: [https://github.com/qubit-ltd/rs-infra-ci](https://github.com/qubit-ltd/rs-infra-ci)
