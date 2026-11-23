# HolderProof 文档索引

更新于 2026-11-23。当前 `main` 使用 3-of-4 委员会。课程报告中的架构图覆盖余额、信用、周期检查点和 Sepolia 四条认证路径。

## 课程报告

- CS5298 课程报告（英文）[PDF](reports/CS5298_FINAL_REPORT.pdf) · [Markdown](reports/CS5298_FINAL_REPORT.md)
- [Web 使用说明](guides/WEB_GUIDE.md)

## 方案 05 验收

- [测试操作指引](guides/TESTING.md)
- [3-of-4 完整验收脚本](../scripts/run-3of4-acceptance.sh)
- [05 E3 自审计和本地回归](reports/PLAN_05_E3_AUDIT.md)
- [外部用户试用任务单](guides/EXTERNAL_USER_TEST.md)

## 方案 06 评测

- [用户 1 操作记录与实测结果](reports/PLAN_06_MEMBER_EVALUATION.md)（3-of-4 本人验收及 2-of-3 早期基准）
- [外部用户 1 · Windows 测试结果](reports/external-tests/EXTERNAL_USER_01_WINDOWS.md)（基础检查通过，Lyquor LDK 环境受阻）
- [外部用户 2 · macOS 测试结果](reports/external-tests/EXTERNAL_USER_02_MACOS.md)（E02 独立完成 Web 试用，附功能验收与查询计时）
- [外部用户 3 · macOS Web 试用结果](reports/external-tests/EXTERNAL_USER_03_MACOS.md)（E03 独立完成）

## 03/04 文档快照

| 内容 | 源文件 | 离线 HTML | PDF |
|---|---|---|---|
| 架构与流程 | [Markdown](architecture/architecture.md) | [HTML](architecture/architecture.html) | [2026-11-15](architecture/HolderProof_Architecture_2026-11-15.pdf) |
| 代码导读 | [Markdown](guides/CODE_WALKTHROUGH.md) | [HTML](guides/CODE_WALKTHROUGH.html) | [2026-11-15](guides/HolderProof_Code_Walkthrough_2026-11-15.pdf) |
| 协作与复现 | [Markdown](guides/COLLABORATOR_GUIDE.md) | [HTML](guides/COLLABORATOR_GUIDE.html) | [2026-11-15](guides/HolderProof_Collaborator_Guide_2026-11-15.pdf) |
| 03/04 进度 | [Markdown](progress/PROJECT_PROGRESS.md) | [HTML](progress/PROJECT_PROGRESS.html) | [2026-11-15](progress/HolderProof_Project_Progress_2026-11-15.pdf) |

- [端到端 SVG](architecture/assets/holderproof-flow.svg) / [PNG](architecture/assets/holderproof-flow.png)
- [03/04 逐项验收及实现适配](reports/PLAN_03_04_ACCEPTANCE.md)
- [当前证书规则和信任边界](operations/step4-certificate.md)
- [如何更新和重建文档](MAINTAINING_DOCS.md)

Markdown 是说明正文的源文件；SVG 是流程图源文件。HTML/PDF/PNG 是对应的派生输出。
GitHub 会渲染架构 Markdown 中的 Mermaid 时序图；离线导出保留完整架构图，并注明时序图源码位置。

## 原始计划与历史记录

以下资料保持原始版本，其委员会规模、接口和测试数量不代表当前实现。

- [场景 1 原始执行方案](reports/HolderProof_Scenario1_Plan.pdf)
- [早期环境记录](operations/lyquor-step1.md)
- [早期认证调用记录](operations/step2-certified-call.md)
- [早期 fixture 重放记录](operations/step3-fixture-replay.md)
- [早期 Step 2-4 操作记录](operations/step2-operation-log.md) / [HTML](operations/step2-operation-log.html)
- [早期 Step 2-4 操作记录 PDF](reports/HolderProof_Step2-4_Operation_Log.pdf)
- [归档 Step 2-3 操作记录](archive/HolderProof_Step2-3_Operation_Log.pdf)
- [2026-11-12 代码导读 PDF](guides/HolderProof_Code_Walkthrough_2026-11-12.pdf)
- [2026-11-12 项目进度 PDF](progress/HolderProof_Project_Progress_2026-11-12.pdf)
