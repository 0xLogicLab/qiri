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

## 当前验收状态

**DONE（按任务书的基线对照口径验收）**。Legacy 服务实现已拆除，域名逐行审计通过；38 个点名交付/改动 crate 的 `cargo check --tests` 通过，35 个库套件全部跑完且无新增失败，CLI/协议测试无新增失败，最终 CLI 的真实 profile exec 通过。未推送、未合并。

保留的协议/历史数据逐项列于下文；没有使用“删除会破坏协议”的死结例外豁免官方服务实现。仍存在的基线测试失败和本机环境限制均有对照证据，详见构建测试节。

## 分阶段删除记录

| 提交 | 项目与结果 | 验证证据 |
| --- | --- | --- |
| 0d1d0fa | 先写盘点表，确定服务/协议边界 | 本报告开头 |
| df9b42f | 删除 cloud-tasks/client/mock、云 diff 命令及构建引用 | item2-backend-tests.log |
| 155a3a1 | Responses proxy 删除官方默认 URL、官方 federation/OIDC 交换；保留显式上游协议转发 | item5-proxy-tests.log |
| b676faf | 删除反馈 HTTP transport、analytics HTTP 投递和官方 OTLP 默认值，保留本地归档/显式 collector | item6-tests.log |
| 618b269 | 删除预置官方文档技能、文档 MCP 推荐与白名单 | item3-other-tests.log、item3-mcp-tests.log |
| 9b1c6ad | Realtime sideband 仅随显式 provider/override，保留 wire events | item1-and4-tests.log |
| 463aa37 | 删除浏览器/设备码登录、官方账号处理实现与资源 | item1-and4-tests.log |
| edc0521、532713d | 删除 doctor 上游探针；验证旧 analytics 设置也不上传 | cli-baseline-comparison.json、core-analytics-regressions.log |
| 72780e9 | 删除账号云配置、工作区发现/路由；保留显式自建配置服务器 | cloud-config 13、bootstrap 1；rework-cloud-*.log |
| 33649f4 | 删除 auth 刷新/撤销、PAT/workload/JWKS/官方注册和官方 provider/catalog；默认 local 且需显式 base_url | 六个库合计 359 通过；rework-auth-final-tests.log 与覆盖其旧 provider 失败的 rework-phase2-provider-tests.log；rework-phase2-check.log |
| 182eaf3 | 删除官方安装源与容器 API 默认允许/探测 | 安装器 22 项；rework-installer-tests.log |
| 4c288e5 | 删除 PAT 活动认证剩余变体与 memory 官方账号门控；辅助任务继承配置模型，显式 GCP 目录不再按官方名称筛选；测试目录与发布目录分离 | login 147、model-provider 97、memories-write 49、models-manager 56 通过；delivery-tests.json |
| 91177ce | 修复恢复校验里残留的默认官方 provider 回退，改用 LOCAL_PROVIDER_ID | TUI 原恢复会话回归覆盖 9 种 cwd 场景通过；app-server 378 通过；tui-resume-diagnostic.log |
| b459714 | 删除 backend-client、chatgpt、git-attribution 整 crate；hosted Apps/MCP 注入、官方插件备份源、Guardian decisions、daemon update_loop；Cookie/WS 亲和性/官方主机信任；删除打包官方后端与 imagegen SDK 示例 | delivery-tests.json、delivery-baseline-comparison.json；全部运行结果及基线比较见下文 |
| b459714 | 删除 TUI 浏览器/设备码登录、analytics dashboard、backend banners、安全/订阅/重置额度/通知所有者/官方低价模型及 Reserve 提示；保留 API Key、AWS/GCP、自定义 OAuth、模型选择与本地用量 | delivery-tests.json、delivery-baseline-comparison.json；全部运行结果及基线比较见下文 |
| b459714、1eec5a4 | 旧域名夹具中性化、删除失去所属功能的快照，更新文档；官方 CDN pet 下载改为仅使用本地素材 | delivery-tests.json、delivery-baseline-comparison.json；全部运行结果及基线比较见下文 |

