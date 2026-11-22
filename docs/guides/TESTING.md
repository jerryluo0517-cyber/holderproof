# HolderProof 测试操作指引

第一次试用请从「手动查询与资格判断」开始。团队验收可以直接使用
`make e3`，它包含 P0 验收和 E3 回归。以下命令均从仓库根目录运行。

## 1. 环境检查

```bash
git clone https://github.com/jerryluo0517-cyber/holderproof.git
cd holderproof
./scripts/setup-env.sh --check
```

已验证的环境为 Rust 1.99.0（含 `wasm32-unknown-unknown`）、Lyquor LDK
0.8.4、Foundry 1.7.1 和 Python 3.12。系统还需提供 Bash、Make、curl
和 lsof。若检查失败，先按错误信息补齐依赖。仓库也提供
`./scripts/setup-env.sh` 安装或补齐 Lyquor、Rust 工具链，并执行基础回归。

如果终端找不到已安装的 `shaker`、`cast` 或 `forge`，可在当前终端补充默认安装路径后重试。

```bash
export PATH="$HOME/.shakenup/bin:$HOME/.cargo/bin:$PATH"
command -v shaker cast forge anvil lsof
```

本地节点及诊断服务需要空闲的 TCP 端口
`8545`、`10087`、`11087`、`12087`、`13087`、`8080`、`8081`、`8082`。
首次拉取 LDK 运行镜像或下载 Solidity 0.8.28 编译器需要网络。

## 2. 手动查询与资格判断

启动 demo。等待终端出现 `Reproduction complete.` 后再查询。

```bash
make demo
```

查询示例持有者在 fixture 区块 1000 的余额。

```bash
./scripts/run-proof.sh \
  --token 0x5298000000000000000000000000000000000001 \
  --holder 0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa \
  --block 1000
```

预期证书的 `balance` 为 `"62685"`，`status` 为 `Finalized`，
`signer_ids` 应包含三或四名不同的成员。再次执行同一查询，结果中的
`certificate_id`、`query_hash` 和 `network_state_id` 应一致。

```bash
curl -fsS http://127.0.0.1:8081/api/certificate | python3 -m json.tool
curl -fsS http://127.0.0.1:8080/api/result | python3 -m json.tool
curl -fsS http://127.0.0.1:8082/api/result | python3 -m json.tool
```

| 接口 | 应观察到的结果 |
|---|---|
| Indexer 8081 | 余额 `62685`，状态 `Finalized`，以及区块哈希、Merkle 根和签名 |
| 空投 Gate 8080 | 门槛 `60000`，`eligible: true` |
| 治理 Gate 8082 | 门槛 `70000`，`eligible: false` |

两个 Gate 的 `queryHash` 应与证书的 `query_hash` 一致。资格的差异来自各自门槛。
两个 Gate 的 `/api/history` 也可查看此前的资格检查记录。

团队外参与者可以按[独立试用任务单](EXTERNAL_USER_TEST.md)运行引导脚本，自动生成反馈表。
完成后关闭本次启动的 demo。

```bash
make stop-demo
```

## 3. 浏览器查询

从仓库根目录运行以下命令。如果此前已启动自己的 demo，先用 `make stop-demo` 停止。

```bash
make web
```

