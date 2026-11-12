# HolderProof 项目推进副本

更新时间：2026-11-12  
当前阶段：Step 4 完成，准备进入 Step 5  
当前分支：main  
本地状态：有未提交的重构改动  
GitHub current main：b6451eb Add one-command Step 4 reproduction

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
lyquid-consumer/src/lib.rs         Consumer 证书校验和资格判断
docs/CODE_WALKTHROUGH.md           代码导读
scripts/reproduce-step4.sh         一键复现
scripts/stop-step4.sh              停止复现环境
```

## 6. 本地与 GitHub 状态

本地有未提交改动：

- 共享类型 crate
- Indexer 模块拆分
- HTTP endpoints 拆分
- Consumer 证书校验拆分
- 代码导读
- 新增测试

这些改动尚未提交，GitHub 远程 `main` 仍是：

```text
b6451eb Add one-command Step 4 reproduction
```

因此当前 GitHub 版本不包含最新本地可读性重构。

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
