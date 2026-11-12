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

当前结论：**尚未完成全部专项，不可按本任务标准验收或合并**。已完成下述独立子项并保留本地模型协议；官方认证刷新、默认 provider 等仍有运行实现。以下记录不把未完成项当作协议例外。

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

### 1a. 官方交互登录与已禁用账号处理实现

删除 login crate 的浏览器 OAuth HTTP server、设备码流程、登录回调/成功页、HTML 资源、相应集成测试与 Bazel compile_data；移除专属模板/urlencoding 依赖。保留 API Key、通用自定义 OAuth、密钥存储与共享 HTTP 认证基础。原 server 内的工作区 ID 校验移到共享认证代码，保留错误语义。

删除 app-server 的浏览器/设备码/ChatGPT token 登录方法、账号切换实现、已禁用的用量/额度重置/订阅邮件/工作区消息请求实现及其专属测试。旧 JSON-RPC 账号登录/订阅请求原本已经不支持，仍由分发器明确拒绝。API Key 自定义端点接入方式不变。

同时删除 MCP OAuth 的 ChatGPT 托管 CIMD 地址、选择逻辑和 CLI 选项；自动注册使用目标服务的 DCR，保留已有 client ID 与用户自己的 OAuth 服务。app-server 的旧 CIMD 枚举为兼容纯数据定义保留，旧请求明确提示使用 auto / dcr；不再构造托管 metadata URL。协议/生成镜像未删，34 项保留数据类型逐条列在 strip-evidence/retained-schema.tsv。

验证：login 全部 222 个、rmcp-client 全部 260 个测试通过（另 1 个子进程辅助测试忽略）。扩大 app-server 测试发现的旧枚举映射及测试导入问题已修正，最终交付检查/测试结果另列。

**未完成**：AuthManager 内的官方刷新/撤销、agent-identity/PAT/workload identity、云配置与 workspace routing、连接器以及 TUI 账号/订阅视图仍有实现。这些不是允许保留的本地模型协议例外，不能据本节将项目 1 判定为完成。

## 本地 exec 与命令入口验收

`strip-evidence/local-exec-smoke.py` 使用隔离的应用配置目录，在 loopback 启动模拟 Responses 服务。CLI 连续两次 POST /v1/responses，先执行 `printf QIRI_LOCAL_EXEC_OK`，再将成功的工具输出回传并完成助手回答；进程退出 0。请求次数、工具输出回传和最终标记见 local-exec.json，原始 JSON 事件见 local-exec.stdout。未知模型使用了现有 fallback metadata，并有明确提示。

这是**模拟模型协议/工具执行验收**，不是请求真实本地模型完成推理的验收。尚未获得真实本地模型的可用端点和模型名，不能把模拟结果算作任务要求的真实模型验收通过。

CLI 的 login / logout / cloud / cloud-tasks / apply 均实测返回错误，无崩溃；`--help` 不再展示这些入口，见 removed-entries.json 与 cli-help.txt。TUI 的账号相关残留仍列为未完成，不据 CLI 的结果推断全部 TUI 功能已经删除。

## 保留的协议与数据边界

- Responses 流式事件、工具调用结构、自定义 endpoint / API Key 和通用 HTTP/MCP 能力保留。现有协议 crate 的数据结构没有因名称含 OpenAI 而被机械删除。
- web search / image generation / compaction / realtime 可由用户自己的兼容端点提供的线格式保留；本次已拆出 realtime sideband 的隐式官方地址，其余官方绑定仍需继续审查。
- app-server-protocol 中 34 个与已删路径有关的请求/响应/枚举定义及生成镜像作为纯数据兼容保留，逐条位置、原因和运行时说明见 retained-schema.tsv。保留类型不代表保留服务实现，也不豁免尚未删除的账号刷新等运行代码。
- 共享 login / backend-client / chatgpt / cloud-config crate 仍有未清理的服务调用，不能整 crate 删除后连带破坏自定义模型接入。**当前仅删除了可独立核实的子功能，不能把“共享”当作这些服务实现允许永久保留的理由。**

### 5b. Doctor 上游探针

删除 doctor 中已无调用的上游 GitHub/Homebrew 版本探针，以及上游桌面应用 appcast HTTP 探针与其专属测试。保留 Qiri 自己的 update-feed 状态说明。macOS 网络诊断仅在实际配置了 provider base_url 时检查该地址的系统代理配置，不再为未配置端点补上官方账号服务器地址。

