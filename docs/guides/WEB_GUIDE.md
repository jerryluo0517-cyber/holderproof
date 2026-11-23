# HolderProof Web 使用说明

**Live query** 在本地 Lyquor 网络运行余额查询。**Evidence console** 用于查看已经保存的证书和验收结果，也提供可以在浏览器里操作的样例。

参加外部试用时，运行记录脚本生成 `feedback.md`。macOS 和 Linux 用户在本机终端运行，Windows 用户在 WSL2 Ubuntu 运行。[外部试用任务单](EXTERNAL_USER_TEST.md)

## 启动与停止

**macOS 和 Linux**。在仓库根目录运行以下命令。如果之前启动过自己的 demo，先关闭网页服务并执行 `make stop-demo`。[依赖和端口](TESTING.md)

```bash
make check-env
make web
```

终端显示 `HolderProof live console` 后打开网页。测试结束时，按 `Ctrl+C` 停止网页服务，再运行 `make stop-demo`。

**Windows（WSL2 Ubuntu）**。完成准备后，在仓库根目录的 PowerShell 运行以下命令。[Windows 启动指南](WINDOWS_WSL.md)

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows\holderproof.ps1 -Action start
```

看到 `HolderProof is ready` 后在 Windows 浏览器打开网页。结束时将上面的 `-Action start` 换成 `-Action stop`。

两种方式均使用[本地网页](http://127.0.0.1:8765/)。

## Live query

1. 在 **Fixture holder** 选择 Alice、Bob、Carol 或 Dave。
2. 在 **Snapshot block** 输入 `0–1000` 的整数区块。新启动时默认是 `1000`。
3. 点击 **Run live query**。展开 **Certificate details** 查看证书摘要，点击 **Download certificate JSON** 保存完整证书。

| 页面区域 | 含义 |
|---|---|
| **Fixture holder** / **Dataset** | 持有者的测试地址及使用的 ERC-20 测试数据集。余额以测试数据单位展示。 |
| **Snapshot block** | 重放转账时包含到哪个区块。选中 `950` 表示计算截至区块 `950` 的余额。 |
| **Balance certificate** | 显示余额、证书状态、持有者、区块、签名人数及检查点覆盖的转账条数。**Finalized** 表示证书已经签发。 |
| **Query hash** / **Event Merkle root** | 前者标识这次查询，可用来核对两个 Gate 引用的是哪份证书。后者是检查点对转账事件的承诺。 |
| **Airdrop** / **Governance** | 分别显示空投和治理门槛及资格结果。两个 Gate 都在区块 `1000` 判断资格。查询其他区块时显示 **Not evaluated**。 |

| 试用场景 | 预期结果 |
|---|---|
| Alice，区块 `1000` | 余额 `62,685`，状态 **Finalized**，至少三名委员签名。空投门槛 `60,000`，符合资格。治理门槛 `70,000`，未达到。 |
| Alice，区块 `950` | 余额 `57,685`，显示区块 `950` 的证书。两张资格卡显示 **Not evaluated**。 |

## Evidence console

启动网页后，点击顶部的 **Evidence console**。克隆仓库后，也可以在浏览器中打开本地的 [HTML 文件](../../ui/holderproof_vericredit_idx_console.html)。下表中的模拟操作在浏览器里执行，修改门槛、模拟撤销、领取或投票都只影响页面内的演示状态。导入 JSON 会显示已经保存的结果。回到实时查询时，打开[本地主页](http://127.0.0.1:8765/)。

| 标签页 | 怎么用 |
|---|---|
| **P0 Certificate & Dual Gates** | 切换内置持有者，调整门槛或点击 **Simulate Revocation**，观察两种规则的示例结果。导入余额证书后，页面展示该文件的持有者、区块、余额和签名等字段。点击 **Reset Defaults** 回到样例。 |
| **VeriCredit Anti-Wash & TTL** | 调整评分参数或当前区块，观察四个样例的信用分、费率、LTV 及证书过期后的默认报价。 |
| **Sepolia & Epoch Checkpoints** | 查看 Sepolia WETH 历史窗口的证书样例及四个周期检查点。Sepolia 的余额是该窗口的重放结果。 |
| **E3 Self-Audit & Dispersion Guard** | 查看审计用例，或用预设值观察离散度阈值的通过与拒绝。导入 E3 结果可查看已完成的动作和节点暂停测试记录。 |

### 导入已保存的结果

点击 **Import Local Evidence JSON**，选择下列文件。导入后页面会切换到对应标签页。

| 文件 | 来源 |
|---|---|
| 曾从 Live query 下载的 `holderproof-*.json`，或 `.run/step4/certificate.json` | 余额查询证书。 |
| `.run/acceptance/sepolia-certificate.json` | `make sepolia-check` 保存的 Sepolia 历史证书。 |
| `.run/acceptance/epoch-checkpoints.json` | `make epoch-check` 保存的周期检查点。 |
| `.run/acceptance/e3-results.json` | `make e3` 保存的动作、撤销与委员会暂停结果。 |

运行[完整验收脚本](../../scripts/run-3of4-acceptance.sh)后，也可以从对应的 `.run/acceptance/3of4-*/` 批次目录中选取这些文件。报告取证时记录实际测试的提交号和文件来源。
