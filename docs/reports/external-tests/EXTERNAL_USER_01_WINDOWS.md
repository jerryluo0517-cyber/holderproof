# 外部用户 1 测试结果

**2026 年 11 月 23 日 · 原生 Windows（Git Bash） · 未完成，环境受阻**

外部用户 1 在 GitHub `main` 的 `34e1fad` 上进行测试。`make check` 通过。当前测试环境缺少 Lyquor LDK 和 `shaker`，在启动本地 devnet 前受阻。此次记录适合用于评估 Windows 用户的环境准备步骤。详情见[结构化结果](external-user-01-windows.json)。

## 环境

| 项目 | 测试记录 |
|---|---|
| 版本 | `34e1fad2562d7003c51c6025b5aca536c5a9d2b0` |
| 主机与终端 | 原生 Windows，Git Bash |
| 系统构建号 | `22631.6199`。微软资料将该构建号归为 Windows 11 23H2，原记录填写 Windows 10，具体版本待测试者用 `winver` 核对。 |
| 工具 | Python 3.12.1、Rust 1.99.0（GNU 工具链）、GNU Make 4.4.1，已安装 `wasm32-unknown-unknown` 目标。 |
| 环境障碍 | `./scripts/setup-env.sh --check` 退出码为 1，提示缺少 Lyquor LDK。 |

[微软构建号说明](https://support.microsoft.com/en-us/servicing/os/windows-11/2025/11/november-11-2025-kb5068865-os-build-22631-6199)

## 检查结果

| 检查 | 结果 |
|---|---|
| `make check` | 退出码 0。重新生成 308 条 fixture 事件，9 项 fixture 校验和 6 项 Python 单元测试通过，Rust workspace 编译检查通过。 |
| 历史余额基准 | Alice 在区块 `900`、`950`、`1000` 的 fixture 余额分别为 `47685`、`57685`、`62685`。 |
| Rust WASM 类型检查 | `cargo check --workspace --all-targets --target wasm32-unknown-unknown` 通过，覆盖业务与测试目标的类型检查。 |
| `make guest-test` | 命令退出码为 2，找不到 `shaker`。36 项 Guest 测试均未执行。 |
| `make demo` | 命令退出码为 2，缺少 `shaker`。实时证书、两个 Gate、Web 查询与 E3 验收均未启动。 |
| Rust 格式与静态检查 | `cargo fmt --all -- --check` 在 11 个文件报告 31 处格式差异。Clippy 退出码为 0，共报告 6 条警告。 |

## 反馈与证据

测试者建议下一轮使用 WSL2 + Ubuntu、原生 Linux 或 macOS 跑完整流程。本轮记录的结果来自原生 Windows，WSL2 运行情况待复测。这条反馈可用于在试用指南中提前说明 LDK 的平台要求。

本记录整理自参与者提供的文字报告和 JSON 摘要，已去除本机目录。原始日志待归档，优先保留环境检查、`make check` 和 `make demo` 的输出。
