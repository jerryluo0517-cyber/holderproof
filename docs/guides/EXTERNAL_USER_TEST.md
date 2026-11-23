# HolderProof 独立用户试用

方案 06 的目标是让至少三位团队外参与者各自完成一次余额查询和资格核对。
团队为每位参与者分配一个匿名编号，如 `E01`。

## 任选一条测试路线

克隆仓库，从仓库根目录运行所选命令，把 `E01` 换成团队给你的编号。
完整试用需要 Lyquor LDK 和 `shaker`。macOS 和 Linux 用户在本机终端运行，Windows 用户在 WSL2 Ubuntu 运行。开始前先关闭自己此前启动的演示，避免占用端口。[测试操作指引](TESTING.md) · [Windows 外部试用](#windows-外部试用)

| 测试路线 | 启动命令 | 在哪里核对结果 |
|---|---|---|
| 终端输入 | `python3 scripts/record_external_test.py E01` | 直接在终端查看余额证书和两个 Gate 的结果。 |
| Web 测试 | `python3 scripts/record_external_test.py E01 --web` | 在本地网页发起查询并核对结果。终端负责记录反馈。 |

### 终端输入

1. 等环境检查通过，按终端提示确认启动 demo，再确认查询 Alice 在区块 `1000` 的余额。
2. 核对终端里的证书、空投与治理结果，然后继续填写反馈。

### Web 测试

1. 按终端提示启动 demo，打开脚本给出的[本地网页](http://127.0.0.1:8765/)。
2. 在 **Live query** 选择 **Alice**，输入区块 `1000` 并点击 **Run live query**。
3. 看到 **Query complete.** 后，核对余额证书和两个资格结果，再回到终端继续。页面字段见[Web 使用说明](WEB_GUIDE.md)。

两条路线默认查询 Alice 在区块 `1000` 的余额。余额应为 `62685`，证书为 `Finalized`，有三或四名签名者。空投门槛 `60000` 通过，治理门槛 `70000` 未通过。

Web 路线也可以指定 `--holder alice|bob|carol|dave`。例如，`python3 scripts/record_external_test.py E02 --web --holder bob` 会记录 Bob 在区块 `1000` 的查询。Bob 的余额应为 `51886`，低于两个 Gate 的门槛。

也可以先在网页查询 Bob 的区块 `950`，查看余额 `46886` 的历史证书。此时两张资格卡显示 **Not evaluated**。随后查询区块 `1000` 并回到终端确认，脚本会将区块 `1000` 的证书与资格结果写入表单。历史区块的观察可写入反馈。

### Windows 外部试用

1. 按 Windows 启动指南准备 WSL2 Ubuntu，并在 PowerShell 运行 `-Action setup` 和 `-Action check`。如果已经用启动器启动了演示，先在 PowerShell 执行 `-Action stop`，让试用脚本启动本轮服务。[Windows 启动指南](WINDOWS_WSL.md)
2. 打开 Ubuntu 终端，进入同一份仓库。对于克隆在 Windows 盘符上的仓库，可用 `wslpath` 转换路径。示例中的路径和编号分别替换为实际仓库位置和分配的编号。

   ```bash
   cd "$(wslpath -u 'D:\Course Projects\holderproof')"
   python3 scripts/record_external_test.py E01 --web
   ```

3. 可以在 Windows 浏览器打开 <http://127.0.0.1:8765/>。Web 查询完成后回到 Ubuntu 终端填写反馈。走终端路线时，省去 `--web`。在反馈栏注明使用 WSL2 Ubuntu。
4. 保存表单后，如本轮启动了 demo，在 Ubuntu 终端执行 `make stop-demo`。

## 交回结果

在终端选择是否获得协助，可以补充一行卡点或建议。把终端显示路径下的 `feedback.md` 发给团队。
脚本会填写路线、提交号、系统、耗时、证书和资格结果。证书和日志随表单保存在本机 `.run/external-tests/` 下。

遇到报错或在提示处按 `q` 结束试用，也可以交回生成的表单。安装依赖前后的记录会保存在不同目录。
团队按路线汇总独立完成、协助后完成和未完成的情况。

保存证据后，如果本轮启动了 demo，运行 `make stop-demo` 关闭测试网络。Web 路线的网页服务由脚本结束时关闭。
