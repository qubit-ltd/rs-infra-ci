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
cargo install --git https://github.com/qubit-ltd/rs-infra-ci.git --tag v0.1.0 qubit-infra-ci
```

## 快速开始

在 Rust 项目根目录查看命令帮助：

```bash
cargo run --manifest-path /path/to/rs-infra-ci/Cargo.toml -- --help
```

项目的 `.infra` 配置仍然是行为的唯一来源；工具仓库不会复制项目配置。具体策略由项目配置决定。

## 工作流契约

迁移脚本可以通过 `.infra/ci.toml` 选择要生成的任务：

```toml
tasks = ["style", "verify", "coverage", "pages", "dependency"]
```

`verify` 会展开为锁文件检查，以及 build、test、doc、package 四套验证。
其余任务分别调用 `rs-infra-style check`、`rs-infra-coverage check`、
`rs-infra-pages build` 和 `rs-infra-dependency check`。

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
`cargo install --git`；rs-infra-ci 只调用独立的 rs-infra-* 二进制，不依赖旧
rs-ci 运行时。

## 能力与限制

当前版本只提供上文列出的专门能力，刻意保持为小型基础设施组件：项目策略放在 `.infra`，任务编排交给 `rs-infra-ci`。对于旧版 `rs-ci` 脚本，只有测试覆盖的命令可视为兼容。

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
