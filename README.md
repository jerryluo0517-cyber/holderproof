# HolderProof & VeriCredit-Idx

A **3-of-4 Byzantine Fault Tolerant (BFT) Verifiable Historical On-Chain Indexer and Multi-Gate System** built on **Lyquor Network (LDK 0.8.4)** and **Rust (Edition 2024)**.

Four independent committee nodes deterministically replay historical on-chain events and reach **$N = 3f + 1 = 4$, $2f + 1 = 3$** consensus via Lyquor's native Two-Phase Oracle protocol, certifying complete query facts and SHA-256 Merkle checkpoints into shared Network State. Independently deployed downstream consumer Lyquids (Airdrop Gate, Governance Gate, and VeriCredit Dynamic Fee / Lending LTV Gate) atomically read and verify the same persisted certificate within a single cross-Lyquid `call!`, delivering verifiable historical computation without off-chain ZK proving latency.

---

## 1. Overall System Architecture

The architecture enforces a **Three-Layer State Boundary** (node-local private `Instance State` → periodic global `Epoch Checkpoints` in `Network State` → on-demand certified user cache in `Network State`) with dual-track support for both deterministic local fixtures and authentic Ethereum Sepolia RPC logs:

```mermaid
flowchart TB
    subgraph DataSources["Dual-Track Data Sources (Deterministic Local + Real Ethereum Sepolia)"]
        DS1["Local ERC-20 Fixture<br/>308 Transfer Events (Blocks 0..1000)"]
        DS2["VeriCredit Behavior Fixture<br/>40 Swap / Repay / Liquidation Events"]
        DS3["Real Sepolia Public RPC Fetcher<br/>169 Authentic WETH Transfers (Blocks 12184050..12184649)"]
    end

    subgraph Committee["Lyquor 4-Node Localnet · 3-of-4 BFT Committee (N=3f+1, 2f+1=3)"]
        direction LR
        N1["Node 1 :10087<br/>Instance Replay"]
        N2["Node 2 :11087<br/>Instance Replay"]
        N3["Node 3 :12087<br/>Instance Replay"]
        N4["Node 4 :13087<br/>Instance Replay"]
    end

    subgraph Indexer["lyquid-indexer (:8081) · Four Two-Phase Oracle Channels"]
        O1["balance_proof<br/>Historical Balance Certs"]
        O2["credit_proof<br/>VeriCredit Anti-Wash Certs"]
        O3["checkpoint_proof<br/>Periodic Global Epoch Checkpoints"]
        O4["sepolia_proof<br/>Real Sepolia WETH Certs"]
        NS[("Network State Persistence<br/>certificates / credit_certificates<br/>epoch_checkpoints / max_dispersion_bps")]
        O1 & O2 & O3 & O4 --> NS
    end

    subgraph Core["holderproof-gate-core (Shared Security & Verification Engine)"]
        V1["6-Layer Certificate Verification + OracleCert Provenance<br/>Audit-05 TTL Staleness Guard + Dynamic Fee/LTV Tiering"]
    end

    subgraph Consumers["Independent Downstream Gate Lyquids (Atomic Cross-Lyquid call!)"]
        C1["lyquid-consumer :8080<br/>AirdropGateLyquid<br/>Threshold 60,000 · One-Time claim()"]
        C2["lyquid-governance-gate :8082<br/>GovernanceGateLyquid<br/>Threshold 70,000 · Per-Proposal vote()"]
        C3["lyquid-credit-gate :8083<br/>VeriCreditGateLyquid<br/>Dynamic Fee 5/15/30 bps · LTV 85/75/60%<br/>quote_or_fallback() Fail-Safe"]
    end

    UI["Interactive Web Console<br/>ui/holderproof_demo.html (4 Tabs + Local JSON Import)"]

    DS1 & DS2 & DS3 --> Committee
    Committee -->|"3-of-4 Native Quorum Signatures (OracleCert)"| Indexer
    NS -->|"Atomic Cross-Lyquid call!"| Core
    Core --> C1 & C2 & C3
    Indexer & C1 & C2 & C3 -.->|"HTTP JSON + CORS"| UI
```

