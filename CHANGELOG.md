# QIRI 变更记录

## 公开仓发布清理

- 重写 QIRI README 与使用文档，明确自带 API Key、兼容模型端点与科研工具的配置方式。
- 将私有端点示例替换为公开占位地址，保留 Apache-2.0 许可证和上游归因。
- 清理上游专属计划、展示资源与项目自动化目录。

## QIRI main 基线（44bb27a）

- 科研人格和品牌定制，用户级科研指令与项目 AGENTS.md 配合模型模板。
- 文献检索 MCP、独立网页搜索与抓取 MCP；网页搜索使用 Keenable API。
- 启动器和自定义 provider 配置，支持兼容的本地推理服务与云端 API。
- 移除上游账号和反馈入口，修复 daemon 包不可用时 TUI 的回退。
- 提供前端 WebSocket 流式 API。

QIRI fork 基线来自 [openai/codex](https://github.com/openai/codex)，导入提交为 `d538f76`；该仓库历史未单独标记精确的上游提交，因此不推断上游版本号。上游完整历史与变更记录请参阅上游仓库。QIRI 的开发路线图见 [README](README.md)。
