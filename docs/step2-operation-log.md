# HolderProof Step 2/3 操作记录与后续步骤

日期：2026-11-11  
状态：Step 1–3 已完成；Step 4 待开始

## 1. 当前成果

已经跑通：

```text
4 节点 devnet
→ 4 节点委员会配置
→ 3-of-4 认证
→ certified balance 写入共享状态
→ Consumer 通过 call! 读取
→ eligible=true
```

## 2. 当前环境

| 项目 | 版本/状态 |
|---|---|
| Rust | 1.99.0 |
| WASM target | wasm32-unknown-unknown |
| Foundry | 1.7.1 |
| Lyquor LDK | 0.8.4 |
| shaker | 0.8.4+54c5e11 |

本地 devnet：

```text
Node 1: http://127.0.0.1:10087
Node 2: http://127.0.0.1:11087
Node 3: http://127.0.0.1:12087
Node 4: http://127.0.0.1:13087
```

## 3. 当前 Deployment

| 角色 | Lyquid ID | Backend Contract |
|---|---|---|
| Indexer | Lyquid-ahjg4huwvban3bvhxue7rcbaw4mnltwyolhqa | 0x1613beB3B2C4f22Ee086B2b38C1476A3cE7f78E8 |
| Consumer | Lyquid-wsd6v4xjliqgjrcaiikuafrjmaho4q4hqiwqa | 0x84eA74d481Ee0A5332c457a4d796187F6Ba67fEB |

当前诊断地址：

```text
Consumer: http://127.0.0.1:8080/api/result
Indexer:  http://127.0.0.1:8081/api/certified
```

## 4. 操作记录

### 4.1 启动 4 节点 devnet

```bash
cd holderproof
./scripts/start-devnet.sh multi
```

### 4.2 部署 Indexer

```bash
shaker deploy \
  --endpoint ws://127.0.0.1:10087/ws \
  --reference http://127.0.0.1:10087/lyquids:holderproof-indexer-local \
  lyquid-indexer/Cargo.toml
```

### 4.3 部署 Consumer

```bash
shaker deploy \
  --endpoint ws://127.0.0.1:10087/ws \
  --reference http://127.0.0.1:10087/lyquids:holderproof-consumer-local \
  lyquid-consumer/Cargo.toml
```

### 4.4 配置 4 节点委员会

NodeID 被转换为 bytes32 后，调用：

```bash
cast send <INDEXER_CONTRACT> 'configure_committee(bytes32[])' \
  '[<NODE1>,<NODE2>,<NODE3>,<NODE4>]' \
  --private-key <ANVIL_KEY> \
  --rpc-url ws://127.0.0.1:10087/ws
```

委员会阈值为：

```text
4 / 2 + 1 = 3
```

### 4.5 激活并 finalize oracle epoch

```bash
cast call --rpc-url ws://127.0.0.1:10087/ws \
  <INDEXER_CONTRACT> \
  '__lyquor_oracle_advance_epoch(string,address,bool) returns (bool)' \
  balance_proof <INDEXER_LYQUID_HEX> false

cast call --rpc-url ws://127.0.0.1:10087/ws \
  <INDEXER_CONTRACT> \
  '__lyquor_oracle_finalize_epoch(string,address,bool) returns (bool)' \
  balance_proof <INDEXER_LYQUID_HEX> false
```

### 4.6 生成认证余额

```bash
cast call --rpc-url ws://127.0.0.1:10087/ws \
  <INDEXER_CONTRACT> \
  'report_balance() returns (bool)'
```

结果：

```json
{
  "holder": "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa",
  "block": 1000,
  "balance": "62685",
  "signerCount": 3
}
```

### 4.7 配置 Consumer

```bash
cast send <CONSUMER_CONTRACT> 'configure(address,address,uint64,uint256)' \
  <INDEXER_LYQUID_HEX> \
  0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  1000 \
  60000 \
  --private-key <ANVIL_KEY> \
  --rpc-url http://127.0.0.1:8545
```

### 4.8 执行资格检查

```bash
cast send <CONSUMER_CONTRACT> 'check_eligible()' \
  --private-key <ANVIL_KEY> \
  --rpc-url http://127.0.0.1:8545
```

### 4.9 查看结果

```bash
curl http://127.0.0.1:8080/api/result
```

结果：

```json
{
  "balance": "62685",
  "block": 1000,
  "checkCount": 1,
  "eligible": true,
  "holder": "0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa",
  "indexer": "0x01D26E1e96A840dD86A7BD09F88820b718D5CEd8",
  "threshold": "60000"
}
```

## 5. 当前代码职责

### Indexer

- `configure_committee`：配置委员会，阈值为 `n/2+1`
- `propose`：当前返回固定测试余额 62685
- `aggregate`：检查至少 3 个提案一致
- `update_certified_balance`：写入认证结果和 signer count
- `report_balance`：触发认证流程
- `get_certified_balance`：供 Consumer 通过 `call!` 读取
- `/api/certified`：HTTP 诊断

