# QIRI 斜杠命令

在 TUI 输入 `/` 查看当前版本实际提供的命令；可用项受配置和运行环境影响。

| 命令 | 用途 |
| --- | --- |
| `/new` | 开始新对话 |
| `/resume` | 恢复已保存会话 |
| `/init` | 创建项目 AGENTS.md |
| `/model` | 选择模型 |
| `/permissions` | 查看或调整权限 |
| `/skills` | 查看技能 |
| `/mcp` | 查看 MCP 工具 |
| `/diff` | 查看工作区变更 |
| `/review` | 审查当前改动 |
| `/compact` | 压缩会话上下文 |
| `/status` | 查看当前配置和 token 用量 |
| `/export` | 导出对话 Markdown |
| `/quit`、`/exit` | 退出 |

QIRI 使用 API Key 与自己的端点；上游账号、订阅、官方云端任务或反馈入口不属于 QIRI 的接入方式。命令的实现清单见 [`slash_command.rs`](../codex-rs/tui/src/slash_command.rs)。
