# 穹理 QIRI

**穹理 QIRI——面向 AI4S（AI for Science）科研任务的智能体，在 OpenAI Codex 基础上二次开发。**

QIRI 面向文献调研、数据处理与科研编程，通过本地命令行和工具扩展辅助研究工作。研究结论仍需结合原始资料和可复现实验核验。

## 基于 Codex

本项目基于 [OpenAI Codex](https://github.com/openai/codex)（Apache-2.0）二次开发，保留上游许可证、版权声明与历史。感谢 Codex 团队及社区提供的基础设施。QIRI 的增量集中于科研场景定制。

## 快速开始

从源码构建（Rust 版本见 `codex-rs/rust-toolchain.toml`）：

```bash
git clone https://github.com/0xLogicLab/qiri.git
cd qiri/codex-rs
cargo build --release --locked -p codex-cli --bin codex
# 保留内部构建目标名，将产物安装为面向用户的 qiri。
mkdir -p "$HOME/.local/bin"
install -m 755 target/release/codex "$HOME/.local/bin/qiri"
cd ..
```

使用自己的 API Key 和 OpenAI 兼容端点，无需 OpenAI 或 ChatGPT 账号。将下列配置写入 `~/.qiri-codex/config.toml`，按提供方填写模型 ID 与 `base_url`：

```toml
model = "YOUR_MODEL_ID"
model_provider = "custom"
check_for_update_on_startup = false

[model_providers.custom]
name = "My model endpoint"
base_url = "https://api.example.com/v1"
env_key = "QIRI_API_KEY"
wire_api = "responses"
requires_openai_auth = false
```

该示例使用 Responses API；端点需要支持相应协议与工具调用，并非仅提供同名路径即可。可以接入本地 OpenAI 兼容推理服务（如启用相应接口的 vLLM）或云端 API。

```bash
export QIRI_API_KEY='YOUR_API_KEY'
CODEX_HOME="$HOME/.qiri-codex" "$HOME/.local/bin/qiri"
```

也可以使用 [`tools/qiri-launcher/qiri`](tools/qiri-launcher/qiri)：

```bash
mkdir -p "$HOME/.qiri"
# 将 QIRI_API_KEY=... 写入 ~/.qiri/.env；不要提交密钥。
chmod 600 "$HOME/.qiri/.env"
QIRI_CORE_BIN="$HOME/.local/bin/qiri" tools/qiri-launcher/qiri
```

启动器会读取 `~/.qiri/.env`，默认配置目录为 `~/.qiri-codex`，默认代理为 `http://127.0.0.1:7897`。按需设置 `QIRI_PROXY` 或标准代理变量；不使用代理时可令 `QIRI_PROXY=''`。启动器不会自动复制配置或安装 MCP 工具。完整科研配置模板见 [`tools/qiri-launcher/config.toml`](tools/qiri-launcher/config.toml)，先填写自己的端点、模型 ID，再安装并配置所需工具，避免启用尚未安装的 required MCP 服务。

## 已实现的 AI4S 定制

以下能力已在 main 基线 `44bb27a` 中合入，提交核对见发布报告：

- **QIRI 品牌与科研人格**：模型目录中的基础提示词模板、用户级 `developer_instructions`、项目级 `AGENTS.md` 三层配置共同约束文献调研、数据处理与编程任务；模型模板也可用 `model_instructions_file` 覆盖。
- **科研文献检索**：[`tools/qiri-literature`](tools/qiri-literature/README.md) 提供 OpenAlex/arXiv 检索 MCP 工具，返回摘要、DOI 和来源 URL。
- **通用网页工具**：[`tools/qiri-web`](tools/qiri-web/README.md) 提供 `web_search` 和 `fetch_url`；搜索使用 Keenable API，需要单独配置 `KEENABLE_API_KEY`，网页提取不包含 PDF 解析或浏览器渲染。
- **一键启动器与 provider 预置**：`tools/qiri-launcher` 集中加载环境、模型端点、科研提示词和 MCP 配置模板。
- **移除上游账号与反馈入口**：QIRI 的产品入口使用自带 API Key，不要求官方账号或订阅。
- **本地与云端双后端**：通过自定义 provider 接入支持所需协议的本地服务或云端 API。
- **TUI 回退修复**：daemon 包不可用时回退到可用的运行路径。

## 正在开发

以下为路线图，不代表当前 main 已具备完整能力：

- 文献深读闭环：全文解析、页码级引用、引文自检。
- 科研工作台基座：学科环境预置、数据登记与来源哈希、敏感数据策略。
- 溯源与运行记录：原型已完成，正在整合；目标是每次运行自动记录代码、数据、环境、模型和提示词版本，导出供评审的 session 报告。
- 最小服务化：邀请码、用户自带 Key、用量计量与预算护栏。
- 配套 Web 界面 [QIRI Web](https://github.com/0xLogicLab/qiri-web) 已单独开源，集成持续推进。

## 与上游的区别

| 方面 | OpenAI Codex 基础 | QIRI 定制 |
| --- | --- | --- |
| 定位 | 通用编程智能体 | AI4S 科研任务 |
| 接入 | 上游支持多种认证方式 | 自带 Key 与兼容 API 端点 |
| 指令与工具 | 通用智能体与扩展机制 | 科研人格、文献检索、网页检索与抓取 |
| 工程基础 | Rust CLI、TUI、沙箱、MCP | 保留基础实现与内部兼容命名 |
| 服务与协议 | 集成上游账号、云任务与回传 | 已移除全部 OpenAI 服务依赖；仅保留 OpenAI 兼容协议，通过显式本地/自定义端点接入模型。删除边界与验收证据见[专项报告](REPORT-STRIP.md) |

## 文档

- [安装与构建](docs/install.md) · [开始使用](docs/getting-started.md)
- [认证](docs/authentication.md) · [配置](docs/config.md) · [配置示例](docs/example-config.md)
- [项目指令](docs/agents_md.md) · [Skills](docs/skills.md) · [斜杠命令](docs/slash_commands.md)
- [非交互执行](docs/exec.md) · [沙箱与审批](docs/sandbox.md) · [执行策略](docs/execpolicy.md)
- [前端流式 API](docs/frontend-streaming-api.md) · [贡献指南](docs/contributing.md)

## 许可证

Apache-2.0，见 [LICENSE](LICENSE)。上游版权声明见 [NOTICE](NOTICE)。
