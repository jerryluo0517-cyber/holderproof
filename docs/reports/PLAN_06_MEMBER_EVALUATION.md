# HolderProof 个人评测报告

用户 1 在 2-of-3 版本的终端查询中查到 Alice 在 fixture 区块 1000 的余额为 **62685**，证书状态为 `Finalized`。同一阶段的团队自动基准使用 `db1532f`，五次新查询的中位耗时为 **58.10 ms**。当前 3-of-4 版本的评测见[课程报告初稿](CS5298_FINAL_REPORT_DRAFT.md)。

## 用户 1 操作记录

| 记录项 | 用户 1 终端输出 |
|---|---|
| 查询 | Alice（`0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa`）在 fixture 区块 `1000` 的持仓 |
| 余额与状态 | 余额 `62685`，状态 `Finalized`，列出 `2` 名不同签名者 |
| 检查点 | `holderproof-fixture-5298-v1`，区块范围 `0–1000`，覆盖 `308` 条事件 |
| 证书 ID 与查询哈希 | 两者均为 `1613062534e5d67e47f7bff3afe047f17be2e8f724311d9005eb41681279a884` |
| 网络状态 | `Lyquid-ahjg4huwvban3bvhxue7rcbaw4mnltwyolhqa@11` |
| 查询结束区块哈希 | `0x1f1057f10cbab9cb537ec95ad57e4ccd5ba52ef4efd7d00dfd3aedb64e65a28e` |
| 事件 Merkle 根 | `2e89babbc7b45637458bd08a2d843087354bf175eb559e5c1ddc7fcc0e79617b` |
| Gate 判定、命令耗时与操作反馈 | 本次提供的输出中未记录 |

用户 1 提供了终端证书。余额、事件数和 Merkle 根与 fixture 标准答案一致。

## 自动复现环境与流程

| 项目 | 条件 |
|---|---|
| 代码版本 | `db1532f` |
| 环境 | macOS 27.0 arm64、Python 3.12.10、Rust 1.99.0、Lyquor LDK 0.8.4、Foundry 1.7.1 |
| 数据 | 固定 ERC-20 fixture，308 条 Transfer 记录 |
| 部署 | 同机 localnet，四个基础设施节点，其中三个加入业务委员会，门槛 2-of-3 |
| 正确性基准 | Python fixture 标准答案及方案 05 的本地 EVM 历史余额对照 |

用 `./scripts/setup-env.sh --check` 检查环境，运行 `make demo` 启动服务。
通过 `run-proof.sh` 重复查询 Alice 在区块 1000 的持仓，并在两个 Gate 核对资格结果。
随后串行执行五次首次查询，记录查询确认和出证时间。
[测试操作指引](../guides/TESTING.md)

## 自动复现与既有验收结果

| 指标 | 结果与证据 |
|---|---|
| 正确性 | 五个新查询均返回与 Python 一致的余额。既有 P0 验收还验证了 308 条本地 EVM Transfer 日志和 18 次历史持仓余额读取 |
| 查询幂等 | 重复查询返回与 demo 初始化时相同的证书 JSON |
| 可组合性 | 对同一 `query_hash`，空投以 `60000` 为门槛返回 `true`，治理以 `70000` 为门槛返回 `false`。两个 Gate 的 `queryHash` 均与证书一致 |
| 鲁棒性 | 既有 05 验收覆盖错误字段、签名不足、重复动作和撤销。暂停一名成员仍能出证，暂停两名成员时观察窗口内没有出证，恢复后可以出证 |

鲁棒性和历史余额数据来自 2026 年 11 月 20 日的 `make e3` 验收。
该轮有 25 项 Rust guest 测试、6 项 Python 测试、9 项 fixture 检查通过。
[方案 05 自审计记录](PLAN_05_E3_AUDIT.md)

## 自动基准的查询和出证耗时

启动后用单调时钟计时。`proof` 提交到查询可读为「查询确认」，
`report_balance` 发起到 `Finalized` 证书可读为「认证出证」。
总耗时由两段原始计时相加后四舍五入。

| 持有者 | fixture 区块 | Rust 与 Python 一致的余额 | 签名者 | 查询确认 ms | 认证出证 ms | 总耗时 ms |
|---|---:|---:|---:|---:|---:|---:|
| Alice | 900 | 47685 | 2 | 23.12 | 33.44 | 56.56 |
| Alice | 950 | 57685 | 2 | 23.40 | 34.22 | 57.63 |
| Bob | 1000 | 51886 | 2 | 23.88 | 34.22 | 58.10 |
| Carol | 1000 | 43249 | 2 | 22.76 | 54.75 | 77.50 |
| Dave | 1000 | 51856 | 2 | 44.74 | 54.72 | 99.46 |
| **中位数** | | | | **23.40** | **34.22** | **58.10** |

环境检查用时 **0.11 秒**，`make demo` 启动并完成内置断言用时 **13.71 秒**。
重复查询的完整 CLI 调用用时 **88.45 ms**。表中的五个样本是在同一台机器、
服务已启动的条件下测得的串行首次查询结果。

## 自动复现证据

原始计时、完整证书和两个 Gate 的结果见本地 `.run/evaluation/member-evaluation.json`。
同一目录保存采集脚本和启动日志。
[团队外用户试用任务单](../guides/EXTERNAL_USER_TEST.md)