CLI 全套：285 通过、4 失败、1 忽略；清理前已有测试产物复跑：289 通过、相同 4 个失败、1 忽略。移除的 4 个测试均属于本节删去的上游探针；共同测试的失败集合和可解析结果均相同，见 cli-baseline-comparison.json。基线测试产物的 SHA-256、先前 release-doctor-bin.log 中的确切路径以及源码提交时间关联均记录在该 JSON，不将当前测试产物冒充基线。

## 遗留项：不满足硬验收线

以下均为**未完成工作**，不是“功能端点例外”，也不是本地模型协议层的保留理由：

| 范围 | 仍需删除/拆分的实现位置 | 状态 |
| --- | --- | --- |
| 官方认证和账号授权 | login/src/auth/manager.rs、personal_access_token.rs、workload_identity.rs；agent-identity；runtime-agent-instance/registration.rs | 官方刷新/撤销/注册仍存在 |
| 云配置与账号路由 | cloud-config；app-server/config_manager、in_process_bootstrap、account_processor/workspace_routing；exec-server-cli/config.rs | 仍有账号云配置/默认地址绑定 |
| 官方 provider 与模型 | model-provider-info、model-provider/chatgpt_base_url、provider、models_endpoint；models-manager 模型目录 | 官方预置/路由尚未清完；不能宣称默认只走本地 |
| 其他托管能力 | connectors、chatgpt、codex-mcp 的 hosted apps；core-plugins/startup_sync 的官方备份源；Guardian decisions；app-server-daemon/update_loop | 仍需拆除服务实现，保留通用本地工具/远程自建协议 |
| 账号界面与推广 | TUI onboarding/status/backend_banners、安全与订阅提示；protocol/error.rs 的订阅引导 | 仍需连同入口、配置、文案与测试清理 |
| 测试/文案 | domain-audit.tsv 中标记未完成的旧夹具、快照和产品文案 | 尚未全部删除或中性化 |
| 真正本地模型推理 | 需要可用的本地 endpoint / model / Key 环境变量信息 | 当前只有模拟端点验收 |

目前没有证据证明这些剩余实现“删掉就必然删除本地协议层”。因此本报告**不调用任务书的死结例外将它们算作完成**，也没有为 README 写入“全部 OpenAI 服务依赖已移除”的不实声明。README 已补充已完成子项和未通过全量验收的状态。

## 构建与测试证据（不能解读为全量通过）

全部 Cargo 操作使用仓库固定 Rust 1.95.0，工作目录为当前 worktree 的 codex-rs；基线提取/测试也只在本工作区内部进行，没有访问其他开发 worktree。日志仅在保存时去掉行末空白。

| 检查 | 结果 | 证据 / 限制 |
| --- | --- | --- |
| `cargo check --locked -p codex-cli -p codex-app-server -p codex-core` | 通过 | final-delivery-check.log / .exit |
| `cargo build --locked -p codex-cli --bin codex` | 通过 | final-cli-build.log / .exit；随后再次运行 loopback exec 成功 |
| 默认 `cargo check --locked` | 失败 101，与基线系统依赖错误相同 | baseline-default-check.log、final-default-check.log；default-check-comparison.json 内 3 个 pkg-config 诊断块逐字相同：glib-2.0、gobject-2.0、gstreamer-1.0。编译顺序/缓存日志不同，不声称完整日志字节相同 |
| backend-client 单元套件 | 50 通过 | item2-backend-tests.log |
| codex-api / login / rmcp-client 单元套件 | 200 / 222 / 260 通过，另 1 忽略 | item1-and4-tests.log |
| features / sandboxing / skills / MCP | 47 / 83 / 54 / 279 通过 | item3-other-tests.log、item3-mcp-tests.log；MCP 已补齐 CLI 后复跑 |
| config 单元套件 | 399 通过、3 个基线失败 | baseline-config-tests.log、item3-tests.log；402 项结果完全相同 |
| analytics / feedback / otel | 126 / 26 / 36 通过，另 1 忽略 | item6-tests.log |
| CLI 完整单元套件 | 285 通过、4 个基线失败、1 忽略 | cli-tests.log、baseline-cli-tests.log、cli-baseline-comparison.json；基线 294 项，本次 290 项，删除 4 个上游探针测试；失败集合相同 |
| core / app-server 原始单元运行 | 测试栈溢出中止 | delivery-lib-tests.log；不是编译失败，不计为通过 |
| app-server，64 MiB 测试线程栈、8 并发 | 420 通过、4 失败 | app-server-tests.log；1 项缺少 test_stdio_server，其他为 gateway 登录和 routing 超时；未完成全套基线对照，不能宣称无新增失败 |
| core，64 MiB 测试线程栈、8 并发 | 未完成 | core-tests.log；恢复重试测试等待超过 8 分钟后终止，core-full-run-limit.json；含旧遥测断言、缺少 code-mode-host、快照等失败，不能标为通过 |
| 本地协议工具调用 | 通过（模拟模型） | local-exec.json、local-exec.stdout；不替代真实本地模型验收 |

