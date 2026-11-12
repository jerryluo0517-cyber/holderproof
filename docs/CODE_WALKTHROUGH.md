# HolderProof 代码导读

本文档面向第一次阅读 HolderProof 代码的成员。目标是在 20–30 分钟内理解：

```text
一次查询如何从用户输入
走到 3-of-4 委员会认证
再走到 Consumer 的 eligible 结果
```

当前本地与 GitHub `main` 已同步；Git 状态以仓库页面和 `git status` 为准。

## 1. 先认识四个概念

### Lyquid

Lyquid 是用 Rust 编写、部署到 Lyquor 上的应用。

### Network State

多个节点共享的状态，例如：

```text
query_holder
query_block
certificates
latest_query_hash
```

### Instance State

节点本地状态，例如 Consumer 的诊断计数。当前 Indexer 主要使用共享状态。

### Certified Call

多个委员会节点执行 `propose`，通过 `aggregate` 达成一致，再由
`submit_certified_call!` 提交认证后的 network call。

---

# 2. 文件职责

## 共享类型

```text
holderproof-types/src/lib.rs
```

包含：

- `TransferEvent`
- `QuerySpec`
- `Checkpoint`
- `QueryCertificate`
- `CertificateStatus`
- `CHAIN_ID`
- `PARSER_VERSION`
- `COMMITTEE_THRESHOLD`

Indexer 和 Consumer 共同使用这些类型，避免证书结构重复。

## Indexer

```text
lyquid-indexer/src/
├── lib.rs             # state、网络方法、委员会流程
├── fixture.rs         # LazyLock fixture、余额、TWAB、query hash
├── http_endpoints.rs  # HTTP 诊断端点
├── merkle.rs          # 事件叶子、Merkle root
├── models.rs          # BalanceProposal
└── tests.rs           # Indexer guest tests
```

### `lib.rs`

推荐按以下顺序阅读：

1. `state!`
2. `set_query`
3. `propose`
4. `aggregate`
5. `update_certified_balance`
6. `get_certificate_json`
7. `report_balance`

HTTP 方法在独立的 `http_endpoints.rs` 中。

### `fixture.rs`

负责所有与本地数据有关的纯逻辑：

- 读取嵌入的 `transfers.json`
- 使用 `LazyLock` 只解析和排序一次
- 按 holder 和 block 重算余额
- 计算 TWAB
- 计算 block hash
- 生成 checkpoint
- 生成 `QuerySpec` 和 query hash

### `merkle.rs`

负责：

- 事件规范化字符串
- SHA-256 event leaf
- `hash_hex` / `hash_hex_prefixed` 辅助函数
- mirrored-pair Merkle root

Rust 实现与 Python ground truth 使用完全相同的序列化规则。

### `models.rs`

只包含委员会提案：

```text
BalanceProposal { holder, block, balance }
```

## Consumer

```text
lyquid-consumer/src/lib.rs
```

推荐阅读顺序：

1. `state!`
2. `configure`
3. `validate_certificate`
4. `ensure_finalized`
5. `ensure_sufficient_signers`
6. `ensure_query_matches`
7. `ensure_metadata`
8. `ensure_checkpoint`
9. `parse_balance`
10. `check_eligible`
11. `http_result`

Consumer 不直接重算余额。它只做：

```text
call! Indexer
→ 获取 QueryCertificate JSON
→ 校验证书
→ 读取 balance
→ 判断 eligible
```

---

# 3. 一次查询的完整流程

```text
User / Script
    |
    | set_query(holder, block)
    v
Indexer network state: query_holder, query_block
    |
    | report_balance()
    v
propose() on each committee node
    |
    | fixture_balance(holder, block)
    v
aggregate()
    |
    | require 3 matching proposals
    v
update_certified_balance()
    |
    | create QueryCertificate
    v
network certificates[queryHash]
    |
    | get_certificate_json(holder, block)
    v
Consumer check_eligible()
    |
    | validate status, signers, holder, block,
    | blockHash, eventMerkleRoot, queryHash
    v
eligible = balance >= threshold
```

---

# 4. 状态说明

## Indexer

```text
initializer             部署者地址
balance_proof           oracle topic
query_holder            当前查询的 holder
query_block             当前查询的 block
certified_holder        最新认证 holder
certified_block         最新认证 block
certified_balance       最新认证余额
certified_signer_count  最新认证签名数
latest_query_hash       最新证书 ID
certificates            按 queryHash 保存的完整证书
```

## Consumer

```text
indexer                  Indexer Lyquid 地址
expected_holder          预期 holder
expected_block           预期 block
threshold                资格阈值
last_balance             最新认证余额
last_eligible            最新资格结果
last_query_hash          最新 query hash
last_block_hash          最新 block hash
last_event_merkle_root   最新事件 Merkle root
last_certificate_status  最新证书状态
last_signer_count        最新签名数量
```

---

# 5. Lyquid 方法类型

## `#[method::network]`

在共享 network state 上执行，通常对应链上或共识路径。

当前使用：

```text
constructor
set_query
publish_balance
get_balance
get_certified_balance
configure_committee
update_certified_balance
get_certificate_json
get_twab
```

## `#[method::instance]`

在单个节点实例上执行。

当前使用：

```text
propose
aggregate
report_balance
http_balance
http_certified
http_certificate
http_result
```

## `oracle::two_phase`

表示委员会两阶段流程：

```text
propose -> aggregate -> certified update
```

---

# 6. 证书字段从哪里来

| 字段 | 来源 |
|---|---|
| certificate_id | query hash |
| query_hash | chain/token/holder/block/parser version |
| chain_id | 固定 fixture chain ID |
| token | fixture token |
| holder | `set_query` |
| block_number | `set_query` |
| balance | fixture replay |
| checkpoint.block_hash | deterministic block hash |
| checkpoint.event_count | 过滤后的事件数 |
| checkpoint.event_merkle_root | `merkle.rs` |
| signer_ids | `ctx.cert.signers` |
| status | `Finalized` |
| created_at | 链上位置 |

---

# 7. 修改代码时的检查清单

## 修改证书字段

同时更新：

- `holderproof-types/src/lib.rs`
- Indexer 的 `update_certified_balance`
- Consumer 的 `validate_certificate`
- `docs/step4-certificate.md`
- Consumer 测试

## 修改余额算法

同时检查：

- `fixture.rs`
- Python `fixtures/generate_fixture.py`
- Rust tests
- Python expected results

## 修改委员会流程

同时检查：

- `configure_committee`
- `propose`
- `aggregate`
- `report_balance`
- `scripts/reproduce-step4.sh`

---

# 8. 本地验证

```bash
make check
shaker test lyquid-indexer/Cargo.toml
shaker test lyquid-consumer/Cargo.toml
./scripts/reproduce-step4.sh
./scripts/stop-step4.sh
```

预期：

```text
Indexer tests:      5 passed
Consumer tests:     4 passed
Python tests:       6 passed
Fixture checks:     9 passed
Certificate status: Finalized
Signer count:       3
eligible:           true
```

也可以直接运行 Step 4 一键复现：

```bash
./scripts/reproduce-step4.sh
./scripts/stop-step4.sh
```

当前工具链由 `rust-toolchain.toml` 固定为 Rust 1.99.0。`LazyLock` 的最低稳定版本要求是 Rust 1.80，因此项目环境应至少使用 Rust 1.80，推荐直接使用项目的 1.99.0。
