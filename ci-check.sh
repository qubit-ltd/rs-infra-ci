#!/usr/bin/env bash
set -euo pipefail
project_root=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)
exec cargo run --quiet --manifest-path "$project_root/Cargo.toml" -- --project "$project_root" check