---

## 2. Core Features Implemented

| Module / Capability | Key Engineering Highlights | Code Location |
|---|---|---|
| **Standard BFT 3-of-4 Committee** | All 4 localnet nodes participate in the active committee ($N=4, f=1, 2f+1=3$); duplicate submitters are excluded before voting, and quorum requires byte-identical complete facts | `holderproof-types/src/lib.rs`, `lyquid-indexer/src/models.rs` |
| **HolderProof Historical Balance & Dual Gates** | Deterministically replays 308 `Transfer` events into a SHA-256 Merkle-anchored `QueryCertificate`; Airdrop Gate (threshold `60,000` → `true`) and Governance Gate (threshold `70,000` → `false`) reuse the same `queryHash`; supports both locked single-user mode and multi-user `configure_policy` / `check_eligible_for`, one-time `claim()` / `vote()`, and `invalidate_certificate` revocation | `lyquid-indexer/`, `holderproof-gate-core/`, `lyquid-consumer/`, `lyquid-governance-gate/` |
| **Periodic Global Epoch Checkpoints (Layer 2)** | Dedicated `checkpoint_proof` oracle commits 3-of-4 certified global summaries every `250` blocks (`250 / 500 / 750 / 1000`), persisting cumulative transfer volume, unique holder count, event count, and Merkle root for zero-recomputation downstream reads | `lyquid-indexer/src/fixture.rs`, `scripts/verify_epoch_checkpoints.py` |
| **VeriCredit-Idx Anti-Wash Credit & Dynamic Pricing** | Automatically strips wash-trading volume (same counterparty, opposite direction, $\Delta\text{block} \le \text{wash\_window}$); computes deterministic integer credit scores $S(u) \in [0, 1000]$ via `u128::ilog2`; supports default (`35/20/2/3`) and `initializer`-configurable weights; `lyquid-credit-gate` synchronously quotes dynamic DEX fees (`5/15/30 bps`) and lending LTV (`85%/75%/60%`) with `quote_or_fallback()` fail-safe on missing, revoked, or stale (Audit-05 TTL) certificates | `lyquid-indexer/src/credit.rs`, `holderproof-gate-core/src/credit.rs`, `lyquid-credit-gate/`, `scripts/verify_credit.py` |
| **Dual-Track Real Ethereum Sepolia Verification** | `fetch_sepolia_logs.py` paginates authentic Sepolia WETH `Transfer` logs (`169` events across blocks `12184050..12184649`) and real block hashes; `sepolia_proof` certifies authentic chain balances (`0.002 WETH`) via 3-of-4 quorum; `holderproof-gate-core` cleanly dispatches validation by `dataset_id` without altering offline tests | `scripts/fetch_sepolia_logs.py`, `lyquid-indexer/src/sepolia.rs`, `holderproof-gate-core/src/certificate.rs`, `scripts/verify_sepolia.py` |
| **Dual-Mechanism Security (NDSS 2026 Defense)** | Historical queries enforce deterministic `ChainPos` binding + byte-level consensus; time-skewed observation queries enforce a Network State honest-core dispersion guard $(v_{m-f} - v_{f+1}) \times 10000 \le M \times \epsilon_{\max}$ (default `100 bps` = 1%) to defeat Byzantine outliers and leader cherry-picking | `lyquid-indexer/src/dispersion.rs` |
| **Archival EVM Cross-Check & Web Console** | `FixtureToken.sol` replays all 308 transfers on an isolated Anvil (Chain ID `31338`) to cross-verify 18 archival `eth_call balanceOf` snapshots; all HTTP endpoints return JSON + CORS headers, paired with a standalone 4-tab English web console (`ui/holderproof_demo.html`) | `evm/`, `scripts/verify_historical_balance.py`, `ui/holderproof_demo.html` |

---

## 3. Project & Documentation Structure

