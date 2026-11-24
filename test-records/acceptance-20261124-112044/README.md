# 2026-11-24 验收原始证据

本轮由 Codex 协助在 Windows + Ubuntu WSL2 上运行，使用同机四进程 Lyquor 委员会和独立 Anvil。以下记录与团队外用户试用分开保存。

**适用版本：[`878842d`](https://github.com/jerryluo0517-cyber/holderproof/tree/878842da7ef8e666de3ee667f8e6fe582cf802b9)。** 这是该版本的实测记录，不是后续提交的重新验收。发布时 `main` 已到 `4a9b25a`，Sepolia 快照已更换；本包中的 169 条事件、区块 12,184,649 和 0.002 WETH 仅对应被测版本的旧快照。

- [完整证据 ZIP](../acceptance-20261124-112044.zip)：阶段日志、证书、检查点、Gate 动作结果、交易回执、源码快照和采集脚本。
- [命令、耗时与退出码](summary.json)
- [核对后的结果摘要](verified-results.json)
- [归档哈希与发布说明](archive-metadata.json)

现有 `fixtures/` 是测试输入，`scripts/` 是测试逻辑，早期验收文档是文字结果。本包补充对应实际运行的原始输出，便于逐项核查报告。

发布包只剔除了四个误收集的 RocksDB 数据库日志（不是测试 stdout/stderr）；其余原始证据字节不变。原始完整归档仍保留在本地。ZIP 内 `PUBLICATION.json` 记录删去的路径和原始归档哈希，`SHA256SUMS.json` 校验保留文件。以下英文说明来自原审计包。

# HolderProof acceptance reproduction evidence

Date: 24 November 2026 (Asia/Shanghai)
Source commit: 878842da7ef8e666de3ee667f8e6fe582cf802b9
Result: all 12 recorded stages passed.

The tests ran on one Windows host using Ubuntu WSL2, four Lyquor processes and a
separate Anvil backend. This is not a native Windows Lyquor runtime or a deployment
on four independently hosted machines. The existing Web demo was unchanged.

## Verified observations

- 9 fixture checks, 6 Python tests, 21 Indexer and 15 Gate Core guest tests passed.
- Alice at fixture block 1000: 62685, 3 distinct signers;
  identical certificate on repeated query; both Gates reused its query hash.
- Local Anvil: 308 Transfer logs and 18 historical balanceOf reads matched the
  Python fixture reference. This checks synthetic fixture replay, not Ethereum Mainnet.
- Both Gate action suites passed threshold, caller identity, duplicate, configuration
  permission and revocation checks. Transaction receipts are included, but Anvil
  success alone is not proof that a Lyquid action committed; the scripts also check state.
- One paused member: certification succeeded. Two paused members: no certificate
  during 30.09 seconds measured by the test. Recovery
  succeeded. This is a bounded process-pause test, not a proof for every network fault.
- Epoch event counts: 89 / 172 / 255 / 308. Individual signer counts and full
  checkpoint objects are retained in results/epoch-checkpoints.json.
- Sepolia captured snapshot: 169 events, end block
  12184649, 3 signers, 2000000000000000 wei of WETH
  window net transfer amount. This is not the holder's complete historical balance.
  This run replays the captured data; it does not freshly fetch public-chain logs.
- Credit quotes and Bob's wash-window parameter comparison passed in credit.log.

## Evidence and reproduction

summary.json records every command, time and exit status. verified-results.json is
checked against the raw result files. The top-level stage logs preserve stdout/stderr;
results/ holds certificates, history reconciliation, action cases and checkpoints.
gate-transaction-receipts.json contains the E3 action and revocation receipts.
source.tar is the unmodified Git snapshot. isolation.patch changes only script ports;
ldk-isolation.patch changes the installed launcher port bases. Rust application and
fixture hashes are checked against source.tar. No scoring, quorum or validation rules
were changed. The pack includes the runner used for this session; its source paths
must be adjusted on another machine. The standard repository acceptance entry point
remains scripts/run-3of4-acceptance.sh and requires its default ports to be free.

The separate test used node APIs 20087/21087/22087/23087, Anvil 18545, and diagnostics
18080/18081/18082. The node runtime used a Linux /tmp directory so chmod 600 works.
The earlier setup attempt failed because WSL's Windows mount could not enforce the
required development-key permissions; its logs are retained in earlier-setup-failure/.
The next historical-check attempt stopped before comparison because solc 0.8.28
was absent in offline mode. Foundry installed the pinned compiler, after which the
check was rerun. historical-offline-failure.log and attempt-before-solc-install.json
retain this failure; solidity-build.log records the successful offline build.
The test's own processes were stopped after completion; the original demo was preserved.

SHA256SUMS.json authenticates the archived file bytes relative to this directory.
It is an integrity manifest, not independent authentication of who ran the tests.
