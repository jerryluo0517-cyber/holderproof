# HolderProof and VeriCredit-Idx

**CS5298 Final Project Report · 23 November 2026**

The project follows Direction 3, Verifiable Indexer, and evaluates the three-of-four design on a Lyquor local devnet. The public repository contains the source, test scripts and evidence summaries. [HolderProof repository](https://github.com/jerryluo0517-cyber/holderproof)

## Abstract

HolderProof certifies historical token balances so applications can use one result to make different eligibility decisions. Four committee members replay the same ordered events. Lyquor certifies the result once at least three members agree on the complete fact. The Airdrop and Governance gates read the resulting certificate and apply their own thresholds. VeriCredit-Idx uses the same certification process to turn activity within a block window into fee and loan-to-value quotes. The local evaluation reconciled 308 EVM Transfer logs and 18 historical balance reads, and verified both gates, four epoch checkpoints, four credit quotes and a certificate for a captured Sepolia WETH window. Three external users tested independently. Two completed the assigned query and eligibility checks, and one encountered a Windows setup barrier.

## 1. Problem Analysis

An airdrop and a governance proposal can both use a holder’s token balance at a snapshot block. Each application needs the answer for the same token, account and position in the chain’s history. An archival RPC endpoint can return contract state at that block. A shared indexer can serve both applications, provided that each can identify the input history, verify the issuer and check the certificate when it acts. [Ethereum JSON-RPC](https://ethereum.org/developers/docs/apis/json-rpc/)

HolderProof addresses two requirements. Committee members need a reproducible way to derive the same result from the same history. Applications then need to reuse that result while retaining control over their own thresholds, actions and records. A certificate links the answer to the original query and the checkpoint for the input data.

The target users are airdrop operators and governance developers that need to apply different policies to the same historical balance. For example, a distribution may require 60,000 tokens while a governance proposal requires 70,000. If both refer to block 1,000, a single holder may qualify for one action and fall short of the other. A shared certificate lets both applications check which history produced the balance and apply their policies independently.

The ERC-20 standard defines the `Transfer` event and the `balanceOf` interface. A historical RPC read supplies a balance from an endpoint. A subgraph provides structured event-derived data through GraphQL. HolderProof serves a different integration point by writing an issuer-bound certificate that application Lyquids can read during their own state transitions. Reusing that certificate removes the need for each gate to run its own event index for the same snapshot. [ERC-20 standard](https://eips.ethereum.org/EIPS/eip-20) · [The Graph, Subgraphs Overview](https://thegraph.com/docs/en/subgraphs/overview/)

## 2. Original Contribution

HolderProof combines deterministic event replay with a certificate that independently deployed Lyquids can read in the same call that checks eligibility. The Indexer certifies the query, balance and checkpoint together. The Airdrop and Governance gates use shared validation code and store their own policies and action records. A certified balance of 62,685 meets the example airdrop threshold of 60,000 and falls below the governance threshold of 70,000.

VeriCredit-Idx extends the design to activity within a block window. Its query includes the user, window and scoring parameters. The Credit Gate uses the certified score to return a fee and a loan-to-value (LTV) quote. A separate oracle topic certifies checkpoint summaries at selected blocks, allowing readers to compare event counts and commitments over time.

The project implements the event replays, fact schemas, checkpoint layout, gate policies and evaluation scripts. Lyquor provides certified calls and cross-Lyquid execution. Publishing periodic checkpoints and issuing holder certificates on demand lets the network expose aggregate history at fixed intervals while it stores individual results only when requested. The distinguishing feature is how these mechanisms work together in the eligibility workflow. [Lyquor certified calls](https://docs.lyquor.dev/docs/ldk/oracle/)

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

Each member replays events locally. The Indexer stores certificates and epoch checkpoints in shared network state. Each gate stores its configuration and action or quote records. The CLI and local web console accept fixture blocks from 0 to 1000. For a query at block 1000, the web console also requests eligibility decisions from both gates, whose policies use that snapshot. The console can export the complete balance certificate as JSON. The separate evidence view displays saved certificates, checkpoint and audit records, and browser simulations. [Web user guide](../guides/WEB_GUIDE.md)

In the evaluation, four separate processes on one host serve as the committee. Each maintains its replay inputs in node-local instance state. Certificate consumers read shared records from the Indexer’s network state. Separating the replay inputs from the published result allows a gate to validate the certificate with a network-state call, then persist its decision in its own state.

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

**Balance replay.** The Indexer processes transfers in block and log order, including those at the requested block. The replay applies checked integer arithmetic. It hashes each event’s fields to form a Merkle leaf. The checkpoint ties the Merkle root to the dataset, block range and parser. The fixture uses deterministic synthetic block hashes. For the archival comparison, a test token executes the same transfers on a separate Anvil instance. The verification script maps fixture blocks to corresponding EVM blocks and reads historical balances. [Fixture replay](../../lyquid-indexer/src/fixture.rs) · [Historical EVM comparison](../../scripts/verify_historical_balance.py)

```text
balance = 0
for transfer in events ordered by block and log index:
    if transfer.block > snapshot_block: stop
    if transfer.from == holder: subtract amount
    if transfer.to == holder: add amount
return balance and the checkpoint for the replayed prefix
```

**Credit replay.** The 40-event fixture contains buys, sells, repayments and liquidations. For a given user, the algorithm matches opposite-direction swap amounts with the same counterparty within the configured wash window. The unmatched amount is the clean volume. The calculation uses integer arithmetic so that each member obtains the same score. For clean volume `V`, repayment count `R` and liquidation count `L`, the score is the integer quotient of `w1 × floor(log2(1 + V)) + w2 × R` and `1 + alpha × L`, capped at 1,000. The defaults are `w1 = 35`, `w2 = 20`, `alpha = 2` and a three-block wash window. [Credit replay](../../lyquid-indexer/src/credit.rs) · [Credit fixture answers](../../fixtures/credit_expected.json)

**Epoch checkpoints.** The verification script requests checkpoints at fixture blocks 250, 500, 750 and 1,000. Members summarize the event history through each block. The Indexer stores the certified summaries in network state. [Epoch verification script](../../scripts/verify_epoch_checkpoints.py)

**Sepolia replay.** The fetch script requests WETH `Transfer` logs in pages and records the boundary block hashes. The stored snapshot contains 169 normalized events from blocks 12,184,050 through 12,184,649. Starting from zero and replaying this window gives 0.002 WETH for the sample account. The snapshot metadata also includes a separate latest-balance query. [Ethereum log method](https://ethereum.github.io/execution-apis/api/methods/eth_getLogs/) · [Sepolia snapshot metadata](../../fixtures/sepolia_expected.json) · [Sepolia replay](../../lyquid-indexer/src/sepolia.rs)

## 5. Evaluation and Results

### 5.1 Team 3-of-4 acceptance and self-audit

On 22 November 2026, a team member ran `make e3`, `make epoch-check`, `make credit-check` and `make sepolia-check` on a fresh four-node local devnet at `77d5d66`. The current acceptance script runs the same classes of checks and saves JSON evidence under `.run/acceptance/`. The evaluation checks replay correctness against an independent Python implementation and an Anvil token, tests the two gates against the same certificate, and observes how the quorum behaves when members are paused. [Team acceptance record](PLAN_06_MEMBER_EVALUATION.md) · [Testing guide](../guides/TESTING.md) · [Acceptance script](../../scripts/run-3of4-acceptance.sh)

| Check | Observed result |
|---|---|
| Fixture, Python and Rust tests | Nine fixture checks, six Python tests and 36 Rust guest tests passed, and the workspace compiled. |
| Balance certificate and gate reuse | Alice’s balance at block 1,000 was 62,685. The `Finalized` certificate had three distinct signers. Alice qualified for the airdrop at 60,000 and fell short of the governance threshold of 70,000. Both gates used the same query hash, and repeating the query returned the original certificate. |
| Archival EVM comparison | All 308 Transfer logs and 18 historical `balanceOf` reads matched the Python reference. |
| Gate actions and revocation | Each gate recorded one qualifying action and rejected repeats. The initializer could revoke a certificate, after which both gates rejected actions based on it. |
| Committee availability | With one member paused, the other three certified the query. With two paused, the query remained uncertified during the observation window and finalized after recovery. |
| Epoch checkpoints | All four checkpoints had three signers. Event counts rose from 89 at block 250 to 172 at 500, 255 at 750 and 308 at 1,000. |
| Sepolia snapshot | Three members signed the certificate for block 12,184,649. Replaying the 169 Transfer events gave 0.002 WETH for the sample holder. |

The self-audit also exercised changed checkpoint fields, signer-count checks, eligibility, caller binding and administrator-only revocation. The live pause test covers process availability and recovery. The field-level checks and the pause test exercise different failure modes. [Self-audit scenarios](PLAN_05_E3_AUDIT.md)

### 5.2 Credit quotes

The Credit Gate returned the expected score, fee and loan-to-value quote for each sample. The matching rule removed 2,000,000 units of short-window reversals from Bob’s raw volume, leaving 15,000 units of clean volume. After the operator set the wash window to zero, Bob’s score rose from 495 to 740 and the quoted fee dropped from 30 to 15 basis points. [Credit fixture answers](../../fixtures/credit_expected.json) · [User 1’s quote results](PLAN_06_MEMBER_EVALUATION.md)

| Sample | Score | Tier | Fee | LTV |
|---|---:|---|---:|---:|
| Alice | 940 | VIP | 5 basis points | 85% |
| Dave | 675 | Standard | 15 basis points | 75% |
| Bob | 495 | Base | 30 basis points | 60% |
| Carol | 250 | Base | 30 basis points | 60% |

### 5.3 Local web console

The web checks ran against the local Lyquor devnet. Alice's certificate showed
50,000 at block 0, 57,685 at block 950, and 62,685 at block 1000. The historical
queries left gate state unchanged and the page marked both decisions as
`Not evaluated`. At block 1000, the page showed the airdrop decision as
eligible and the governance decision as ineligible. The API returned a client
error for out-of-range or fractional blocks. Importing the saved block 950
certificate into the evidence console displayed its block, balance and signer
IDs. To repeat the browser checks, run `make web` and query Alice at blocks
1000 and 950. [Web user guide](../guides/WEB_GUIDE.md)

### 5.4 Query latency

Both timing runs used five sequential first-time queries on an already running local devnet. The 3-of-4 run verified that the certificates did not yet exist and measured query confirmation and certification, including local CLI calls. The two runs used different revisions and environments, so the observed medians describe their respective setups. [2-of-3 benchmark](PLAN_06_MEMBER_EVALUATION.md) · [3-of-4 benchmark](external-tests/EXTERNAL_USER_02_MACOS.md#五次首次查询计时)

| Committee | Revision | First-time queries | Median elapsed time |
|---|---|---:|---:|
| 2-of-3 | `db1532f` | 5 | 58.10 ms |
| 3-of-4 | `2552c5e` | 5 | 218.95 ms |

### 5.5 External user testing

Three participants outside the team operated independently. Two completed the assigned query and eligibility checks, and one stopped after encountering a Windows setup barrier. The completed Web sessions ran on the same project machine with two different participants. The feedback form records the route, revision, elapsed trial time, certificate, eligibility results and reported assistance. [External test task sheet](../guides/EXTERNAL_USER_TEST.md)

| Participant | Environment and route | Observed outcome |
|---|---|---|
| E01 | Native Windows, Git Bash | Independently ran the setup and basic checks. `make check` passed. Missing Lyquor LDK and `shaker` prevented the demo from starting. |
| E02 | macOS, Web | Completed without assistance in 67.5 s. Bob at block 1,000 had 51,886 tokens. Airdrop and governance both returned ineligible. |
| E03 | macOS, Web | Completed without assistance in 358.4 s. Alice at block 1,000 had 62,685 tokens. The airdrop returned eligible and governance returned ineligible. |

E02 and E03 each produced a finalized certificate with three distinct signers. Both gates used the certificate’s query hash. The reported trial durations run from the environment check through result confirmation. E03’s earlier terminal attempt ended before demo startup and is part of the same participant’s record. The team’s goal of three completed tasks remains one short. [E01 Windows record](external-tests/EXTERNAL_USER_01_WINDOWS.md) · [E02 Web record](external-tests/EXTERNAL_USER_02_MACOS.md#web-试用) · [E03 Web record](external-tests/EXTERNAL_USER_03_MACOS.md)

E01’s report localizes the problem to the demo toolchain. The task sheet now calls out that requirement before startup. E02’s choice of Bob also prompted the Web feedback script to accept alternate holders and check eligibility against each holder’s expected balance. The resulting form captured a case where both gates reject eligibility. [External test task sheet](../guides/EXTERNAL_USER_TEST.md) · [Web trial recorder](../../scripts/record_external_test.py)

## 6. Limitations and Reflection

**Input coverage.** The Merkle root commits to the events the committee received. Complete history depends on how those events were collected and how block finality was established. Applying replay to a later capture window requires an opening balance. Tokens whose balances change outside the recorded transfer history need additional data or replay rules. A next step is to compare a finalized Sepolia range across independently configured RPC endpoints, capture the opening `balanceOf`, and verify the replay against an archival `balanceOf` at the end block.

**Committee deployment.** The four processes run on one machine and replay copies of the same dataset. This setup tests agreement and recovery under controlled conditions. A shared parser or input error can affect every member’s result. Independent operators, RPC sources and hosts would exercise failure modes beyond those observed on the local network.

**Credit policy.** Same-counterparty matching within a fixed window captures one form of repeated trading. The synthetic cases measure whether the implementation follows that rule. Estimating repayment risk would require representative data and calibration of the score and quote tiers.

**Reproduction.** The CLI and live web console require compatible versions of Rust, Lyquor and Foundry and available local ports. Both completed external sessions used the same macOS host. Testing on a separate supported host would provide evidence about installation and startup outside the team’s environment. The Windows session identifies the LDK as a prerequisite for further platform testing.

## 7. Contributions and Attribution

| Contributor | Contributions evidenced in Git |
|---|---|
| `jerryluo0517-cyber` | Balance certification and shared gate validation, Airdrop and Governance integration, historical EVM comparison, local acceptance, live Web console and trial documentation. |
| `hwzhang0317` | Credit replay, scoring and Credit Gate, checkpoint and Sepolia certification paths and fixtures, and the dispersion check. |
| Third team member | |

[Balance and dual-gate implementation](https://github.com/jerryluo0517-cyber/holderproof/commit/154939c) · [Credit, checkpoint and Sepolia extensions](https://github.com/jerryluo0517-cyber/holderproof/commit/5f3a904) · [Dispersion check](https://github.com/jerryluo0517-cyber/holderproof/commit/d55a449) · [Live Web console](https://github.com/jerryluo0517-cyber/holderproof/commit/b87c980)

The implementation uses Rust, Lyquor LDK, Python and Foundry/Anvil. The repository includes the synthetic fixtures and the captured Sepolia WETH snapshot used for reproduction. [Lyquor build, deploy and test documentation](https://docs.lyquor.dev/docs/ldk/build-deploy-test/) · [Foundry Anvil documentation](https://www.getfoundry.sh/anvil/index.html)

## 8. AI Use Statement

OpenAI Codex assisted with certificate-code review and refactoring, test and documentation tooling, automated checks, and report drafting and editing. The team supplied the project direction and its acceptance output. The report’s technical claims were checked against the Rust code, Python reference results and saved logs. The external Web results were checked against the feedback forms and certificates, with assistance status taken from the participants’ responses.

## 9. Conclusion

HolderProof shows how a Lyquor committee can certify a historical balance once and let separate applications apply different rules to it. The local evaluation exercised correctness, certificate reuse and quorum recovery, and extended the same workflow to credit and periodic checkpoints. The external trials confirmed two Web workflows and identified a concrete setup barrier on native Windows.

## References

- Ethereum Improvement Proposals. [ERC-20 Token Standard](https://eips.ethereum.org/EIPS/eip-20).
- Ethereum Execution APIs. [JSON-RPC logs method](https://ethereum.github.io/execution-apis/api/methods/eth_getLogs/).
- The Graph. [Subgraphs Overview](https://thegraph.com/docs/en/subgraphs/overview/).
- Lyquor. [Oracle and certified calls](https://docs.lyquor.dev/docs/ldk/oracle/) and [Build, deploy and test](https://docs.lyquor.dev/docs/ldk/build-deploy-test/).
- Miguel Castro and Barbara Liskov. [Practical Byzantine Fault Tolerance](https://pdos.lcs.mit.edu/6.824/papers/castro-practicalbft.pdf). OSDI, 1999.
- Foundry. [Anvil documentation](https://www.getfoundry.sh/anvil/index.html).
- HolderProof project. [Source code, reproducibility scripts and evidence summaries](https://github.com/jerryluo0517-cyber/holderproof).