| Path | Type | Responsibility |
|---|---|---|
| `holderproof-types/` | Rust `rlib` | Runtime-independent wire types & canonical SHA-256 hashing (`QuerySpec`, `BalanceFact`, `QueryCertificate`, `CreditParams`, `CreditCertificate`, `EpochCheckpoint`) |
| `holderproof-gate-core/` | Rust `rlib` | Shared consumer verification engine: atomic `call!` wrappers, dual-track 6-layer certificate validation, `OracleCert` provenance, VeriCredit tiering, and Audit-05 `is_stale` TTL checks (**15** Guest unit tests) |
| `lyquid-indexer/` | Rust `cdylib + rlib` | Core indexer: `fixture.rs`, `merkle.rs`, `models.rs`, `credit.rs`, `sepolia.rs`, `dispersion.rs`, `certificate_builder.rs`, `http_endpoints.rs`, and 4 oracle channels (**21** Guest unit tests) |
| `lyquid-consumer/` | Rust `cdylib + rlib` | `AirdropGateLyquid` (port `8080`): airdrop threshold gate, multi-user `configure_policy`/`check_eligible_for`, one-time `claim()`, and 32-entry instance history |
| `lyquid-governance-gate/` | Rust `cdylib + rlib` | `GovernanceGateLyquid` (port `8082`): independent governance gate, multi-user mode, per-proposal `vote()` anti-replay, and instance history |
| `lyquid-credit-gate/` | Rust `cdylib + rlib` | `VeriCreditGateLyquid` (port `8083`): same-call dynamic DEX fee & lending LTV gate with `quote_or_fallback()` and configurable `max_staleness` TTL |
| `fixtures/` | Python + JSON | Deterministic balance fixture (`transfers.json`), VeriCredit behavior fixture (`credit_events.json`), real Sepolia WETH logs (`sepolia_transfers.json`), and ground-truth expectations |
| `evm/` | Foundry (Solidity 0.8.28) | `FixtureToken.sol` test ledger for archival `balanceOf` verification |
| `scripts/` | Shell + Python + JS | Environment setup, 4-node devnet lifecycle, P0/E3/Epoch/VeriCredit/Sepolia verification suites, Sepolia RPC fetcher, and doc exporters |
| `tests/` | Python `unittest` | `test_ground_truth.py` (**6** baseline tests for determinism, block boundaries, Merkle proofs, and tamper detection) |
| `ui/` | HTML + JS | Zero-dependency standalone interactive English console `holderproof_demo.html` (4 tabs + local evidence JSON import) |

---

## 4. Core Workflow Diagrams

### 4.1 3-of-4 Two-Phase Oracle Certification & Cross-Lyquid Atomic Consumption

```mermaid
sequenceDiagram
    autonumber
    actor User as Caller / CLI Script
    participant Indexer as lyquid-indexer (:8081)
    participant Nodes as 4-Node Committee (3-of-4)
    participant Seq as Lyquor Sequencer (Anvil :8545)
    participant Gate as Downstream Gate (:8080 / :8082 / :8083)

    User->>Indexer: set_query / set_credit_query / set_sepolia_query
    User->>Indexer: report_balance() / report_credit() / report_sepolia_balance()
    Indexer->>Nodes: Phase 1 (propose): 4 nodes independently replay logs & compute Fact + Merkle Root
    Nodes-->>Indexer: Return 4 deterministic Fact proposals
    Indexer->>Indexer: Phase 2 (aggregate): Drop duplicate submitters; require >= 3 byte-identical votes
    Indexer->>Nodes: Collect >= 3 native 64-byte signatures over CertifiedCallParams
    Indexer->>Seq: submit_certified_call! (with native OracleCert)
    Seq->>Indexer: Verify signatures & sequence update_certified_* (defense-in-depth replay + idempotency)
    User->>Gate: check_eligible() / claim() / vote() / quote_or_fallback()
    Gate->>Indexer: Atomic cross-Lyquid call! to fetch persisted certificate JSON
    Gate->>Gate: 6-layer integrity check + OracleCert provenance + TTL/threshold evaluation
    Gate-->>User: Return eligibility / dynamic fee / LTV; persist anti-replay & instance history
```

