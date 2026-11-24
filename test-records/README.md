# Test evidence archives

测试输入见 [`fixtures/`](../fixtures/)，测试逻辑见 [`scripts/`](../scripts/)，报告与团队外试用记录见 [`docs/reports/`](../docs/reports/)。这里保存带版本号的实际运行证据。

| 运行日期 | 被测提交 | 环境与范围 | 结果与原始证据 |
|---|---|---|---|
| 2026-11-24 | `878842d` | Windows + Ubuntu WSL2，同机四进程、3-of-4；fixture、guest、余额复用、EVM 对账、Gate 动作与撤销、暂停恢复、epoch、信用报价、已捕获 Sepolia 快照 | [说明与结果](acceptance-20261124-112044/README.md) · [完整 ZIP](acceptance-20261124-112044.zip) |

归档仅证明记录中提交和输入数据对应的测试结果。后续代码或快照更新需要单独验收。这里的本地运行不代表四台独立主机部署，也不计为团队外参与者试用。
