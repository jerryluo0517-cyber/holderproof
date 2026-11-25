# HolderProof course project reproduction guide

This guide explains how to build the submitted application, reproduce the local
balance and eligibility workflow, run the acceptance suite, and locate the
evidence cited in the report. Commands use Bash on macOS, Linux or Ubuntu WSL2
unless marked as PowerShell. Run them from the repository root.

## 1. Choose the version

| Purpose | Revision | Expected Sepolia input |
|---|---|---|
| Submitted implementation baseline | `af54fe79ada3054a86fa88f3fa658278b1067d72` | 215 events, blocks 12,188,186–12,188,785 |
| Archived acceptance run cited in report Table 4 | `878842da7ef8e666de3ee667f8e6fe582cf802b9` | 169 events, blocks 12,184,050–12,184,649 |
| Final report publication | `9f11c24` | Report DOCX and PDF; this commit changes no application code |

The archived run establishes results for `878842d`, not a fresh acceptance run of
every later commit. The application baseline and archived run use different
Sepolia snapshots. Both snapshots contain captured public testnet data; their
replay computes the holder's **net transfers within the captured window**, not
an absolute historical balance unless a verified opening balance is included.

For the submitted implementation, start with a fresh checkout:

```bash
git clone https://github.com/jerryluo0517-cyber/holderproof.git
cd holderproof
git checkout --detach af54fe79ada3054a86fa88f3fa658278b1067d72
git rev-parse HEAD
```

Keep this guide open from the published repository: it was added after the pinned
application revision. On WSL2, a checkout under the Linux home directory is
recommended for acceptance testing. The archived run encountered development-key
permission problems on a Windows-mounted filesystem; see its retained failure
logs and [evidence notes](../../test-records/acceptance-20261124-112044/README.md).

## 2. Prepare the environment

Recorded tool versions are Rust 1.99.0 with `wasm32-unknown-unknown`, Lyquor LDK
0.8.4, Foundry 1.7.1 and Python 3.10 or newer. The project pins Rust in
`rust-toolchain.toml` and dependencies in `Cargo.lock`. The LDK installer is an
external dependency and may supply newer runtime tools; record the actual
versions printed by the environment check.

Bash, Git, curl, Make and lsof are required. Ubuntu also needs build tools,
pkg-config, OpenSSL development headers and CA certificates. For a new Ubuntu
environment:

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev curl git python3 make lsof ca-certificates util-linux
```

If the Lyquor toolchain is not installed, run the repository setup script. It
downloads tools and executes the project checks:

```bash
./scripts/setup-env.sh
```

For an existing installation:

```bash
./scripts/setup-env.sh --check
make check
make guest-test
```

For a custom LDK location, set `LYQUOR_LDK_DIR` to the directory containing
`scripts/localnet.sh`; optionally set `LYQUOR_BIN_DIR` to the tool directory.
See [environment configuration](TESTING.md#1-环境检查).

Windows users can follow the [WSL2 setup and PowerShell launcher guide](WINDOWS_WSL.md).
The launcher starts the application in Ubuntu; the browser runs on Windows.
Its `-Action test` runs the basic checks and guest tests. Run the complete
acceptance suite below in an Ubuntu terminal.

Allow network access for initial tool, crate, runtime-image and Solidity 0.8.28
compiler downloads. Captured Sepolia replay itself does not require a public
RPC endpoint, a wallet or testnet funds. Transactions in the acceptance suite
use the local Anvil development chain.

## 3. Reproduce the balance and eligibility workflow

Use free local ports: Anvil `8545`, node APIs `10087/11087/12087/13087`, Gate and
Indexer diagnostics `8080/8081/8082`, Web `8765`; acceptance also uses `8083` for
the Credit Gate. The LDK uses additional internal peer ports. Stop an earlier
HolderProof session with its own checkout's stop command before starting another.

```bash
make web
```

Wait for the demo deployment and open `http://127.0.0.1:8765/`. Select Alice and
query block 1000, then block 950:

| Query | Balance | Airdrop | Governance |
|---|---:|---|---|
| Alice, fixture block 1000 | 62,685 | Eligible; threshold 60,000 | Ineligible; threshold 70,000 |
| Alice, fixture block 950 | 57,685 | Not evaluated | Not evaluated |

The Web eligibility policies use snapshot block 1000. Other blocks return a
certificate without evaluating those policies. At block 1000, inspect the
certificate details: status must be `Finalized`, with at least three distinct
signers. Both Gates must reference the same query hash. Repeat the query to
check certificate reuse. These are eligibility decisions and application
records, not a public-chain token payout or an executed DAO proposal.

For a terminal-only workflow, use `make demo` instead of `make web`, then:

```bash
./scripts/run-proof.sh \
  --token 0x5298000000000000000000000000000000000001 \
  --holder 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  --block 1000
curl -fsS http://127.0.0.1:8081/api/certificate | python3 -m json.tool
curl -fsS http://127.0.0.1:8080/api/result | python3 -m json.tool
curl -fsS http://127.0.0.1:8082/api/result | python3 -m json.tool
```

