# HolderProof and VeriCredit-Idx

**CS5298 Final Project Report · First draft · 22 November 2026**

This report describes the three-of-four committee design introduced in commit `c11e69c`. The public repository contains the source code and reproduction instructions. [HolderProof repository](https://github.com/jerryluo0517-cyber/holderproof) · [Design baseline](https://github.com/jerryluo0517-cyber/holderproof/tree/c11e69c)

## Abstract

HolderProof certifies historical token balances so applications can use the same result to make different decisions. Four committee members replay the same ordered events. Lyquor certifies the result once at least three members agree on the complete fact. The Airdrop and Governance gates use one balance certificate to apply their own eligibility thresholds. VeriCredit-Idx uses the same certification process to turn activity within a block window into fee and loan-to-value quotes. The prototype also certifies checkpoint summaries and supports replay of a captured Sepolia WETH event window. In the earlier two-of-three prototype, replay matched all 18 historical balance reads from a local EVM ledger. The three-of-four implementation passed 36 Rust guest tests.

## 1. Problem Analysis

An airdrop and a governance proposal can both use a holder’s token balance at a snapshot block. Each application needs the answer for the same token, account and position in the chain’s history. An archival RPC endpoint can return contract state at that block. A shared indexer can serve both applications, provided that each can identify the input history, verify the issuer and check the certificate when it acts. [Ethereum JSON-RPC](https://ethereum.org/developers/docs/apis/json-rpc/)

HolderProof addresses two requirements. Committee members need a reproducible way to derive the same result from the same history. Applications then need to reuse that result while retaining control over their own thresholds, actions and records. A certificate links the answer to the original query and the checkpoint for the input data.

The ERC-20 standard defines the `Transfer` event and the `balanceOf` interface. For the fixture token, a member can reconstruct an account’s balance by processing transfers through the requested block. Historical `balanceOf` reads on an EVM ledger provide an independent way to check that calculation. [ERC-20 standard](https://eips.ethereum.org/EIPS/eip-20)

## 2. Original Contribution

HolderProof combines deterministic event replay with a certificate that independently deployed Lyquids can read in the same call that checks eligibility. The Indexer certifies the query, balance and checkpoint together. The Airdrop and Governance gates use shared validation code and store their own policies and action records. A certified balance of 62,685 meets the example airdrop threshold of 60,000 and falls below the governance threshold of 70,000.

VeriCredit-Idx extends the design to activity within a block window. Its query includes the user, window and scoring parameters. The Credit Gate uses the certified score to return a fee and a loan-to-value (LTV) quote. A separate oracle topic certifies checkpoint summaries at selected blocks, allowing readers to compare event counts and commitments over time.

The project implements the replay, fact schemas, state layout and gate policies. Lyquor provides certified calls, and the design draws on established Merkle commitments, ERC-20 event semantics and Byzantine quorum arithmetic. [Lyquor certified calls](https://docs.lyquor.dev/docs/ldk/oracle/) · [Castro and Liskov, Practical Byzantine Fault Tolerance](https://pdos.lcs.mit.edu/6.824/papers/castro-practicalbft.pdf)

## 3. Technical Design

### 3.1 Data path and state

```mermaid
flowchart LR
    A["ERC-20 fixture"] --> D["Four committee members<br/>Node-local event replay"]
    B["Credit activity fixture"] --> D
    C["Captured Sepolia WETH logs"] --> D
    D -->|"At least 3 matching proposals"| E["Lyquor certified call"]
    E --> I["Indexer network state"]
    I -->|"Fixture balance certificate"| F["Airdrop Gate"]
    I -->|"Same fixture balance certificate"| G["Governance Gate"]
    I -->|"Credit certificate"| H["Credit Gate"]
    I -->|"Epoch checkpoints"| J["Checkpoint reader"]
    I -->|"Sepolia balance certificate"| K["Sepolia certificate reader"]
```

Each member replays events locally. The Indexer stores certificates and epoch checkpoints in shared network state. Each gate stores its configuration and action or quote records. Users run queries through the CLI and use the standalone browser console to inspect built-in examples or import evidence as JSON.

| Oracle topic | Certified result |
|---|---|
| `balance_proof` | A holder’s balance through the requested fixture block. |
| `credit_proof` | A user’s activity metrics and credit score for the requested window and parameters. |
| `checkpoint_proof` | The event count, cumulative volume, holder count and Merkle root through an epoch boundary. |
| `sepolia_proof` | A holder’s replay result for the captured Sepolia WETH event window. |

### 3.2 Certification and provenance

Each committee member proposes a complete fact. For a balance query, that fact contains the canonical query, computed balance and checkpoint. The checkpoint records the dataset, block range, boundary hashes, parser version, event count and Merkle root. The Indexer excludes members who submit multiple proposals and requires identical facts from at least three distinct members. The certified write stores the fact and Lyquor’s native oracle evidence. The exported certificate includes the signer information and `Finalized` status. A repeated query uses the same hash to retrieve the stored certificate.

Any two quorums of three out of four overlap in at least two members. If at most one member is faulty, the overlap includes at least one honest member. If honest members approve at most one fact per instance and Lyquor enforces its certified-call rules, the honest member in the overlap prevents two conflicting facts from reaching quorum. Certification can proceed when three members can communicate and agree on the full fact. The Indexer also exposes an experimental dispersion check for time-skewed observations. [Lyquor certified calls](https://docs.lyquor.dev/docs/ldk/oracle/) · [Castro and Liskov, Practical Byzantine Fault Tolerance](https://pdos.lcs.mit.edu/6.824/papers/castro-practicalbft.pdf)

### 3.3 Gate policies

The two balance gates use a network-state `call!` to read a certificate from their configured Indexer. Shared validation checks the status, query, checkpoint and issuer. It also matches the exported signer information against the native oracle evidence. Each gate compares the certified balance with its threshold. When recording a claim or vote, it reads the certificate again and requires it to be finalized. The Airdrop Gate permits one qualifying claim per caller. The Governance Gate permits one qualifying vote per caller and proposal. [Balance certificate rules](../operations/step4-certificate.md)

The Credit Gate reads a credit certificate and applies the following demonstration policy. With an Indexer configured, `quote_or_fallback` applies Base terms when the certificate is unavailable, invalid or stale. [Credit Gate implementation](../../lyquid-credit-gate/src/lib.rs)

| Tier | Score | Fee | LTV |
|---|---:|---:|---:|
| VIP | 800–1,000 | 5 basis points | 85% |
| Standard | 500–799 | 15 basis points | 75% |
| Base | 0–499 | 30 basis points | 60% |

## 4. Implementation

The Rust workspace uses Lyquor LDK 0.8.4. The `holderproof-types` crate defines queries, facts, checkpoints and certificates. The Indexer handles replay, proposals and persistence. The gate core supplies shared certificate checks. The three gate crates implement application policies, actions or quotes, and diagnostic endpoints. [Shared types](../../holderproof-types/src/lib.rs) · [Indexer](../../lyquid-indexer/src/lib.rs) · [Gate core](../../holderproof-gate-core/src/lib.rs)

**Balance replay.** The Indexer processes transfers in block and log order, including those at the requested block. It hashes each event’s fields to form a Merkle leaf. The checkpoint ties the Merkle root to the dataset, block range and parser. The fixture uses deterministic synthetic block hashes. For the archival comparison, a test token executes the same transfers on a separate Anvil instance. The verification script maps fixture blocks to corresponding EVM blocks and reads historical balances. [Fixture replay](../../lyquid-indexer/src/fixture.rs) · [Historical EVM comparison](../../scripts/verify_historical_balance.py)

**Credit replay.** The 40-event fixture contains buys, sells, repayments and liquidations. For a given user, the algorithm matches opposite-direction swap amounts with the same counterparty within the configured wash window. The unmatched amount is the clean volume. The calculation uses integer arithmetic so that each member obtains the same score. For clean volume `V`, repayment count `R` and liquidation count `L`, the score is the integer quotient of `w1 × floor(log2(1 + V)) + w2 × R` and `1 + alpha × L`, capped at 1,000. The defaults are `w1 = 35`, `w2 = 20`, `alpha = 2` and a three-block wash window. [Credit replay](../../lyquid-indexer/src/credit.rs) · [Credit fixture answers](../../fixtures/credit_expected.json)

**Epoch checkpoints.** The verification script requests checkpoints at fixture blocks 250, 500, 750 and 1,000. Members summarize the event history through each block. The Indexer stores the certified summaries in network state. [Epoch verification script](../../scripts/verify_epoch_checkpoints.py)

**Sepolia replay.** The fetch script requests WETH `Transfer` logs in pages and records the boundary block hashes. The stored snapshot contains 169 normalized events from blocks 12,184,050 through 12,184,649. Starting from zero and replaying this window gives 0.002 WETH for the sample account. The snapshot metadata also includes a separate latest-balance query. [Ethereum log method](https://ethereum.github.io/execution-apis/api/methods/eth_getLogs/) · [Sepolia snapshot metadata](../../fixtures/sepolia_expected.json) · [Sepolia replay](../../lyquid-indexer/src/sepolia.rs)

## 5. Evaluation and Results

Evaluation covered replay correctness, certificate reuse, quorum availability and query latency. The table identifies the version or committee configuration for each result. The testing guide lists the commands and expected outputs for reproduction. [Testing guide](../guides/TESTING.md)

| Check | Version | Result |
|---|---|---|
| Fixture, Python and build checks | 3-of-4 integration based on `c11e69c` | Nine fixture checks and six Python tests passed. The Rust workspace compiled with the lockfile. |
| Rust guest tests | 3-of-4 integration based on `c11e69c` | 21 Indexer tests and 15 Gate Core tests passed. They cover replay, quorum, certificate validation, scoring and tier rules. |
| Local EVM comparison | `be9560d`, 2-of-3 | All 308 Transfer logs and 18 historical balance reads matched. |
| Gate policy and committee availability | `be9560d`, 2-of-3 | Both gates used the same query hash. Alice qualified for the airdrop and fell short of the governance threshold. Each gate recorded one qualifying action and rejected repeats. With one member paused, certification continued. With two paused, it stalled until a member recovered. |
| User 1 terminal query | 2-of-3 | Alice’s balance at fixture block 1,000 was 62,685. The `Finalized` certificate contained two signers and a checkpoint covering 308 events. |
| Query latency | `db1532f`, 2-of-3 | Five sequential first-time queries on one running local network had a median of 58.10 ms from submission to readable certificate. The median for the certification step was 34.22 ms. |

The EVM comparison checks replay arithmetic and block boundaries by comparing balances from event replay with historical contract state. User 1’s terminal output matched the expected balance, event count and Merkle root. The two gate decisions and latency measurements came from the team’s automated runs. [Plan 05 audit](PLAN_05_E3_AUDIT.md) · [User 1 record and timing results](PLAN_06_MEMBER_EVALUATION.md)

The credit fixture expects scores of 940 for Alice, 495 for Bob, 250 for Carol and 675 for Dave. In Bob’s case, the matching rule removes 2,000,000 units of short-window reversals from a raw volume of 2,015,000. The remaining 15,000 units contribute to a score of 495 and a Base quote. The guest tests check scoring and tier policy against fixture expectations. [Credit fixture answers](../../fixtures/credit_expected.json)

The planned 3-of-4 evaluation covers end-to-end acceptance, live credit quotes, epoch checkpoints and Sepolia certification. Timing will use the same definition as the earlier benchmark. The usability evaluation calls for at least three participants outside the team to record setup effort, certificate results, gate decisions and any assistance they receive. [External test task sheet](../guides/EXTERNAL_USER_TEST.md)

## 6. Limitations and Reflection

**Input coverage.** The Merkle root commits to the events the committee received. Complete history depends on how those events were collected and how block finality was established. Applying replay to a later capture window requires an opening balance. Tokens whose balances change outside the recorded transfer history need additional data or replay rules.

**Committee deployment.** The four processes run on one machine and replay copies of the same dataset. This setup tests agreement and recovery under controlled conditions. A shared parser or input error can affect every member’s result. Independent operators, RPC sources and hosts would exercise failure modes beyond those observed on the local network.

**Credit policy.** Same-counterparty matching within a fixed window captures one form of repeated trading. The synthetic cases measure whether the implementation follows that rule. Estimating repayment risk would require representative data and calibration of the score and quote tiers.

**Reproduction.** The CLI gives participants commands and outputs they can record. It requires compatible versions of Rust, Lyquor and Foundry, along with available local ports. Participants can use the standalone console to inspect examples and exported evidence. External walkthroughs will help identify which setup steps and certificate fields need clearer explanations.

## 7. Contributions and Attribution

The project includes event replay and validation, oracle integration, policy gates, the credit and Sepolia extensions, evaluation and documentation. Individual member responsibilities and supporting commits or artifacts are pending team confirmation. [Repository history](https://github.com/jerryluo0517-cyber/holderproof/commits/main/)

The implementation uses Rust, Lyquor LDK, Python and Foundry/Anvil. The repository includes the synthetic fixtures and the captured Sepolia WETH snapshot used for reproduction. [Lyquor build, deploy and test documentation](https://docs.lyquor.dev/docs/ldk/build-deploy-test/) · [Foundry Anvil documentation](https://www.getfoundry.sh/anvil/index.html)

## 8. AI Use Statement

OpenAI Codex assisted with simplifying certificate validation code, inspecting the repository, running automated checks, and drafting and editing this report. The team supplied the project direction and User 1’s terminal output. The report’s technical statements and results were checked against source code, fixture answers, test output and earlier evaluation records. Any additional AI use is pending team confirmation.
