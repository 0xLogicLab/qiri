# QIRI 配置

QIRI 从 `CODEX_HOME/config.toml` 读取用户配置；底层变量名为兼容性保留。未设置时底层默认目录仍为 `~/.codex`，QIRI 启动器将它设为 `~/.qiri-codex`。项目级 `.codex/config.toml` 受项目信任设置约束，命令行 `-c key=value` 可临时覆盖配置。

常用字段：

| 字段 | 用途 |
| --- | --- |
| `model` | 提供方实际支持的模型 ID |
| `model_provider` | 对应 `model_providers` 下的本地别名 |
| `developer_instructions` | 用户级科研工作原则 |
| `model_instructions_file` | 从文件覆盖模型基础提示词 |
| `approval_policy` | 工具执行审批策略，例如 `on-request` |
| `sandbox_mode` | 例如 `workspace-write` 或 `read-only` |
| `mcp_servers` | MCP 命令、参数、环境变量及工具配置 |

[配置示例](example-config.md)展示端点和环境变量认证；完整字段及类型以仓库的 [`config.schema.json`](../codex-rs/core/config.schema.json) 为准。科研人格还包括模型目录模板与项目级 [AGENTS.md](agents_md.md)，并非只有单一提示词开关。

## Lifecycle hooks

管理员可以在 `requirements.toml` 顶层设置 `allow_managed_hooks_only = true`，忽略用户、项目及会话 hook 配置，只允许 requirements 和 managed config 层提供的 hooks。此选项仅支持 `requirements.toml`，放进 `config.toml` 不会启用该模式。