托管服务删除与 TUI 调用方在 `b459714` 原子提交：两侧共同依赖 backend-client/chatgpt，入口、实现、Cargo/Bazel 与所属测试一并移除，保持最终交付图一致。`1eec5a4` 单独收尾外部夹具、Bazel 时长表与 README。

## 保留项与可达性

保留 Responses/Chat Completions 数据格式、流式事件、tool call、显式 endpoint/API Key、自定义 OAuth、通用 MCP/HTTP/WebSocket、网络隔离与本地 exec。默认 provider 为 local，必须显式配置 base_url；发布的模型目录为空，模型 ID/目录来自调用者配置或显式端点。

保留 74 个 account/apps 数据定义及生成 SDK/schema 镜像，逐项见 `strip-evidence/retained-schema.tsv`：官方请求在分发器拒绝；API-key 登录、AWS/GCP 和通用 OAuth 仍处理其通用变体。Schema 保留仅维护客户端数据合同，不含服务 transport。另见 `retained-runtime-data.tsv`，逐项说明 JWT 字段、身份本地签名、显式自建 runtime registration、旧事件/配置数据的原因和调用可达性。没有用“删除会破坏协议”的死结例外豁免任何官方服务实现。

JWT 中 `https://api.openai.com/auth`、`/profile` 是 JSON 字段名：只参与解码/本地比较，不交给 URL parser 或 HTTP 客户端。存储的官方凭据不再激活。协议参考链接只在注释或生成 description 中存在，不是预置 MCP、文档抓取器或请求目的地。

## 域名审计

完整 `git grep` 输出：`strip-evidence/domain-grep.txt`；每一行的文件、行号、原文、归类和理由：`domain-audit.tsv`；计数：`domain-summary.json`。审计按词法范围识别测试模块，不把 `cfg(test)` 导入后的生产代码自动当作测试。两份审计输出自身排除以防递归复制；报告与证据中的历史域名单独归类为审计元数据。

扩展审查还覆盖官方 staging、发布站、Cookie 主机与静态资源 CDN：运行实现不再含其内置目的地。测试命名空间键保留为数据，其他旧域名随删除功能移除或改为 example.test/loopback。

## 构建、测试与功能证据

仓库固定 Rust 1.95.0。最终点名命令见 `strip-evidence/delivery-check-command.json`（38 个 crate，`cargo check --tests`，退出 0）；最终 CLI 构建见 `delivery-cli-build.log/.exit`（退出 0）。构建环境为 `CARGO_TARGET_DIR=strip-evidence/delivery-target`、`CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_PROFILE_TEST_DEBUG=0`，测试使用 `RUST_MIN_STACK=67108864`、`INSTA_UPDATE=no`。基线与交付分别使用隔离 target；基线副本的 9,355 个跟踪 blob 与 `7c3786d` 完全一致，见 `baseline-source-provenance.json`。

35 个库的点名构建命令见 `final-cargo-command.json`，最终编译流完整保存在 `delivery-final-build.jsonl.gz`，产物清单见 `delivery-final-build-provenance.json`。每个最终测试二进制的 SHA-256、退出码、耗时、过滤项与完整日志路径见 `delivery-tests.json`；逐套件/逐失败名称的基线比较见 `delivery-baseline-comparison.json`。**共 12,568 项通过，30 项失败全部属于基线失败集合，新增失败 0。** 修复和补齐 helper 后仅复测受影响的套件；首次诊断结果保存在 `delivery-tests-first.json`，不算最终验收。