### Consumer

- `configure`：绑定 Indexer、holder、block、threshold
- `check_eligible`：调用 `get_certified_balance` 并写入资格结果
- `/api/result`：读取最新的认证余额和资格结果

## 6. 已验证的内容

```text
cargo check --workspace: PASS
Python unit tests: 6 passed
Fixture checks: 9 passed
Committee signer count: 3
Consumer result: eligible=true
```

## 7. 已知限制

1. `propose` 当前仍使用固定测试余额 62685，尚未读取 `transfers.json`。
2. 本地 devnet 是临时状态；重启后 deployment、委员会和认证状态都会重置。
3. network 写入与 oracle epoch activation 使用了不同的本地 RPC 路径，需要按本文命令执行。
4. 当前只有认证余额，还没有完整的 queryHash、blockHash、Merkle root 和 signer list 证书结构。
5. 尚未实现 AirdropGate / GovernanceGate 两个正式消费合约。
6. 尚未实现缓存、历史查询、reorg 处理和 UI。

## 8. 后续步骤

### Step 3（已完成）：接入真实 Transfer fixture

目标：

```text
读取 transfers.json
→ 按 holder + block 重算余额
→ 用真实余额替换 propose 中的常量
```

需要修改：

- `lyquid-indexer/src/lib.rs`
- 新增富余索引数据加载或 Rust fixture 解析
- `fixtures/transfers.json`
- `docs/step2-certified-call.md`

完成标准：

```text
Rust 重算的 62685 与 Python ground truth 一致
委员会认证结果与 fixture 标准答案一致
```

### Step 4：完整证书

加入：

- query hash
- block hash
- event Merkle root
- parser version
- signer list
- certificate status

需要新增/修改：

- `lyquid-indexer/src/lib.rs`
- `lyquid-consumer/src/lib.rs`
- `tests/`
- `docs/architecture.md`

### Step 5：正式消费方

实现：

- `AirdropGateLyquid`
- `GovernanceGateLyquid`

需要新增：

- `lyquid-airdrop-gate/`
- `lyquid-governance-gate/`
- `tests/e2e/`

### Step 6：E3 自审计

测试：

- 篡改一条 Transfer
- 只有一个节点返回错误结果
- 错误 block hash
- 不足 3 个签名
- 重复证书
- 不存在的 holder

### Step 7：最小 UI

页面：

- 输入 holder / block / threshold
- 查看认证余额
- 查看 signer count
- 查看 eligible
- 展示委员会和证书状态

### Step 8：交付材料

- 3 分钟预录 Demo
- 6–10 页报告
- GitHub 公开仓库
- 用户测试记录
- AI 使用声明
- 提交 ZIP 和 PDF

## 9. 建议时间安排

| 日期 | 目标 |
|---|---|
| 11-12 | 真实 fixture replay，替换固定测试余额 |
| 11-13 | 完整证书字段与校验 |
| 11-14 | AirdropGate / GovernanceGate |
| 11-15 | E3 异常和篡改测试 |
| 11-16 | 最小 UI 与用户测试 |
| 11-17 | 报告主体与图表 |
| 11-18 | Demo 录制、README、复现脚本 |
| 11-19 上午 | 最终检查并提交 |

## 10. 操作注意事项

- 当前 devnet 和两个 `shaker serve` 仍在运行，便于查看结果。
- 如果要停止：在对应终端按 `Ctrl-C`。
- localnet 重启后，Lyquid ID 和 contract address 可能变化，必须重新部署并记录新地址。
- 不要直接沿用旧 contract address 调用新 devnet。
- 后续每次改完 Rust 代码，先运行 `make check`，再部署。

## 11. Step 3 完成记录

### 实现

- Indexer 通过 include_str! 嵌入 fixtures/transfers.json
- Rust 按 block_number / log_index 排序事件
- Rust 重算 holder 在指定区块的余额
- 新增 set_query(holder, block)
- 委员会 propose 使用真实重算结果

### 测试

- fixture 事件数：308
- Alice @ 900：47685
- Alice @ 950：57685
- Alice @ 1000：62685
- Bob @ 1000：51886
- Rust guest tests：3 passed

### Live 认证结果

Alice：

```json
{"holder":"0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa","block":1000,"balance":"62685","signerCount":3}
```

Bob：

```json
{"holder":"0xbBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB","block":1000,"balance":"51886","signerCount":3}
```

Consumer 最终结果：

```json
{"balance":"62685","block":1000,"checkCount":3,"eligible":true,"holder":"0xaAaAaAaaAaAaAaaAaAAAAAAAAaaaAaAaAaaAaaAa","indexer":"0x01D26E1e96A840dD86A7BD09F88820b718D5CEd8","threshold":"60000"}
```

Step 3 已证明委员会不是返回固定值，而是根据 holder 和 block 重放真实 fixture。
