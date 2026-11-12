# Qiri OpenAI 服务依赖删除报告

## 盘点表（先于任何删除写入）

分支：`chore/public-release-cleanup`；专项基线：`7c3786d`。只修改当前工作区，不推送、不合并。用户提供的未跟踪任务书不纳入提交。

| 项目 / 功能 | 涉及 crate / 文件 | 删除方式 | 风险与保留边界 |
| --- | --- | --- | --- |
| 1. OpenAI/ChatGPT 账号、OAuth、刷新、账号界面及云配置 | login、agent-identity、cloud-config、app-server/request_processors/account_processor、TUI onboarding/status、CLI | 删除服务入口和实现；共享认证层只保留自定义端点 API Key；清理配置/测试/构建引用 | AuthManager 深度共享，不能整删 login；保留本地 API Key、自定义 OAuth 与通用 HTTP 能力需逐一辨别 |
| 2. 云端任务 | cloud-tasks、cloud-tasks-client、cloud-tasks-mock-client、backend-client、chatgpt、CLI、TUI | 可独立整删的 crate 连同 Cargo/Bazel/文档引用删除；共享 backend-client 按调用方拆解 | backend-client 同时被账号、用量、插件调用，不能未查引用就整删；不删本地 exec/apply |
| 3. 官方文档源 | skills/assets/samples/openai-docs、codex-mcp、sandboxing、TUI 提示、文档 | 删除预置文档技能、推荐 MCP 与域名白名单；改成本项目说明 | 保留用户自行配置的通用 MCP 和本地工具 |
| 4. 官方托管远程能力 | codex-api、model-provider、core、ext/web-search、ext/image-generation、realtime-webrtc、voice-host、remote compaction | 删除官方绑定及仅服务专用的入口；审查能否独立删除实现 | Responses/Chat Completions、工具调用、流式事件，以及本地端点可提供的同类协议结构必须保留 |
| 5. 内置官方端点 / 模型 / 网络特例 | model-provider-info、models-manager、http-client、network-proxy、responses-api-proxy、app-server-daemon、CLI doctor、codex-cli/scripts/init_firewall.sh | 删除官方 provider 预置、云端路由/安装/更新及白名单；以显式本地/自定义端点配置为入口 | 不能机械替换端点冒充删除；不改通用代理、网络隔离与协议线格式 |
| 6. 遥测 / 反馈 / 订阅推广 | analytics、otel、feedback、app-server/feedback_processor、TUI backend_banners / 订阅用量界面 | 删除自动回传、远程反馈上传及升级入口，保留用户指定遥测后端（如无官方绑定） | 区分本地诊断、OTLP 协议与向官方回传 |
| 7. 文档 / 文案 / 审计与验收 | README、docs、各 crate README、schema、测试夹具与快照 | 同步实际删除项；逐条审计跟踪域名；记录提交、规模、构建测试及本地 exec 证据 | 不伪造验证结果；许可证/版权/归因保留；纯数据兼容类型如需保留必须列明可达性 |

## 执行记录与证据

盘点发现服务功能横跨共享认证、配置、插件和 app-server，不能只删除题目中列出的几个 crate。各项结果、提交和验证将随执行补充。当前尚未宣称通过任何专项验收。
