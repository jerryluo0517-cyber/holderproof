# 更新与导出文档

## 源文件和生成物

`scripts/docs_manifest.json` 指定四组 Markdown、HTML 和 PDF 路径。
正文以 Markdown 为准；流程图以 `docs/architecture/assets/holderproof-flow.svg` 为准。
不要只改 PNG、HTML 或 PDF。

当前源文件统一描述三成员 2-of-3 委员会、完整事实认证和两个独立消费者。
更新时同时核对代码接口、状态分层、运行结果和信任边界；计划能力不能写成已验收能力。
原始执行方案和 dated 历史记录不追溯改写，在索引中标明历史版本。

## 安装可选文档依赖

应用运行不需要这些依赖。导出环境需要 Node.js 20+、Python 3.12、marked、sharp 和 ReportLab。

```bash
npm install --prefix docs
python3 -m pip install -r scripts/requirements-docs.txt
make docs
```

`make docs` 先从 SVG 生成 PNG，再将 Markdown 转为离线 HTML 和结构化 tokens，
最后用同一组 tokens 生成 PDF。HTML 内嵌当前 SVG，不依赖在线服务。
PDF 中包含完整架构图；可编辑 Mermaid 时序图保留在 Markdown。

可以覆盖 `DOCS_NODE`、`DOCS_PYTHON` 使用已有工具环境。
若包已安装在其他位置，标准 `NODE_PATH` 也受支持。PDF 字体默认使用 macOS 的 Arial Unicode；
其他系统可通过 `HOLDERPROOF_DOC_FONT` 指定覆盖中文的 TrueType 字体。
没有该字体时回退到 STSong CID 字体，仍必须检查实际渲染结果。

## 更新后的验证

1. 重新生成全部四组导出，核对日期、章节标题和链接。
2. 渲染 PDF 的每一页，检查中文、代码注释、表格和分页。
3. 查看 SVG/PNG，确认箭头不穿过文字，节点数、门槛和双消费者一致。
4. 运行 `git diff --check`，检查当前文档没有过期的委员会规则和“待实现 Gate”描述。
5. 修改代码接口或认证逻辑时运行 `make acceptance`；仅修改排版不需要重复启动网络。

本轮 PDF 已逐页渲染检查，四份均为 3 页。运行生成过程的临时 tokens 和预览保存在 `.run/docs/`，不提交。