After Web use, press Ctrl+C to stop the Web server, then stop the devnet:

```bash
make stop-demo
```

If started with the Windows launcher, use its `-Action stop` instead.

## 4. Run the full local acceptance suite

With earlier sessions stopped and test ports free:

```bash
./scripts/run-3of4-acceptance.sh
```

The script records the commit and environment, then runs E3 (including basic
checks, guest tests, deployment and EVM reconciliation), epoch, credit and
captured Sepolia verification. It exits nonzero on failure. On success it prints
`All 3-of-4 acceptance checks passed` and the evidence directory.

| Check | Expected observations |
|---|---|
| Basic checks and guest tests | 9 fixture checks, 6 Python tests, 21 Indexer and 15 Gate Core guest tests |
| Balance and reuse | Alice 62,685 at block 1000; finalized certificate; repeated query preserves certificate identity; both Gates reuse the query hash |
| Historical EVM comparison | 308 Transfer logs and 18 historical balance reads agree with the fixture reference |
| Gate actions and revocation | Threshold, caller, permission and duplicate-action checks pass; revocation blocks subsequent actions |
| Committee availability | One paused member permits certification; two paused members produce no certificate within the test's observation window; recovery permits certification |
| Epoch checkpoints | Blocks 250/500/750/1000 have 89/172/255/308 events |
| Credit quotes | Alice/Dave/Bob/Carol score 940/675/495/250; changing Bob's wash window to zero gives 740 |
| Sepolia at `af54fe7` | 215 events; closing block 12,188,785; sample holder net transfer amount `61398085361568879` token units, or 0.061398085361568879 WETH |
| Sepolia at archived `878842d` | 169 events; closing block 12,184,649; sample holder net transfer amount `2000000000000000` token units, or 0.002 WETH |

Use `fixtures/sepolia_expected.json` in the chosen revision as the expected
holder, block, event count, amount and Merkle root. Signer membership, deployment
identifiers, transaction hashes and timings may vary between fresh runs; do not
require the same signer subset or latency as the archived run.

Each run saves logs and result JSON under `.run/acceptance/3of4-*`, including
`commit.txt`, `environment.log`, the stage logs and copied result files.
This directory is ignored by Git; copy it elsewhere if you need to retain it.
After inspection, stop the services left running by the suite:

```bash
make stop-demo
```

See [test commands and failure diagnosis](TESTING.md) for individual stages.
First-time compilation and downloads can take substantially longer than an
already prepared run. For development-key permission failures on WSL mounts,
use a checkout and runtime storage on the Linux filesystem. For a missing solc,
allow Foundry to download the pinned compiler before retrying EVM verification.

## 5. Inspect or reproduce the archived report evidence

The published [evidence index](../../test-records/README.md) links to:

- [Run notes and version boundary](../../test-records/acceptance-20261124-112044/README.md).
- [Commands, durations and exit statuses](../../test-records/acceptance-20261124-112044/summary.json).
- [Checked result summary](../../test-records/acceptance-20261124-112044/verified-results.json).
- [Complete evidence ZIP](../../test-records/acceptance-20261124-112044.zip), containing raw logs, result JSON, transaction receipts, source snapshot, collection scripts and integrity manifest.

The ZIP SHA-256 is
`d5857990bf024389dbc54a7d8a84db01e486d0193d2b172f5c9bdb2f80bd3c7b`.
On Linux use `sha256sum`, on macOS `shasum -a 256`, or in PowerShell
`Get-FileHash -Algorithm SHA256` to check the downloaded archive. Its internal
`SHA256SUMS.json` lists the retained file hashes. Hashes check file integrity;
they do not independently authenticate the test operator.

To repeat Table 4 against the same source and captured inputs, stop the current
demo, use a separate checkout pinned to
`878842da7ef8e666de3ee667f8e6fe582cf802b9`, and follow sections 2 and 4 above.
The archived run used isolated ports and Linux runtime storage; the standard
acceptance script uses its default ports and therefore needs them free.
Its collection scripts contain session-specific paths; they are evidence of
collection, not a portable replacement for the repository acceptance command.

## 6. Preserve the captured data during reproduction

Run `make sepolia-check` against the already running demo to verify its bundled
snapshot. **Do not run `scripts/fetch_sepolia_logs.py` first when reproducing the
report.** That optional script fetches a recent window and overwrites
`fixtures/sepolia_transfers.json` and `fixtures/sepolia_expected.json`.

Fresh fetching is a separate experiment: use a separate checkout, record the
commit and changed input files, and rebuild/redeploy the Indexer before checking
the new snapshot. The data is embedded at build time; an already running
Indexer still uses its previous snapshot.

This guide covers local reproduction. It does not modify the team's AWS
deployment instructions or establish production readiness.
