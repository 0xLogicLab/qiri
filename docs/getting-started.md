# 开始使用 QIRI

先[从源码安装](install.md)，再[配置模型端点与 API Key](authentication.md)。以下命令假设 `qiri` 已在 PATH 中，且已设置 `CODEX_HOME` 指向自己的配置目录。

```bash
export CODEX_HOME="$HOME/.qiri-codex"
qiri
qiri '解释本项目，列出复现实验的步骤'
```

进入科研项目目录后启动，先说明研究问题、数据路径、评价指标与限制。将长期约定写入 [AGENTS.md](agents_md.md)。在结果中要求代码、来源、参数和不确定性；核验文献全文后再引用。

使用[文献 MCP](../tools/qiri-literature/README.md)检索论文，使用[网页 MCP](../tools/qiri-web/README.md)搜索网页并提取正文。网页搜索需要单独的 Keenable Key，模型 Key 不会替代它。

[`tools/qiri-launcher/qiri`](../tools/qiri-launcher/qiri)可加载 `~/.qiri/.env`。其默认代理是 `http://127.0.0.1:7897`，可设置 `QIRI_PROXY=''` 禁用默认值，或配置标准代理变量指向自己的代理。启动器默认核心路径为 `~/.local/lib/qiri/qiri-core`，可以用 `QIRI_CORE_BIN` 覆盖。

常用操作见[斜杠命令](slash_commands.md)，脚本使用 [qiri exec](exec.md)，前端集成见[流式 API](frontend-streaming-api.md)。