core 的 5 处旧 HTTP 遥测测试已同步为验证“即使启用旧设置也不上传”的回归测试，保留 MCP 审批决策与线程正常启动的断言；这不是通过调整产品网络行为绕过测试。其单独复跑证据与辅助 MCP 二进制补齐后的单项结果随最终记录附后。

## 审计方法

完整跟踪文件域名 grep（仅去除行末空白）见 strip-evidence/domain-grep.txt；逐行位置、分类、保留理由或未完成标记见 domain-audit.tsv，统计见 domain-summary.json。命令为 `git grep -I -n -i -E 'chatgpt[.]com|developers[.]openai[.]com|auth[.]openai[.]com|api[.]openai[.]com|openai[.]com'`。仅排除这两份审计输出本身，避免递归复制；新旧报告/证据若匹配，单独标为审计元数据，不作为产品端点豁免。

8 处协议注释引用已逐条核对，内容为工具调用、推理、结构化输出、Apps 文件元数据格式参考；注释不执行请求。生成的 schema/SDK 纯数据定义单独归类。旧服务夹具、文案及运行源码候选均标为“未完成”，没有机械替换、拆分域名字符串或将候选误标为版权/协议例外。

当前跟踪树共 865 个匹配行：117 个运行源码候选、681 个旧测试/夹具、49 个其他待审查项、10 个生成协议数据定义和 8 个已核对的协议参考注释。

**运行源码仍存在官方服务调用路径，域名硬验收不通过。** 本报告与 README 均明确这一点。

补充验证：`RUST_MIN_STACK=67108864 cargo test --locked -p codex-core --lib analytics -- --test-threads=8`：11 通过、0 失败，包含上述 5 项同步用例，日志 core-analytics-regressions.log。只证明这组改动的回归结果；不替代仍未完成的 core 全套验收。


补充验证：已构建 `cargo build --locked -p codex-rmcp-client --bin test_stdio_server`，随后 app-server 缺少辅助程序的那一项单独复跑通过（app-server-fixture-retest.log）。原始全套运行的另外 3 项失败仍未完成基线对照，不计为通过。

## 提交与删除规模

| 子项 | 提交 |
| --- | --- |
| 先写盘点表 | `0d1d0fa` |
| 云任务 crate 与云 diff 入口 | `df9b42f` |
| Responses 代理官方认证与默认地址 | `155a3a1` |
| feedback / analytics 回传与默认指标 | `b676faf` |
| 内置官方文档源 | `618b269` |
| Realtime sideband 默认官方绑定 | `9b1c6ad` |
| 官方交互登录、账号处理实现与 hosted CIMD | `463aa37` |
| Doctor 上游探针 | `edc0521` |
| core 遥测隐私回归测试 | `532713d` |

相对专项基线的产品树（排除本报告和证据）共修改 130 个文件，删除 71 个文件，新增 219 行、删除 21550 行，另含 1 个二进制文件变更。可复核统计见 change-stats.json。根 LICENSE 和 NOTICE 与专项基线无差异。

报告/审计证据由最终审计提交交付；旧的回传/文档本地提交曾仅为校正 Cargo.lock 的分项边界而重建，表中均为最终分支上的提交号，没有推送或合并。

仍在 `chore/public-release-cleanup`。用户提供的两份任务书保持未跟踪、未修改。没有修改主 worktree 或其他开发分支。

## Supervisor rework — 阶段 1：账号云配置与工作区路由

已删除 cloud-config 的 ChatGPT 来源、后台账号 bundle 客户端及专用测试；app-server 不再安装账号 workspace resolver、不再在登录通知中触发账号读取/工作区迁移。CLI、exec-server-cli 和 Windows 策略加载只接受 requirements 中显式配置的自定义配置服务器。bootstrap 初始网络权限不再预设官方账号地址。

自建配置服务器的通用 OAuth、按端点/凭据隔离的缓存和网络策略校验保留；其 HTTP/DTO 已独立实现，cloud-config 不再依赖 backend-client。不是用假域名替换官方服务。旧 GetAccount 纯协议请求明确拒绝，相关 TUI 入口仍待后续阶段清除。

验证：`cargo test --locked -p codex-cloud-config --lib` 13 通过；app-server `in_process::bootstrap` 1 通过。日志：rework-cloud-tests.log、rework-cloud-bootstrap-tests.log。交付 crate 检查见 rework-cloud-check.log。当前仍不宣称全项 DONE。
