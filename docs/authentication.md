# QIRI 认证

QIRI 使用自己的 API Key 和 OpenAI 兼容模型端点，无需官方账号、订阅或云端任务服务。

在配置中指定 `model_provider`，并为对应 provider 设置 `base_url`、`env_key`、`wire_api = "responses"` 与 `requires_openai_auth = false`。端点须支持 Responses API 和所需工具调用。完整示例见[配置示例](example-config.md)。

```bash
export QIRI_API_KEY='YOUR_API_KEY'
CODEX_HOME="$HOME/.qiri-codex" qiri
```

启动器可从 `~/.qiri/.env` 加载 Key，配置目录默认是 `~/.qiri-codex`。不要将密钥、带凭证的 URL 或私有数据提交到仓库。文件建议设置为仅所有者可读写。

认证失败时检查环境变量是否导出、端点与模型 ID 是否匹配，以及提供方是否支持所需协议。代理配置见[快速开始](getting-started.md)。