### 4.2 VeriCredit Anti-Wash Scoring & Audit-05 TTL Fail-Safe Flow

```mermaid
flowchart LR
    A["Window Behavior Logs<br/>[B_start, B_end]"] --> B["Anti-Wash Filter<br/>Same Counterparty + Opposite Side<br/>Δblock <= wash_window -> Canceled"]
    B --> C["Pure Integer Score<br/>log_term = ilog2(1 + V_u)<br/>S(u) = min(1000, floor((w1*log + w2*repay)/(1 + alpha*liq)))"]
    C --> D["3-of-4 Certified Persistence<br/>CreditCertificate (with created_at)"]
    D --> E{"quote_or_fallback()<br/>Valid & Finalized Cert?"}
    E -- No --> F["Fail-Safe Conservative Tier<br/>Base: 30 bps / 60% LTV<br/>fallback = true"]
    E -- Yes --> G{"Audit-05 TTL Check<br/>now_block <= created_at + max_staleness?"}
    G -- No (Stale) --> F
    G -- Yes (Fresh) --> H["Dynamic Pricing Quote<br/>VIP (>=800): 5 bps / 85% LTV<br/>Standard (>=500): 15 bps / 75% LTV<br/>Base (<500): 30 bps / 60% LTV"]
```

---

## 5. Security Self-Audit & Robustness (Bonus E3)

### 5.1 Structured Security Self-Audit Matrix (Audit-01 to Audit-05)

| Audit ID | Threat Vector | Implemented Safeguard | Verification Evidence | Remaining Limitations |
|---|---|---|---|---|
| **Audit-01** | Cross-Batch & Action Replay Attack | Native `OracleCert` binds `lyquid_id`, `seq_id`, `epoch`, and `nonce`; immutable `certificates[query_hash]` rejects conflicting overwrites; consumer `claimed[caller]` and `voted["proposal\|caller"]` prevent action replay | `scripts/verify_e3.py`: duplicate `claim()`/`vote()` calls strictly keep `committed_actions = 1` | Relies on Lyquor sequencer runtime enforcing one-time `OracleCert.nonce` consumption |
| **Audit-02** | Sybil Duplicate Signer Forgery | `agreeing_proposal` excludes any node submitting more than once; `ctx.signer_node_id` enforces active-committee membership; strict deduplication in `certificate.rs`, `credit.rs`, and `dispersion.rs` | Guest tests: `duplicate_nodes_and_split_votes_cannot_form_quorum`, `duplicate_or_empty_signers_are_rejected` | Assumes committee enrollment via `configure_committee` is protected by `initializer` governance |
| **Audit-03** | Integer Overflow & Empty Window Div-by-Zero | Zero floating-point math; all `U256`/`u128` calculations use `checked_add`/`checked_mul`/`checked_sub`; empty ranges (`end <= start`) and zero denominators (`median == 0`) return explicit errors | Guest tests: `eligibility_handles_boundary_and_invalid_balance`, `twab_respects_half_open_window_and_large_gaps` | Callers must handle explicit error returns on extreme inputs |
| **Audit-04** | Byzantine Outlier & Leader Cherry-Picking (NDSS 2026) | Deterministic historical queries bind `ChainPos` for byte-level equality; time-skewed observations enforce Network State honest-core dispersion $(v_{m-f} - v_{f+1}) \times 10000 \le M \times \epsilon_{\max}$ (default `100 bps`) | Guest tests: `dispersion_guard_*` reproducing `$3,702.42` vs `$4,302.42` (single outlier filtered; 7.69% spread rejected) | Requires standard BFT honest majority ($f < n/3$) |
| **Audit-05** | Stale Historical Report Exploitation | `holderproof-gate-core/src/credit.rs` enforces `is_stale(now, created_at, max_staleness)`; `quote_or_fallback()` automatically reverts expired reports to conservative `Base` parameters (`30 bps / 60% LTV`) | Guest test: `credit_report_staleness_ttl_is_enforced` (validates exact `250` vs `251` TTL boundary and `u64::MAX` saturation) | Sudden defaults occurring within the active TTL window still require fresh re-certification |

