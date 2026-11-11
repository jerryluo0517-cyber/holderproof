# HolderProof 协作指南

当前版本：Step 3 完成（2026-11-11）

## 1. 当前已经完成

- Lyquor LDK / Rust / Foundry 环境验证
- 4 节点本地 devnet
- Indexer Lyquid
- Consumer Lyquid
- 4 节点委员会，3-of-4 认证
- Consumer 通过 `call!` 读取认证余额
- 本地 ERC-20 Transfer fixture
- Rust 余额重算
- Rust guest tests 3 个通过
- Python ground-truth tests 6 个通过

## 2. 每台机器需要准备

1. 安装 Rust 1.99.0 和 `wasm32-unknown-unknown`
2. 安装 Foundry
3. 安装 Lyquor LDK 0.8.4：

```bash
curl -L https://ldk.lyquor.dev | bash
source ~/.zshenv
~/.shakenup/status
```

项目依赖使用 crates.io 的 `lyquid 0.8.4`，不需要硬编码本机 LDK 路径。

## 3. 本地检查

```bash
cd holderproof
make check
```

预期：

```text
Python fixture checks: 9 passed
Python unit tests: 6 passed
cargo check --workspace: PASS
```

运行 Rust guest tests：

```bash
shaker test lyquid-indexer/Cargo.toml
shaker test lyquid-consumer/Cargo.toml
```

## 4. 启动 devnet

```bash
./scripts/start-devnet.sh multi
```

会启动 4 个节点：

```text
Node 1: 127.0.0.1:10087
Node 2: 127.0.0.1:11087
Node 3: 127.0.0.1:12087
Node 4: 127.0.0.1:13087
```

该命令会占用当前终端，按 `Ctrl-C` 停止。

## 5. 部署

```bash
shaker deploy \
  --endpoint ws://127.0.0.1:10087/ws \
  --reference http://127.0.0.1:10087/lyquids:holderproof-indexer-local \
  lyquid-indexer/Cargo.toml

shaker deploy \
  --endpoint ws://127.0.0.1:10087/ws \
  --reference http://127.0.0.1:10087/lyquids:holderproof-consumer-local \
  lyquid-consumer/Cargo.toml
```

具体委员会配置、epoch 激活、Consumer 配置和查询命令见：

```text
docs/step2-operation-log.md
```

## 6. 注意：本地地址不能跨机器访问

以下 URL 只属于启动 devnet 的机器：

```text
http://127.0.0.1:8080/api/result
http://127.0.0.1:8081/api/certified
```

其他成员必须在自己的机器上启动 devnet 并完成部署。

## 7. 当前数据来源

当前使用：

```text
fixtures/transfers.json
```

这是确定性本地 fixture，不是 Sepolia 实时数据。委员会重算、认证和跨 Lyquid
调用是真实的本地 Lyquor 执行。

## 8. 推荐协作方式

### 一次性分享

发送协作 ZIP，组员解压后按照本指南运行。

### 持续同步

创建私有 GitHub Repository：

```bash
cd holderproof
git init
git add .
git commit -m "HolderProof Step 3 complete"
git remote add origin <PRIVATE_GITHUB_REPO_URL>
git branch -M main
git push -u origin main
```

之后每位成员：

```bash
git clone <PRIVATE_GITHUB_REPO_URL>
cd holderproof
git checkout -b <feature-branch>
```

不要把以下目录提交：

```text
target/
lyquid_tools_target/
__pycache__/
```

这些已写入 `.gitignore`。

## 9. 下一步工作包

- Step 4：完整查询证书
- Step 5：AirdropGate / GovernanceGate
- Step 6：E3 篡改和异常测试
- Step 7：最小 UI
- Step 8：用户测试、报告、Demo
