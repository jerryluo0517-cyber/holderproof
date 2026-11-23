# 外部用户 2 · macOS 测试结果

**2026 年 11 月 23 日 · macOS Apple Silicon · 环境调整后通过**

用户 2 提供了完整验收批次、手动查询和浏览器记录。测试基于提交 `383e7e1`，本轮为隔离安装的工具链临时调整了两个启动脚本的路径，随后完成验收。详细数据见[结构化摘要](external-user-02-macos.json)。

## 环境与测试过程

| 项目 | 记录 |
|---|---|
| 测试版本 | `383e7e116fd1193ee6d0b58715b60991fdbe8234` |
| 系统 | macOS，Darwin 25.6.0，Apple Silicon arm64 |
| 工具版本 | Python 3.10.22、Rust 1.99.0、Lyquor LDK 0.8.4、Foundry 1.7.1、Solidity 0.8.28 |
| 首次运行 | 默认 Python 未满足 3.10 要求，Rust/Cargo 和 `shaker` 未就绪。首次完整验收停在环境检查。 |
| 环境调整 | 选用可用的 Python 3.10，隔离安装所需工具，并临时调整 `scripts/setup-env.sh` 和 `scripts/start-devnet.sh` 的 LDK 路径。报告记录了测试结束后还原文件和关闭服务。 |
| 试用记录 | 附件包含技术日志、网页证据和测试报告。试用总耗时、独立操作或协助情况未记录。 |

## 验收结果

| 检查 | 结果 |
|---|---|
| 基础检查 | 9 项 fixture 检查、6 项 Python 单测和 Rust workspace 编译通过。 |
| Rust Guest 测试 | Indexer 21 项、Gate Core 15 项，均通过。 |
| 余额证书与 Gate | Alice 区块 `1000` 的余额为 `62685`，证书为 `Finalized`，有 3 名签名者。空投通过，治理未通过。两个 Gate 的查询哈希与证书一致。 |
| E3 与 EVM 对账 | 308 条 EVM Transfer 日志和 18 次历史余额读取与 Python 基准一致。领取、投票、撤销和节点暂停恢复用例通过。 |
| 周期检查点 | 区块 `250`、`500`、`750`、`1000` 四个检查点通过。 |
| 信用分档 | Alice `940 / VIP`、Dave `675 / Standard`、Bob `495 / Base`、Carol `250 / Base`。 |
| Sepolia 历史快照 | 区块 `12184649`、169 条 Transfer 记录、样例窗口余额 `0.002 WETH`，证书有 3 名签名者。 |
| 浏览器查询 | 区块 `1000` 返回 `62,685` 和两个资格结果。区块 `950` 返回 `57,685`，两个 Gate 显示 `Not evaluated`。 |

## 反馈与证据

首次启动需要核对 Python 版本和 Lyquor 工具链的安装位置。项目随后在 [`807ae72`](https://github.com/jerryluo0517-cyber/holderproof/commit/807ae72) 提供了 `LYQUOR_LDK_DIR` 和 `LYQUOR_BIN_DIR` 配置。这是对启动路径问题的后续修正，本记录的验收版本为 `383e7e1` 加上上述临时路径调整。

核对过的材料包括验收目录中的提交号、余额证书、Gate 结果、EVM 对账、检查点、信用和 Sepolia 记录，以及两个区块的网页状态。启动前的失败日志和测试后的清理日志一并保存在原证据包中。