打开 [本地查询页面](http://127.0.0.1:8765/)，选择一个样例持有者，输入 `0–1000` 的整数区块，
再点击 **Run live query**。例如，Alice 在区块 `950` 的余额为 `57685`。
两个 Gate 的资格快照固定在区块 `1000`。查询其他区块时，页面显示该区块的证书，资格状态为
**Not evaluated**。在默认区块 `1000`，Alice 的余额为 `62685`，空投门槛 `60000` 通过，
治理门槛 `70000` 未通过。
展开 **Certificate details** 可以核对查询哈希与 Merkle 根，也可以下载证书 JSON。
测试时依次查询 `1000` 和 `950`，确认页面切换了证书和资格状态。
字段含义、模拟操作和可导入的文件见[Web 使用说明](WEB_GUIDE.md)。

网页演示把两个 Gate 配置为多用户模式，使用固定的资格快照和既有门槛。
网页和 devnet 运行在本机。结束演示时按 `Ctrl+C` 关闭网页服务，然后执行
`make stop-demo` 关闭 devnet。本地网页使用 `8765` 端口。外部用户按
[任务单](EXTERNAL_USER_TEST.md)任选终端输入或 Web 测试，两条路线都生成 `feedback.md`。

## 4. 自动验收

运行完整 3-of-4 验收前，先停止自己此前启动的 demo，确认本地测试端口和 `8083` 空闲。
从仓库根目录执行以下命令。

```bash
./scripts/run-3of4-acceptance.sh
```

脚本记录工具版本和提交号，依次运行 E3、周期检查点、信用与 Sepolia 验收。
每项成功后，它会把结果 JSON 和日志保存在本轮的 `.run/acceptance/3of4-*` 目录。
遇到失败时，脚本以非零状态退出，当前日志和此前通过的结果仍可查看。
运行结束后，demo 保持启动，便于检查接口结果。

也可以按目标单独运行以下命令。运行 `make acceptance` 或 `make e3` 前需先释放本地测试端口。

| 命令 | 覆盖范围 | 通过标准 |
|---|---|---|
| `make check` | fixture 校验、Python 单测、Rust workspace 编译 | 校验与测试通过，命令以 0 退出 |
| `make guest-test` | Indexer 与 Gate Core 的 Rust guest 单测 | 两套测试均无失败 |
| `make acceptance` | 基础检查、guest 测试、完整 demo、历史 EVM 余额对照 | 双 Gate 的正反结果通过，308 条 Transfer 日志和 18 次历史余额查询一致 |
| `make e3` | 完整 P0 验收，加上权限、撤销、重复动作、委员会暂停和恢复 | 合格动作各记录一次，拒绝用例不增加计数。一名成员暂停时仍能出证，两名暂停时观察窗口内无证书，恢复后可出证 |
| `make epoch-check` | 在运行中的 demo 上提交并检查周期检查点 | 区块 250、500、750 和 1000 的检查点与 fixture 一致 |
| `make credit-check` | 在运行中的 demo 上部署信用 Gate 并检查分档策略，需空闲的 8083 端口 | 四个样例的评分、费率和 LTV 与断言一致 |
| `make sepolia-check` | 在运行中的 demo 上验证仓库内置的 Sepolia WETH 快照 | 样例查询对应 169 条 Transfer 记录，窗口重放余额为 0.002 WETH |

`make e3` 会在隔离的本地 demo 中改变 Gate 配置及证书状态。
要再次观察第 2 节的初始结果，先关闭 E3 启动的 demo，再运行 `make demo`。

通过后使用 `make stop-demo` 停止本轮测试服务。参见
[用户 1 的 3-of-4 实测结果](../reports/PLAN_06_MEMBER_EVALUATION.md#用户-1-操作记录3-of-4)
和[早期 2-of-3 自审计记录](../reports/PLAN_05_E3_AUDIT.md)。

## 5. 保存证据与排查失败

| 文件 | 用途 |
|---|---|
| `.run/step4/certificate.json` | 初始化 demo 的示例证书 |
| `.run/step4/gate-cases.json` | 两个 Gate 各自的通过和拒绝结果 |
| `.run/acceptance/historical-balances.json` | 历史 EVM 余额对照 |
| `.run/acceptance/e3-results.json` | E3 的动作、权限、撤销和节点恢复证据 |
| `.run/step4/localnet.log` | 本地网络启动和运行日志 |

- **端口占用**。根据错误给出的端口检查占用进程。如果是自己已启动的 HolderProof demo，先用 `make stop-demo` 停止它。
- **guest VM 权限或初始化错误**。在允许本地进程初始化 Lyquor guest VM 的开发终端运行，先用 `make guest-test` 定位。
- **UPC 超时或启动失败**。查看 `.run/step4/localnet.log` 和当次命令输出。停止自己启动的残留 demo 后，从空闲端口重跑。E3 中暂停两名成员的超时属于预期检查，仍应以最终退出码为准。
- **查询结果不符**。记录使用的提交、holder、区块和门槛，核对查询 ID。P0 使用固定 fixture Token 和区块 `0..1000`。

反馈时附上操作系统、相关工具版本、执行命令、退出码和关键日志即可。
`.run/` 是本地证据目录，仓库已将其排除在 Git 提交之外。
