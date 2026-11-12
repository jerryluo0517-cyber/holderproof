# HolderProof Architecture Notes

## P0 flow

1. User requests `proof(token, holder, blockN)`.
2. Four index nodes independently replay `Transfer` events through `blockN`.
3. Each node returns `balance + blockHash + eventRoot + signature`.
4. The coordinator accepts a result only when at least 3 of 4 nodes agree.
5. A certificate is written to shared Lyquor state.
6. The Consumer Lyquid reads the finalized certificate and returns eligibility. Separate AirdropGateLyquid and GovernanceGateLyquid are planned next.

## State split

| Layer | Contents |
|---|---|
| Network state | certificate, query hash, block hash, event root, signatures, finality status |
| Instance state | token, holder, requested block, threshold, eligibility result |
| Node-private state | raw events, local index, replay cache, node key |
| Off-chain data | complete raw logs required for independent replay and audit |

## Certificate sketch

```text
HolderProofCertificate {
  certificateId,
  chainId,
  token,
  holder,
  blockNumber,
  blockHash,
  threshold,
  balance,
  eventMerkleRoot,
  queryHash,
  parserVersion,
  nodeSignatures[],
  networkStateId,
  status,
  createdAt
}
```

## Trust assumptions

- 3 of 4 nodes are independent and do not collude (`n/2 + 1`).
- Nodes can access complete Transfer history for the requested range.
- All nodes do not share the same undiscovered implementation bug.
- The certificate is bound to a finalized block hash, not only a block number.

The committee-rerun design is a practical middle ground between centralized
indexers and ZK proofs. It must not be described as a mathematical proof.

## End-to-end flow

![HolderProof end-to-end flow](assets/holderproof-flow.svg)

```mermaid
flowchart TD
    U[User / Script / Future UI] -->|set_query holder, block| I[Indexer Lyquid]
    I --> Q[(Network State: query_holder, query_block)]
    I -->|report_balance| R[Committee Coordinator]
    F[(Embedded transfers.json)] --> N1[Node 1 replay]
    F --> N2[Node 2 replay]
    F --> N3[Node 3 replay]
    F --> N4[Node 4 replay]
    R --> N1
    R --> N2
    R --> N3
    R --> N4
    N1 --> P[BalanceProposal]
    N2 --> P
    N3 --> P
    N4 --> P
    P --> A{aggregate: at least 3 match?}
    A -->|no| X[Reject certificate]
    A -->|yes| C[update_certified_balance]
    C --> S[(Network State: certificates by queryHash)]
    S -->|get_certificate_json| K[Consumer Lyquid]
    K --> V{validate certificate}
    V -->|invalid| X2[Reject eligibility]
    V -->|Finalized and 3 signers| B{balance >= threshold}
    B -->|yes| Y[eligible = true]
    B -->|no| N[eligible = false]
    Y -.-> G[Future: Airdrop claim]
    N -.-> H[Future: Governance / credit gate]
```

## Query sequence

```mermaid
sequenceDiagram
    participant U as User / Script
    participant I as Indexer Lyquid
    participant N as Committee Nodes
    participant D as Fixture
    participant C as Consumer Lyquid
    U->>I: set_query(holder, block)
    U->>I: report_balance()
    I->>N: propose()
    N->>D: replay Transfer events
    D-->>N: historical balance
    N-->>I: BalanceProposal
    I->>I: aggregate 3-of-4
    I->>I: update_certified_balance()
    U->>C: check_eligible()
    C->>I: get_certificate_json(holder, block)
    I-->>C: QueryCertificate
    C->>C: validate status, signers, hashes
    C-->>U: eligible / not eligible
```

> GitHub renders these Mermaid diagrams directly. Keep the diagram source in this file so it remains reviewable and updatable.
