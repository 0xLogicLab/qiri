# QIRI 配置示例

将以下内容保存到 `~/.qiri-codex/config.toml`，将模型 ID 和 URL 替换为自己的服务信息，并在启动时设置 `CODEX_HOME="$HOME/.qiri-codex"`。

```toml
model = "YOUR_MODEL_ID"
model_provider = "custom"
check_for_update_on_startup = false
approval_policy = "on-request"
sandbox_mode = "workspace-write"
developer_instructions = "你是穹理 QIRI。说明证据来源，保留分析代码，不虚构引用。"

[model_providers.custom]
name = "My OpenAI-compatible endpoint"
base_url = "https://api.example.com/v1"
env_key = "QIRI_API_KEY"
wire_api = "responses"
requires_openai_auth = false
```

```bash
export QIRI_API_KEY='YOUR_API_KEY'
CODEX_HOME="$HOME/.qiri-codex" qiri
```

本地推理服务可使用例如 `http://localhost:8000/v1`，前提是提供兼容的 Responses 接口和工具调用能力。模型提供方的认证要求以其文档为准。

带科研 MCP 工具的模板见 [`tools/qiri-launcher/config.toml`](../tools/qiri-launcher/config.toml)。使用前安装[文献工具](../tools/qiri-literature/README.md)与[网页工具](../tools/qiri-web/README.md)，并确保命令在 PATH 上。`required = true` 的工具启动失败会影响启动；不要直接启用未安装的服务。
