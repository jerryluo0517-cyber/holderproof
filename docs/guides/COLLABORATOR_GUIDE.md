# HolderProof 协作与复现指南

更新日期：2026-11-15。当前基线为执行方案 03/04 的本地 fixture P0。
请先阅读 README；原有 dated PDF 和早期操作日志仅反映历史版本。

## 1. 当前可以复现什么

三个委员会成员独立重放余额与 checkpoint，由两份一致批准完成原生认证。
空投和治理两个独立 Lyquid 复用同一证书，支持各自门槛下的合格与不合格结果。

| 验收内容 | 当前结果 |
|---|---|
| Rust guest 测试 | 24 项通过 |
| Python 标准答案测试 | 6 项通过 |
| fixture 检查 | 9 项通过 |
| 双消费者、双结果 | 网络环境下通过 |
| 重复相同查询 | 原证书保持不变 |
| 实例历史 | 两个消费者均有本地历史记录 |
| 历史 EVM 对照 | 308 条日志、18 次历史余额读取一致 |

## 2. 环境与首次启动

已验证环境为 Rust 1.99.0、Lyquor LDK 0.8.4、Foundry 1.7.1、Python 3.12。

```bash
./scripts/setup-env.sh --check
# 缺少依赖时再运行安装模式：
./scripts/setup-env.sh
```

首次编译 EVM 测试合约会下载固定版本 Solidity 0.8.28；后续对照可以使用已缓存编译器。
guest VM 需要能够初始化内存管理器；受限沙箱可能阻止 VM 或本地监听端口启动。

```bash
make acceptance
```

验收启动官方四节点基础设施，但业务委员会只有前三个成员，门槛为 2-of-3。
需要空闲端口 8545、10087、11087、12087、13087、8080、8081 和 8082。
历史 EVM 检查另外启动隔离 Anvil，并在结束时自动停止它。

## 3. 查看结果与提交查询

| 地址 | 内容 |
|---|---|
| `http://127.0.0.1:8081/api/certificate` | 当前 Indexer 证书及原生证据 |
| `http://127.0.0.1:8080/api/result` | 空投资格结果 |
| `http://127.0.0.1:8082/api/result` | 治理资格结果 |
| 两个消费者的 `/api/history` | 各自实例级查询结果历史 |

```bash
./scripts/run-proof.sh \
  --token 0x5298000000000000000000000000000000000001 \
  --holder 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  --block 1000
```

该命令要求已有运行中的 demo 和委员会配置。
P0 只接受上述 fixture Token、非零 holder 和 fixture 区块 0..1000。
默认是公开的本地 Anvil 开发密钥；`HOLDERPROOF_DEV_KEY` 可以覆盖本地开发账户。

```bash
# 只检查现有默认演示结果：
python3 scripts/verify_step4.py
# 修改本地 demo 门槛，验证两个 Gate 的两种结果：
python3 scripts/verify_step4.py --exercise-gates
# 停止这套 demo：
./scripts/stop-step4.sh
```

## 4. 阅读、修改和验证代码

| 模块 | 修改时关注 |
|---|---|
| `holderproof-types` | 共享 wire 类型、字段顺序、查询和事实摘要的兼容性 |
| `lyquid-indexer` | 确定性重放、完整事实一致性、原生认证和证书存储 |
| `holderproof-gate-core` | 两个消费者必须共享的信任和资格规则 |
| `lyquid-consumer` | 空投资格与领取资格记录，沿用原目录名 |
| `lyquid-governance-gate` | 独立治理策略与每个提案的动作记录 |
| `scripts` / `evm` | 可复现入口、独立 EVM 标准答案与验收证据 |

保持函数职责单一：运行时上下文和状态写入放入口模块，纯校验与数据构造放独立模块。
变量应表达业务含义，例如 `agreed_fact`；不要把一致结果叫作“第一份结果”。
更改事件哈希字段、事实序列化、证书结构或共识逻辑时，要同步检查 Python、Rust 和文档。

```bash
cargo fmt --all -- --check
make check
make guest-test
# 改到公开接口、认证载荷或部署脚本时：
make acceptance
```

不要提交 `.run/`、编译产物、测试链状态或本机日志。
说明文档以 Markdown 为源；更新后重建 HTML、PDF 和 PNG，避免派生版本继续显示旧架构。

## 5. 信任边界与后续范围

消费者信任管理员配置的 Indexer 和 Lyquor 的认证写入路径。
原生签名字节被保留，但消费者不为任意 JSON 维护一套独立委员会密钥验证服务。
共享状态引用也不是状态包含证明。

当前 fixture 和区块哈希是合成数据。独立 EVM 对照验证余额和事件内容，
fixture N 映射到 EVM N+2；它不是实时 Sepolia 接入，也不验证真实网络重组和最终性。

P1 的 MIN_BALANCE、ACTIVE_RATIO 和窗口证书、Sepolia 适配、完整 E3、外部用户测试、
UI、最终报告和录制演示仍需另行推进。`claim()` 与 `vote()` 目前是资格动作记录，
验收主要覆盖资格判断和证书复用，不宣称完整支付或治理系统已验收。
