# rs-infra-ci

[![Rust CI](https://github.com/qubit-ltd/rs-infra-ci/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-infra-ci/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-infra-ci/coverage-badge.json)](https://qubit-ltd.github.io/rs-infra-ci/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-infra-ci.svg?color=blue)](https://crates.io/crates/qubit-infra-ci)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

根据项目本地 CI 配置编排相互独立的 Rust 基础设施工具。

## 安装

```bash
cargo install --git https://github.com/qubit-ltd/rs-infra-ci.git --locked qubit-infra-ci
```

## 快速开始

在 Rust 项目根目录查看命令帮助：

```bash
cargo run --manifest-path /path/to/rs-infra-ci/Cargo.toml -- --help
```

项目的 `.infra` 配置仍然是行为的唯一来源；工具仓库不会复制项目配置。具体策略由项目配置决定。

## 工作流契约

例如，一个 crate 需要测试最小 feature 集，并在打包前运行自己的集成检查。
本地与 GitHub Actions 可以使用同一入口：

```bash
rs-infra-ci --project /path/to/project plan
rs-infra-ci --project /path/to/project check
```

未指定任务列表时，按以下顺序执行：`style`、`clippy`、`coverage-cfg-clippy`、
`verify`、`feature-matrix`、`project-hook`、`package`、`coverage`、`audit`。
任一步失败都会停止后续任务。缺少矩阵或 hook、未开启 coverage cfg Clippy 时会
明确报告跳过。显式任务列表会替换默认列表；`--only` 按指定顺序执行已启用的任务。

在使用此工具的项目中配置 `.infra/ci.toml`：

```toml
tasks = ["style", "clippy", "coverage-cfg-clippy", "verify", "feature-matrix",
         "project-hook", "package", "coverage", "audit"]

[local]
build_toolchain = "1.94.0"
clippy_toolchain = "nightly-2026-06-05"
coverage_cfg_clippy = false
# CI 必须使用最新漏洞库、不允许缓存回退时，设为 false。
audit_cached_fallback = true
matrix = ".infra/ci/cargo-matrix.json"
hook = "project-ci-check.sh"
# 项目启用相应配置时，将高级任务加入 `tasks`。
nightly_toolchain = "nightly-2026-06-05"
fuzz_mode = "smoke"
fuzz_seconds_per_target = 10
fuzz_max_len = 4096
fuzz_version = "0.13.2"
```

工具链配置可省略，省略时使用当前 Cargo 工具链；只配置 `build_toolchain` 时，
Clippy 也使用它。文件路径相对于项目根目录。未知的 `[local]` 配置项或无效配置
会在执行前报错。选项来自 `.infra`，不读取旧的 `RS_CI_*` 或
`RUN_COVERAGE_CFG_CLIPPY` 环境变量。

| 任务 | 实际行为 |
| --- | --- |
| `style` | `rs-infra-style check` |
| `clippy` | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| `coverage-cfg-clippy` | 重复同一 Clippy 检查，仅对子进程设置 `RUSTFLAGS="--cfg coverage"` 并移除继承的 `CARGO_ENCODED_RUSTFLAGS`；默认关闭 |
| `verify` | 依次调用 `rs-infra-verify lock check` 和 build、test、doc、package suite；若显式选择了 `package`，打包仅在该任务的位置执行 |
| `feature-matrix` | 按配置文件顺序执行每个检查及其命令 |
| `project-hook` | 在项目根目录运行指定的普通可执行文件；不存在则跳过，文件无效或非零退出则失败；Windows 使用 Bash |
| `miri` | 安装指定 nightly 的 Miri/rust-src 组件，运行 `miri setup`，再执行已配置的 Miri suite |
| `address-sanitizer` | 安装指定 nightly 的 rust-src 组件，再执行已配置的 AddressSanitizer suite |
| `fuzz` | 确保安装固定版本 cargo-fuzz，再按配置的模式及限制运行 fuzz suite；`disabled` 会跳过安装和执行 |
| `loom` | 使用 `RUSTFLAGS="--cfg loom"` 执行已配置的 release Loom 模型 |
| `package` | `rs-infra-verify run --suite package` |
| `coverage` | `rs-infra-coverage collect`，不再仅调用配置检查 `check` |
| `audit` | `cargo audit`；仅识别到漏洞库获取错误时才允许用 `--no-fetch --stale` 重试一次；漏洞或重试失败均阻断 CI |
| `pages` / `dependency` | 显式开启：`rs-infra-pages build` / `rs-infra-dependency check` |

使用 `.infra/ci/cargo-matrix.json` 配置 feature 和依赖兼容性检查：

```json
{
  "version": 1,
  "checks": [
    {"name": "minimal", "commands": ["check", "test", "clippy"], "defaultFeatures": false},
    {"name": "all", "commands": ["test", "doc", "doc-test"], "allFeatures": true}
  ]
}
```

支持 `check`、`build`、`test`、`doc`、`doc-test` 和 `clippy`。
`features` 指定 feature，`packages` 指定 workspace 中的包；`packages` 一旦出现
就不能为空，省略时使用 `--workspace`。`allFeatures` 不能与指定 feature 或
`defaultFeatures: false` 同时使用。矩阵 Clippy 带有 `--all-targets -- -D warnings`，
文档检查设置 `RUSTDOCFLAGS="-D warnings"`，文档测试调用 `cargo test --doc`。

单个检查可以设置 `"dependency": {"name": "example", "resolution": "precise",
"version": "1.2.3"}`，或 `"dependency": {"name": "example", "resolution": "latest"}`。
工具先更新依赖，验证只解析到一个版本（`precise` 还要求版本完全匹配），再以
`--locked` 执行检查。每次依赖检查结束后恢复原 Cargo.lock，命令失败也会恢复；
原先不存在锁文件时会删除本次生成的锁文件。各检查使用独立的
`target/infra-feature-matrix/<name>` 产物目录。进程被强制终止时可能无法恢复；
不要在同一项目中并发写入锁文件。

迁移脚本运行 `rs-infra-ci --project . plan` 可以查看完整 job 计划。
存在 `.infra/ci/tools.toml` 时，计划还会输出每个选中工具的固定 revision
安装命令：

```toml
[rs-infra-style]
source = "https://github.com/qubit-ltd/rs-infra-style.git"
revision = "0123456789abcdef0123456789abcdef01234567"
binary = "rs-infra-style"
package = "qubit-infra-style"
```

工具 revision 必须是完整 Git SHA。安装使用带 `--rev` 和 `--locked` 的
`cargo install --git`。工具调用独立的 rs-infra-* 二进制、Cargo 和项目自有 hook，
不加载旧 rs-ci 运行时。`RS_INFRA_BIN_DIR` 仅用于定位基础设施二进制，Cargo 从
PATH 查找。完整默认流程需要预先安装 Cargo、Clippy、cargo-audit、所选基础设施
工具及覆盖率工具要求的组件。本工具输出安装计划，不自动安装这些工具。

Rust API 的 `jobs()` 返回静态模板；`workflow()` 根据项目配置展开命令，供
`plan` 与 `check` 共同使用。`CommandSpec.env` 表示仅对子进程生效的环境覆盖。

## 能力与限制

审核的旧脚本 revision 为 `ac84c7b4a439f703a6dee46faec805105ab5d714`，
依次执行：锁文件同步、格式/Clippy（可选 coverage cfg）、style、
debug/release 构建、默认/all-feature 测试、条件 Miri/sanitizer/fuzz/Loom、
严格文档、README 版本检查、feature matrix、项目 hook、package、coverage、audit。

本次补齐真实矩阵执行、严格 Clippy、coverage cfg、项目 hook、audit 及条件高级 suite 编排。
已有显式 `verify` 调用仍保留打包检查。新的默认顺序先运行 style，并将矩阵和
hook 放在独立的 package 任务之前。锁文件仍只校验，不执行旧脚本的自动同步。
构建、测试、文档和打包的语义取决于安装的 `rs-infra-verify` revision，必须选择
具备所需 release 构建、文档及实际打包验证能力的版本。条件 Miri、
AddressSanitizer、fuzz 和 Loom 检查是可选任务；项目存在对应配置时，必须将
任务加入 `.infra/ci.toml`。编排器会安装所需 nightly 组件和固定版本 cargo-fuzz。
README 版本检查、Cargo home 管理和构建产物清理仍需由相应工具或工作流负责。
除非项目启用了全部必需任务，编排器通过不能单独证明与旧 CI 完全等价。

本仓库的 `./ci-check.sh` 通过自身二进制和已提交的 `.infra` 配置执行，
保留原有 all-feature 测试与严格 Clippy 门禁，无须另外安装基础设施工具。

## 延伸阅读

可通过命令帮助和源码测试了解实际接口。切换到 [English README](README.md)。

## 测试

```bash
# 使用默认 feature 集运行测试
cargo test

# 使用项目声明的全部 feature 集运行测试
cargo test --all-features

# 运行项目 CI 检查
./ci-check.sh

# 检查代码覆盖率
./coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./align-ci.sh`格式化代码，运行`./ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-infra-ci](https://github.com/qubit-ltd/rs-infra-ci)
