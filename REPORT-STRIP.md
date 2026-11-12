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

### 2. 云任务删除

删除 cloud-tasks / cloud-tasks-client / cloud-tasks-mock-client 整 crate、Cargo workspace/依赖/锁文件条目、Bazel 目标及 Windows 测试耗时表条目。删除 CLI `apply`（该命令实际从云任务下载 diff；本地 apply_patch 工具不受影响）、chatgpt 的任务客户端和专属夹具，以及 backend-client 的任务创建/查询/兄弟 turn API。chatgpt crate 仍被连接器调用，backend-client 仍被账号等功能调用，尚不能整删。

验证：`cargo check --offline -p codex-cli -p codex-core -p codex-app-server` 通过；`cargo test --offline -p codex-backend-client --lib`：50 通过。日志在 strip-evidence，原有账号/连接器残留不计为已删除。

### 5a. Responses 代理的官方绑定

删除代理的 OpenAI GitHub OIDC 工作负载令牌交换、续期、参数和专属测试；删除官方 Responses 默认地址，`--upstream-url` 现在必填。保留 stdin API Key、POST /v1/responses 转发、流式响应、状态码和本地诊断 dump；拒绝重定向。文档以本地端点举例。未删除 Responses 协议层。

验证：`cargo test --offline -p codex-responses-api-proxy --lib`：10 通过，0 失败。其他默认 provider / 官方认证路径仍需清理，不能据此宣称运行时已无官方服务路径。

### 6a. 回传传输与默认指标导出

删除 feedback 的硬编码上游 Sentry DSN、HTTP 上传/重试/报告分片传输及上传专属测试，移除相应 Cargo HTTP 依赖。保留本地诊断环形日志和原子保存归档；Sentry 库仅提供现有本地事件序列化数据格式，不含网络 transport 功能。

删除 analytics 的 HTTP 目标和 POST 实现及相应远程投递测试。保留本地聚合和显式调试文件 capture；正常运行不再创建 analytics 投递队列。删除 Statsig 的官方 OTLP 地址和内置 Key，默认 metrics exporter 为 none，用户显式配置的 OTLP collector 保留。兼容内部 Statsig 枚举/旧配置值仍存在，但解析结果恒为 None，无法构造官方 exporter；这不是已删除所有账号用量界面的声明。

验证：`cargo test --offline --no-fail-fast -p codex-feedback -p codex-analytics -p codex-otel --lib`：分别 26、126、36 通过，otel 另 1 忽略，无失败。完整日志见 strip-evidence/item6-tests.log。

### 3. 官方文档源

删除内置 openai-docs 技能（含下载脚本、参考资料、资源和专属测试）及 MCP 的官方文档源归因/改写包装。通用 HTTP MCP 和本地技能发现保留。沙箱/配置/feature 帮助说明改为本项目文档；TUI 的 MCP / memories 帮助链接指向本仓库。

验证：features 47、sandboxing 83、skills 54 个测试通过。MCP 初跑缺少 CLI 可执行文件；构建 CLI 后直接重跑同一测试二进制，279 个测试全部通过。config 402 个测试中 399 通过、3 失败；已在当前工作区内以 `git archive 7c3786d` 提取专项基线复跑，402 个逐项结果完全相同（config-baseline-comparison.json）。三项均为此前 QIRI/Codex 文案不一致，不为本项更改测试掩盖基线失败。

其他账号引导中的官方文档链接尚随账号界面保留，列入最终审计的未完成项；本节不宣称全部文案清理完成。

### 4a. Realtime sideband 的官方默认绑定

删除 WebRTC sideband 隐式指向官方 API 的地址；默认随已选择 provider 的 base_url，保留显式 sideband override，以支持本地/自建端点。对应测试改为 loopback 地址并验证路径。没有删除 Realtime 请求/流式事件结构，也没有删除自定义 provider 可提供的 web search、image generation、remote compaction 协议与执行能力。其官方默认 provider 和其他仅官方服务能力尚未全部清理，不计为已完成整项 4。

验证：`cargo test --offline --no-fail-fast -p codex-login -p codex-rmcp-client -p codex-api --lib` 中 codex-api 全部 200 个测试通过（同次运行 login 222、rmcp-client 260 通过，另 1 忽略）。
