# HolderProof 当前进度

更新时间：2026-11-15。当前实现：执行方案 03/04 的本地 fixture P0，
包含三成员 2-of-3 完整事实证书、独立空投/治理消费者，以及历史 EVM 对照。

完整验收范围、原生 LDK 适配和剩余限制见
[03/04 验收记录](../reports/PLAN_03_04_ACCEPTANCE.md)。

本次补齐以 `12f622e` 为远端基线；最新提交与同步状态以 Git 记录为准。

验证：`make acceptance` 已通过；Rust guest 测试 24 项、Python 测试 6 项、fixture 检查 9 项、
308 条 EVM 日志和 18 次历史余额查询一致。

运行：`make acceptance`。原有内部 Step 编号与执行方案章节号不同。
下方是历史进度快照；其中的“尚未实现”、测试数量和 Git 状态按各自日期理解。

---

# HolderProof 项目推进副本

更新时间：2026-11-12
当前阶段：Step 4 完成，准备进入 Step 5
当前分支：main
本地状态：工作区干净
GitHub 状态：main 与 origin/main 同步（2026-11-12）

## 2026-11-15：Step 4 加固

本次改动尚未提交，下面原有的 Git 同步状态仅为 2026-11-12 记录。

- Consumer 配置仅部署者可修改，拒绝零地址，重新配置会清空旧结果。
- 四节点委员会要求成员唯一；聚合支持第一份提案异常时其余三份达成一致。
- 重复节点不参与投票；认证写入要求至少三名不同签名者。
- 查询哈希算法移至共享类型 crate，Consumer 重算并校验证书 ID 和查询字段。
- 限定 fixture token、chain 和 parser，校验 checkpoint 范围及哈希格式。
- Rust guest 测试：Indexer 10 项，Consumer 10 项；Python 6 项，fixture 检查 9 项。
- 新增 `scripts/verify_step4.py`，一键演示会自动比对在线证书与独立 fixture 答案。
- 仍使用本地 fixture；JSON 证书的认证依赖配置的 Indexer 与 Lyquor 运行时。
- 本次只更新 Markdown 文档，已有 PDF 是历史版本。

## 1. 项目定位

HolderProof 是一个基于 Lyquor 的可验证 ERC-20 历史持仓证明：

```text
ERC-20 Transfer fixture
→ Rust 历史余额重算
→ 4 节点委员会
→ 3-of-4 认证
→ QueryCertificate
→ Consumer call!
→ eligible / not eligible
```

当前数据源是本地确定性 fixture，不是 Sepolia 实时数据。Lyquor devnet、多节点委员会和跨 Lyquid 调用是真实本地执行。

## 2. 已完成阶段

| 阶段 | 内容 | 状态 |
|---|---|---|
| Step 1 | Rust/LDK/Foundry/devnet/Hello Lyquid | 完成 |
| Step 2 | 4 节点委员会与跨 Lyquid call! | 完成 |
| Step 3 | ERC-20 fixture 重算与 TWAB | 完成 |
| Step 4 | 完整 QueryCertificate、Merkle root、Consumer 校验 | 完成 |

## 3. Step 4 技术成果

- 共享类型 crate：`holderproof-types`
- TransferEvent 全字段模型
- QuerySpec、Checkpoint、QueryCertificate
- SHA-256 event leaf
- 与 Python 一致的 Merkle root
- queryHash
- deterministic blockHash
- 3 个 signer ID
- Finalized 状态
- certificate 按 queryHash 持久化
- Consumer 读取并校验完整证书
- 一键复现脚本

## 4. 当前验证结果

```text
cargo check --workspace: PASS
Indexer guest tests:     5 passed
Consumer guest tests:    4 passed
Python tests:            6 passed
Fixture checks:          9 passed
```

最近一次一键复现结果：

```json
{
  "balance": "62685",
  "eligible": true,
  "certificateStatus": "Finalized",
  "signerCount": 3,
  "eventMerkleRoot": "2e89babbc7b45637458bd08a2d843087354bf175eb559e5c1ddc7fcc0e79617b"
}
```

## 5. 当前代码结构

```text
holderproof-types/src/lib.rs       共享 wire types
lyquid-indexer/src/lib.rs          Indexer state 和方法
lyquid-indexer/src/fixture.rs      fixture、余额、TWAB、query hash
lyquid-indexer/src/merkle.rs       事件哈希和 Merkle root
lyquid-indexer/src/http_endpoints.rs HTTP 诊断接口
lyquid-indexer/src/models.rs       委员会 proposal
lyquid-indexer/src/tests.rs        Indexer guest tests
lyquid-consumer/src/lib.rs         Consumer state 和入口
lyquid-consumer/src/certificate.rs Consumer 证书校验
lyquid-consumer/src/eligibility.rs Consumer 资格计算
lyquid-consumer/src/http_endpoints.rs Consumer HTTP 接口
lyquid-consumer/src/tests.rs       Consumer guest tests
docs/guides/CODE_WALKTHROUGH.md           代码导读
scripts/reproduce-step4.sh         一键复现
scripts/stop-step4.sh              停止复现环境
```

## 6. 本地与 GitHub 状态

本地与 GitHub 已同步：

```text
本地 HEAD：与 GitHub main 一致
GitHub main：与本地 HEAD 一致
工作区：clean
未提交改动：0
未推送提交：0
```

具体最新 commit 以 GitHub 仓库页面为准。最新可读性重构已经包含在远程 `main`：

- 共享类型 crate
- Indexer 模块拆分
- HTTP endpoints 拆分
- Consumer 证书校验拆分
- 代码导读
- 新增测试

## 7. 尚未完成

- AirdropGateLyquid
- GovernanceGateLyquid
- Sepolia 真实数据适配器
- 完整 E3 篡改与异常测试
- 证书挑战/撤销机制
- UI
- 用户外测
- 正式 6–10 页报告
- 预录 Demo
- 最终提交包

## 8. 下一步建议

### Step 5A：AirdropGateLyquid

状态：

```text
campaign_id
token
snapshot_block
threshold
claim_amount
claimed[address]
```

流程：

```text
QueryCertificate
→ check eligible
→ claim()
→ mark claimed
→ transfer test token
```

### Step 5B：GovernanceGateLyquid

状态：

```text
proposal_id
snapshot_block
minimum_balance
has_voted[address]
```

流程：

```text
QueryCertificate
→ check snapshot balance
→ vote()
→ prevent duplicate voting
```

### Step 6：E3

至少覆盖：

- 篡改 Transfer 事件
- 错误 blockHash
- 伪造 Merkle root
- 少于 3 个 signer
- 证书重放
- holder/block 不匹配
- stale certificate

## 9. 目前最准确的状态描述

> HolderProof 的 P0 可验证持仓证明闭环和完整证书已经完成；项目还没有完成空投领取、治理投票、真实 Sepolia 数据和最终交付材料。

## 10. 恢复工作命令

```bash
cd holderproof
make check
shaker test lyquid-indexer/Cargo.toml
shaker test lyquid-consumer/Cargo.toml
./scripts/reproduce-step4.sh
./scripts/stop-step4.sh
```