| 最终套件 | 最终通过 / 失败 | 基线失败 | 结果 |
| --- | --- | --- | --- |
| app-server | 378 / 0 | — | 通过 |
| core | 2,904 / 7 | 15 | 无新增；5 项缺可选 code-mode-host，2 项既有 Guardian 文案快照差异 |
| Guardian v2 | 106 / 1 | 3 | 无新增；缺同一个可选 helper |
| TUI | 5,586 / 19 | 297 | 无新增；失败名称及正文逐项见比较 JSON 与日志 |
| config | 399 / 3 | 3 | 同一组既有文案断言失败 |
| 其他 30 个库 | 全部完成，零失败 | — | 账号、API、模型、MCP、网络代理、daemon、插件、记忆等见完整清单 |
| CLI lib / app-server-protocol | 10 / 0；323 / 0 | — | 通过；协议库原有 1 项 ignored |
| CLI 主命令 | 285 / 4 | 4 | 与基线相同的 4 个失败，`delivery-cli-comparison.json` |

CLI/协议原始输出为 `delivery-cli-protocol-tests.log`，既有 CLI 基线产物来源与摘要见 `cli-baseline-comparison.json`。Config 基线原始输出见 `baseline-config-tests.log`。基线 core/Guardian/TUI 的完整重跑日志为 `baseline-final-codex_core.log`、`baseline-final-codex_guardian_v2.log`、`baseline-final-codex_tui.log`。

默认 workspace 使用 `cargo check --keep-going`，两侧均退出 101。`baseline-default-keep-going.log` 与 `final-default-check.log` 保存原始输出；`default-check-comparison.json` 记录 **9 段 pkg-config 系统库诊断正文逐字一致**，10 类构建脚本失败在仅归一化隔离工作区路径后相同，包含 GLib/GObject/GIO、GStreamer、ALSA/libcap 与 V8 资源问题。可选 code-mode-host 的 V8 绑定资源不可取得，源代码未被本专项改动；`v8-helper-resource.json`、`delivery-binaries-build.log/.exit` 保存证据，未以此删掉本地代码执行协议。

两个未改动的测试会挂起，完整套件仅过滤这两项，并另做隔离对照：core 的 `environment_selection::connection_tests::lifecycle_waits_for_recovery::retry_exhaustion` 在基线/当前各 45 秒超时；TUI 的 `ide_context::ipc::tests::fetch_ide_context_does_not_fall_back_after_primary_protocol_error` 在基线完整运行超过 300 秒，基线/当前隔离各 45 秒超时。证据分别为 `baseline-isolated-recovery-timeout.json/.log`、`delivery-core-recovery-isolated.json/.log`、`baseline-ide-timeout.json`、`ide-isolated-results.json` 和对应两份隔离日志。没有将这些测试记录为通过。

最终 CLI 使用已有 Qiri profile 的显式非官方 endpoint，在本工作区内的临时隔离配置/目录真实执行 exec：退出 0，shell 工具打印 `QIRI_REAL_EXEC_OK`，最终答复包含 `QIRI_REAL_EXEC_DONE`。`real-profile-exec.json` 保存最终二进制 SHA-256 与判断结果，`.stdout/.stderr` 为脱敏原始输出；凭据未保存，临时目录已回收。复现脚本 `local-profile-exec.py` 不含凭据。`rework-removed-entries.json` 验证 login/logout/cloud/cloud-tasks 均明确提示不支持、无崩溃；TUI 入口不存在或在分发前明确拒绝。

## 交付纪律与统计

只在 chore/public-release-cleanup；不推送、不合并、不更改其他分支/worktree。用户任务书保持未跟踪、未修改。相对 `7c3786d`，产品源码、测试、夹具、文档和构建配置共改变 **1,128 个文件**（删除 323、新增 1、修改 804），新增 4,579 行、删除 **104,514 行**；报告与验收日志不计入该规模。详细统计和阶段提交清单见 `strip-evidence/change-stats.json`。

服务依赖遗留：无待拆项。保留项：74 个逐项登记的纯数据定义/生成镜像、明确列明的通用协议和自建端点接口。测试与环境遗留：上节列出的既有失败、两个挂起测试及系统库/V8 限制；最终对照无新增失败。