### 5.2 Live Fault-Tolerance, Archival EVM Cross-Check & L1 Gas Efficiency

* **3-of-4 Process-Level Fault Tolerance (`scripts/verify_committee_availability.py`)**:
  * **1 member paused (`SIGSTOP` on port `12087`)**: remaining 3 active committee members certified a `Finalized` balance proof in **0.78s** and preserved the certificate identically after `SIGCONT` recovery.
  * **2 members paused (`SIGSTOP` on ports `11087, 12087`)**: only 2 active members remained (below 3-of-4 quorum); `report_balance()` timed out at 25s (**30.2s** total observation) with **zero unauthorized certificates issued**, then automatically finalized upon recovery.
* **Independent Archival EVM Verification (`scripts/verify_historical_balance.py`)**:
  * Replayed all **308** `Transfer` events into `FixtureToken.sol` on an isolated Anvil chain (Chain ID `31338`) and verified **18** archival `eth_call balanceOf` queries against both Python ground truth and Rust certificates with 100% agreement.
* **Measured L1 Sequencer Gas Efficiency**:
  * Verified via `cast receipt`: a single `claim()` transaction on the L1 sequencer consumes only **`34,218 gas`** (~`13,218 gas` above Ethereum's `21,000` base transfer cost), as all heavy historical replay, Merkle verification, and 6-layer certificate validation execute off-L1 inside Lyquor's LVM.

---

## 6. Trust Model & Three-Layer State Boundary

| Layer | Storage Tier | Stored Data | Rationale |
|---|---|---|---|
| **Layer 1: Node-Private** | `instance` state / embedded fixtures | Raw `transfers.json`, `credit_events.json`, `sepolia_transfers.json`, `proposal_cache`, bounded `result_history` (max 32) | Avoids replicating bulky raw logs across consensus state |
| **Layer 2: Global Committed** | `network` state | 3-of-4 committee config, VeriCredit weights, `max_dispersion_bps`, periodic `epoch_checkpoints` (every 250 blocks) | Shared parameters and global summaries readable with zero recomputation |
| **Layer 3: On-Demand Cache** | `network` state | `certificates[query_hash]`, `credit_certificates[query_hash]`, `sepolia_certificates`, `claimed`/`voted` anti-replay maps | Certifies long-tail users only on demand while satisfying LDK 0.8.4's network-only `call!` atomicity rule |

**Trust Anchor**: Consumers and the Indexer operate within the same Lyquor network. Consumers anchor trust in the configured Indexer `LyquidID` plus Lyquor's native runtime `OracleCert` enforcement, verifying 6-layer structural consistency and `OracleCert` provenance (`target`, `seq_id`, `network_state_id`) without maintaining a separate detached public-key registry.

---

## 7. Quickstart, Verification Commands & HTTP APIs

### 7.1 Prerequisites & Commands

Verified with Rust `1.99.0` (`wasm32-unknown-unknown`), Lyquor LDK `0.8.4`, Foundry `1.7.1`, and Python `3.10+`.

| Command | Purpose |
|---|---|
| `./scripts/setup-env.sh --check` | Inspect local Rust / LDK / Foundry / Python toolchain |
| `make check && make guest-test` | Fast local check: Python ground truth (6 tests) + workspace compile + Rust Guest tests (**21 + 15 = 36 tests**) |
| `./scripts/stop-step4.sh && make e3` | Full 3-of-4 P0 acceptance + archival EVM cross-check + E3 action/revocation/SIGSTOP audit |
| `python3 scripts/verify_epoch_checkpoints.py` | Activate `checkpoint_proof` and commit/verify periodic epochs (`250 / 500 / 750 / 1000`) |
| `make credit-check` | Deploy `lyquid-credit-gate` (`8083`) and verify all 4 VeriCredit personas, wash removal & custom weights |
| `python3 scripts/fetch_sepolia_logs.py && make sepolia-check` | Fetch authentic Sepolia WETH logs and issue a 3-of-4 certified Sepolia balance proof |
| `./scripts/run-proof.sh --holder 0x... --block 1000` | Submit an on-demand P0 query to the running local Indexer |
| `./scripts/stop-step4.sh` | Stop all background devnet nodes and HTTP proxies |

### 7.2 Local Ports & HTTP Diagnostic Endpoints (JSON + CORS Enabled)

| Port | Service / Lyquid | Endpoints |
|---|---|---|
| `8545` | Anvil local sequencer | JSON-RPC |
| `10087 / 11087 / 12087 / 13087` | Lyquor 4-node localnet (3-of-4 committee) | Node RPC / WS |
| `8081` | `lyquid-indexer` | `/api/certificate`, `/api/certified`, `/api/credit-certificate`, `/api/epoch-checkpoints`, `/api/sepolia-certificate` |
| `8080` | `lyquid-consumer` (`AirdropGateLyquid`) | `/api/result`, `/api/history` |
| `8082` | `lyquid-governance-gate` (`GovernanceGateLyquid`) | `/api/result`, `/api/history` |
| `8083` | `lyquid-credit-gate` (`VeriCreditGateLyquid`) | `/api/credit`, `/api/history` |

### 7.3 Machine-Verifiable Evidence Artifacts for Final Report

| Artifact Path (generated under `.run/`) | Verified Claims Supported |
|---|---|
| `.run/step4/certificate.json` & `gate-cases.json` | Native 3-of-4 `OracleCert` sample + same-certificate reuse across 4 positive/negative threshold cases |
| `.run/acceptance/historical-balances.json` | 308 EVM `Transfer` logs + 18 archival `balanceOf` queries matching Python & Rust certificates |
| `.run/acceptance/e3-results.json` | All 8 E3 audit records (tx hashes, revocation blocking, `0.78s` 1-node pause certified, `30.2s` 2-node pause blocked) |
| `.run/acceptance/epoch-checkpoints.json` | 4 periodic global Epoch Checkpoints (`250 / 500 / 750 / 1000`) with Merkle roots & holder counts |
| `.run/acceptance/credit-results.json` | 4 VeriCredit personas, `2,000,000` wash volume stripped, dynamic fee/LTV quotes & custom-weight delta |
| `.run/acceptance/sepolia-certificate.json` | Authentic Sepolia WETH certificate (block `12184649`, `0.002 WETH`) signed by committee nodes `[0, 1, 3]` |

---

## 8. Limitations & Future Work

1. **In-Guest Live Sepolia RPC Ingestion**: The current Sepolia track uses an offline fetcher + reproducible replay to eliminate public RPC rate-limit flakiness during evaluation; future work can add retry-aware outbound RPC clients inside node `instance` methods for continuous live syncing.
2. **Full P1 Window Certificate Family**: `TWAB` replay and unit tests are implemented as read-only queries, while `MIN_BALANCE` and `ACTIVE_RATIO` can be integrated into the two-phase oracle pipeline using the same window pattern already proven by `VeriCredit`.
3. **Standalone L1 Solidity Verifier**: Exporting `QueryCertificate` verification to external Ethereum L1 contracts requires deploying a committee public-key registry and signature verifier on L1.
4. **Real-Time Volatility & Trimmed TWAP (`VeriRisk-Idx`)**: Building on the implemented `dispersion.rs` guard, future work can ingest live multi-venue price feeds to compute trimmed TWAP and realized variance $\sigma_{\text{realized}}^2$ for liquidation circuit breakers.
5. **External User Testing & CI**: Conduct external reproducibility walkthroughs with `ui/holderproof_demo.html` and automate `make check && make guest-test` in GitHub Actions.
