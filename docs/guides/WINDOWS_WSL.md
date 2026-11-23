# Windows 启动指南（WSL2）

在 Windows 中用 PowerShell 管理演示，并在 Windows 浏览器查询。Rust 构建、四个 Lyquor 节点、Anvil 和 Web 服务在 **WSL2 Ubuntu** 内运行。Live query 使用本地 fixture。

## 1. 首次准备

需要 Windows 10/11 的 WSL2、Ubuntu（建议 24.04）、Git 和可访问工具下载源的网络。在管理员 PowerShell 中安装 WSL（已安装则跳过）：

```powershell
wsl --install -d Ubuntu
```

按系统提示重启，首次打开 Ubuntu，完成 Linux 用户和密码设置。通过 `wsl -l -v` 确认 Ubuntu 的 VERSION 是 2。以下日常命令使用普通 PowerShell。

在 Ubuntu 终端中安装系统依赖：

```bash
sudo apt-get update
sudo apt-get install -y build-essential pkg-config libssl-dev curl git python3 make lsof ca-certificates util-linux
```

在 Windows 克隆本仓库，进入仓库根目录。支持包含空格的目录，例如 `D:\Course Projects\holderproof`。WSL 必须能访问该 Windows 盘符。脚本自动使用 `wslpath` 转换路径，不包含作者电脑的绝对路径。

```powershell
git clone https://github.com/jerryluo0517-cyber/holderproof.git
cd holderproof
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows\holderproof.ps1 -Action setup
```

`setup` 复用仓库的 `setup-env.sh` 和官方 Lyquor 安装器，可能提示安装 Rust、LDK 和 Foundry，并执行项目检查。需要联网下载。已验证的工具组合为 Rust 1.99.0 + wasm32-unknown-unknown、LDK 0.8.4、Foundry 1.7.1、Python 3.10+；官方安装器将来可能提供更新版本，请保留实际版本日志。无需把这些工具再安装到 Windows。

## 2. 启动、检查和停止

在仓库根目录运行：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows\holderproof.ps1 -Action check
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows\holderproof.ps1 -Action start
```

看到 `HolderProof is ready` 后，打开 <http://127.0.0.1:8765/>。脚本在后台保留 WSL 会话；关闭启动终端后仍可使用网页。首次构建较慢，默认最多等待 20 分钟；超时不强杀构建，查看日志后执行 `status` 或 `stop`。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows\holderproof.ps1 -Action status
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows\holderproof.ps1 -Action test
powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\windows\holderproof.ps1 -Action stop
```

`test` 执行 fixture 检查、Python 测试、workspace 编译和 Guest 测试。`start` 启动新 devnet，部署 Indexer 和两个 Gate，执行 Step 4 验收并启动 Web。重复 `start` 会报告已有服务。停止后再启动会重新创建演示状态。完整 3-of-4 验收在 Ubuntu 终端运行。[完整验收步骤](TESTING.md#4-自动验收)

发行版不叫 Ubuntu 时，在**每次**调用时加 `-Distribution Ubuntu-24.04`（替换为 `wsl -l -v` 中的名称）。其他发行版需要自行适配系统依赖，本文只覆盖 Ubuntu。默认用户必须是安装工具的 Linux 用户。

日志和 PID 保存在 `.run/windows/`，devnet 部署与验收文件位于 `.run/step4/`，均被 Git 忽略。Cargo target 默认放在 Linux 的 `~/.cache/holderproof/target`，减少跨文件系统开销。已有 Linux `CARGO_TARGET_DIR` 设置时保留该设置。

## 3. 验证网页

- Alice / block 1000：余额 62,685、Finalized、至少 3 个不同签名者；空投合格，治理不合格。
- Alice / block 950：余额 57,685，两种 Gate 显示 Not evaluated。
- Windows PowerShell：`Invoke-RestMethod http://127.0.0.1:8765/api/snapshot` 可查看响应。

Live query 和 Evidence console 的操作见 [Web 指南](WEB_GUIDE.md)。

2026-11-23 的验证在工具已安装的 Windows + WSL2 Ubuntu 环境进行，覆盖含空格路径启动、状态查询、重复启动、停止和重启。Windows HTTP 响应符合上述 Alice 区块 1000 的预期。`test` 通过了 9 项 fixture 检查、6 项 Python 测试、workspace 编译及 36 项 Guest 测试。空白系统的首次安装待验证。[WSL2 启动器提交](https://github.com/jerryluo0517-cyber/holderproof/commit/878842da7ef8e666de3ee667f8e6fe582cf802b9)

## 4. 外部试用和反馈

完成环境检查后，在 Ubuntu 终端运行外部试用记录脚本，由脚本启动本轮服务。如果 PowerShell 启动器已启动演示，请先通过 `-Action stop` 停止。Web 路线可用 Windows 浏览器完成查询，再回 Ubuntu 终端填写反馈。[Windows 外部试用步骤](EXTERNAL_USER_TEST.md#windows-外部试用)

## 5. 排错

- **发行版不存在或未初始化**：运行 `wsl -l -v`，打开对应 Ubuntu 完成初始化，并检查 `-Distribution`。
- **缺少工具**：先安装上述 apt 包，再执行 `setup`；检查工具是否安装在同一 WSL 用户下。已有自定义 LDK 可在 Linux 环境设置 `LYQUOR_LDK_DIR` / `LYQUOR_BIN_DIR`。
- **端口占用**：本演示使用 8765、8545、10087/11087/12087/13087、8080/8081/8082（8083 留给信用 Gate）。用对应启动器停止已有演示，勿同时启动 `make web`。仓库外旧版启动器需用其原来的 `-Action stop` 关闭。
- **启动失败或超时**：查看 `.run/windows/stdout.log`、`stderr.log` 和 `.run/step4/localnet.log`。`status` 只报告当前 checkout 管理的进程；`stop` 会核对 PID、命令及工作目录，不关闭整个 WSL。
- **Windows 无法访问 localhost**：确认 WSL 的 localhost forwarding 未被关闭，检查 VPN、防火墙和 WSL 网络配置；勿将本地演示服务公开到互联网。
- **脚本出现 `bash\r` 错误**：仓库通过 `.gitattributes` 强制 `.sh` 使用 LF。不要用编辑器把 Bash 脚本转成 CRLF。
