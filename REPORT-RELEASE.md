# QIRI 公开仓发布清理报告

日期：2026-11-12。工作分支：`chore/public-release-cleanup`。基线：main `44bb27aca7c9dfb768b1d219a9ed71108f8786e6`。

结论：R1–R4 已实施，R5 完成自动化目录改名并按条件保留 CLI/SDK 名称；R6 已审计和执行验证，但**不满足全量发布验收**：全工作区检查受到基线已有系统依赖缺失阻塞，上游服务域名全零要求与“不改行为逻辑”的硬边界冲突，仍有明确列出的剩余项。本报告不将这些结果标为通过。

## R1：README 与能力核对

README 已用中文重写，包含定位、上游归因、源码安装、启动器、兼容端点、自带 Key、已实现能力、路线图、对比表、文档与许可证入口。Cargo 的内部构建目标保持 `codex`，通过 `install .../codex .../qiri` 得到用户使用的 `qiri`；没有改 Cargo 标识。

执行了 `git log main` 及按文件路径、字符串筛选的历史检查；完整输出见 [main-feature-history.txt](release-evidence/main-feature-history.txt) 和 [provider-history.txt](release-evidence/provider-history.txt)。核对如下：

| 声明 | main 提交证据 | 代码/文件证据 |
| --- | --- | --- |
| 科研人格与三层指令 | `d538f76` 初始导入、`8cc707a` 品牌清理 | `models-manager/models.json`、`core/src/config/mod.rs`、`core/src/agents_md.rs`、启动器配置 |
| 文献检索 MCP | `d538f76`、`f5aa796`、`44bb27a` | `tools/qiri-literature/server.py` |
| 独立网页搜索/抓取 MCP | `f5aa796`、`44bb27a` | `tools/qiri-web`；搜索提供方与独立 Key 要求已说明 |
| 启动器与 provider 模板 | `d538f76`、`03acc8d`、`f5aa796` | `tools/qiri-launcher` |
| 移除上游账号/反馈产品入口 | `03acc8d`、`8cc707a` | CLI、TUI 与配置；不表示所有底层上游兼容实现均已删除 |
| 本地/云端兼容端点 | `d538f76` | provider 的可配置 `base_url`；文档明确所需 Responses/工具调用协议，未承诺任意推理 API 无条件兼容 |
| daemon 包不可用时 TUI 回退 | `995b95c` | CLI/TUI 的回退实现 |

溯源记录、文献深读、工作台、最小服务化均放在路线图，未冒充 main 已实现。溯源原型状态来自任务书，没有读取或修改其他分支。独立 Web 项目链接按任务书保留。

## R2：内部端点

- 启动器配置中的私有端点替换为 `https://api.example.com/v1`，补充自填模型/端点注释；保留所有原有配置键和 provider 表结构。`pcl` 仅作为现有本地 provider 别名保留，显示名称改为通用名称。
- doctor remedy 文案与其测试断言共两处同步使用公开占位地址，没有修改判断逻辑、测试名称或快照文件名。正确的 doctor 测试目标是 `--bin codex`。
- 当前基线启动脚本中**不存在**任务书描述的私有 `no_proxy` 条目，因此没有虚构待删内容，也没有新增代理行为；`bash -n` 通过。脚本其他代理默认值及内部 `CODEX_HOME` 读取逻辑保持原状，文档说明了配置方式。
- 任务书列出的三类私有地址模式在跟踪产品文件中均为零，详见终检表。

## R3：16 个 docs 文件逐项处理

| 文件 | 结果 |
| --- | --- |
| `agents_md.md` | 改为项目指令、本地层级与科研约定说明 |
| `authentication.md` | 改为自带 API Key 和兼容端点 |
| `config.md` | 改为本地配置层、字段、schema 链接，保留 managed hooks 技术说明 |
| `contributing.md` | 改为 QIRI issue、脱敏复现、测试与归因要求 |
| `example-config.md` | 提供可填写的 Responses provider TOML 示例 |
| `exec.md` | 改为 QIRI 非交互命令、JSON 事件及退出码说明 |
| `execpolicy.md` | 提供 prefix_rule 与仅评估、不执行的 check 示例 |
| `frontend-streaming-api.md` | 保留协议、事件、审批与鉴权内容；移除特定领域启动器和分支合并操作说明，使用通用路径与 QIRI 示例 |
| `getting-started.md` | 改为科研项目启动、MCP、代理与启动器说明 |
| `install.md` | 改为本仓源码安装、内部目标名说明与正确 doctor 测试命令 |
| `license.md` | 保留 Apache-2.0 和上游归因 |
| `sandbox.md` | 改为权限边界、审批与数据保护说明 |
| `skills.md` | 改为技能结构、发现路径与科研示例 |
| `slash_commands.md` | 对照实际枚举保留本地可用命令 |
| 原 CLA 文档 | 删除上游专属协议，同时删除引用它的 `.github/workflows/cla.yml` |
| 原开源基金文档 | 删除上游专属计划，清除产品文档入口 |

`git grep -ni chatgpt -- docs`：退出码 1、输出为空，没有归因性例外。相对 Markdown 链接检查全部通过。根安全策略和 issue 模板也改为 QIRI 接收问题的实际方式，未编造联系邮箱、赏金或响应承诺。上游 splash 无引用残留。

## R4：根目录与展示资源

- CHANGELOG 改为 QIRI 简要增量，注明 fork 来自上游仓库、导入提交及完整上游历史位置；历史没有标记精确上游提交，不猜测上游版本。
- 公告头部注释改为 QIRI，更新通用升级说明的仓库链接；保留公告结构、匹配条件与日期。
- 选择**删除**上游 `.github/codex-cli-splash.png`，同步清理 blob-size allowlist；README 不再引用上游展示图。
- LICENSE、NOTICE 与基线字节一致；未删除或修改源文件中的版权头。哈希见 [license-hashes.txt](release-evidence/license-hashes.txt)。

## R5：改名及有意保留的名称

已用 `git mv .github/codex .github/qiri` 完成目录改名，Git 记录 5 个 100% 相同内容的文件迁移。全树检索旧目录前缀无剩余引用；没有需要同步修改的 workflow/模板引用。标签提示文件名与 `CODEX_ACTION_*` 占位符保留，以免改变其调用约定。

CLI 盘点涉及 `pnpm-workspace.yaml`、锁文件 importer、dependabot、忽略规则、`scripts/stage_npm_packages.py`、npm 构建器、平台包装依赖、安装脚本和 release workflows。包身份与分平台发布名绑定；当前机器缺少 Node/npm/pnpm，基线 npm 打包测试全部 6 个平台子场景均报错。按任务书“全部引用同步且构建通过才执行”的前提，**未执行** `codex-cli/` 和包名迁移，不保留半完成的路径或命名变更。

SDK 盘点：TypeScript 包为 `@openai/codex-sdk`，公开类包括 `Codex`，查找上游命名的平台包；Python 包包括 `openai-codex`、`openai-codex-cli-bin`，导入名为 `openai_codex`、`codex_cli_bin`，还涉及生成协议、uv 锁文件、运行时 resolver 和发布验证。只改目录或包名会破坏公开导入/API 和运行时安装解析。根据同一条件及“不改行为逻辑”的硬边界，SDK 保留原名；未修改 SDK 文件，也未发布任何包。

| 有意保留 | 原因 |
| --- | --- |
| `codex-rs/`、所有 `codex-*` Rust crate 与库标识 | 明确禁止内部改名，保持依赖解析与构建一致 |
| Cargo `codex` 二进制目标 | 保持内部目标、测试和打包约定，安装时对用户命名为 qiri |
| proto 包、JSON-RPC 方法、生成类型 | 保持线协议兼容 |
| snapshot 文件名与测试名 | 保留测试标识；本次只同步 doctor 断言字符串 |
| `CODEX_HOME`、内部协议环境变量、`.codex` 默认路径与配置键 | 保持读取逻辑；启动器已映射 QIRI 配置目录 |
| `codex-cli/` 与 npm 包/平台包装名 | 条件验证不通过，整体保留 |
| SDK 包名、模块、公开类、运行时依赖名 | 名称与已发布契约及依赖解析绑定，未进行不完整迁移 |
| 底层上游认证域、路由、安装/发布标识与嵌入技能 | 改动会影响运行行为，域名清零项未达成，逐条列入审计表 |
| 上游许可证、版权与归因 | 必须保留的合规与致谢信息 |

## R6：终检审计

完整域名结果见 [grep-summary.txt](release-evidence/grep-summary.txt)；非零匹配逐行位置、分类与原因见 [domain-audit.tsv](release-evidence/domain-audit.tsv)。模式使用 `[.]` 表达字面点号，避免报告自身重复制造待清理域名。

| `git grep -niE` 模式 | 产品文件匹配行 | 结果 |
| --- | --- | --- |
| `chatgpt[.]com` | 448 | 未清零：保留运行端点、认证域、路由、测试、快照和嵌入技能资料 |
| `developers[.]openai[.]com` | 67 | 未清零：继承的技术资料、嵌入技能及部分用户提示 |
| `llmapi[.]pcl` | 0 | 通过 |
| `research-os[.]online` | 0 | 通过 |
| `100[.]75[.]` | 0 | 通过 |
| `chatgpt`，限 `docs/` | 0 | 通过 |

“跟踪文件中服务域名为零”不能通过机械替换达成：例如 agent identity 的 JWT issuer、host 白名单和模型路由必须保持与所接受的协议一致；更改它们或对应测试将违反本任务更高优先级的行为硬边界。没有用字符串拆分、编码等方式规避 grep，也没有将非零结果包装成已完成。

剩余大小写不敏感 `openai` 匹配逐个列在 [identifier-audit.tsv](release-evidence/identifier-audit.tsv)，含文件、行、字符列、分类和保留原因；共 8,033 个出现位置、7,611 个匹配行。汇总见 [audit-summary.json](release-evidence/audit-summary.json)：许可证 1、版权 2、归因/兼容协议说明 14、继承的技术资料 322、内部实现/依赖/发布与测试标识 7,694。技术资料单独列类，不将其伪装成许可证或版权。

审计范围为 Git 跟踪产品树；报告和 `release-evidence/` 是审计元数据，排除递归盘点。原始全树 grep 已在证据提交前复核；证据文件中出现的路径或上游名称均归类为“审计元数据”，不代表产品新增引用。任务书为用户提供的未跟踪文件，保持原样，不纳入发布提交。全跟踪树（含报告及证据元数据）的原始计数见 [final-tree-audit.json](release-evidence/final-tree-audit.json)，其中名称匹配行 7,968 行；新增的审计元数据匹配不属于产品引用。

## 验证与基线对比

所有 Cargo 命令使用仓库固定的 Rust 1.95.0，在相应 `codex-rs` 目录中运行。基线通过 `git archive 44bb27a` 提取到本 worktree 内的临时目录，没有切换分支、修改其他 worktree 或读取开发分支。测试日志随证据交付（仅去除行尾空白和文件末尾多余空行）；初次误选 `--lib doctor::` 的零测试不算 doctor 验证。

| 验证 | 修改后 | 基线与解释 |
| --- | --- | --- |
| `cargo test --locked -p codex-cli --bin codex doctor::` | 133 通过，0 失败，1 忽略 | 基线同为 133 通过、0 失败、1 忽略；无新增失败 |
| `cargo check --locked`（默认成员） | 未通过：缺少 GLib/GObject 开发 pkg-config 文件及 GStreamer >= 1.28 | 基线同样缺少 glib-2.0、gobject-2.0、gstreamer-1.0；两次均退出 101 |
| 网页工具 `python3 -m unittest discover -s tools/qiri-web -p 'test_*.py'` | 10 通过 | 网页工具代码未修改 |
| npm 打包 Python 测试 | 1 个测试、6 个平台子场景报错 | 基线同样 6 个错误；缺少 Node/npm/pnpm，不计为新增失败 |
| `bash -n tools/qiri-launcher/qiri` | 通过 | 启动脚本未修改 |
| TOML 解析（启动器、公告） | 通过 | 保留原配置键结构 |
| README/docs/CHANGELOG/SECURITY 相对链接 | 通过 | 逐个目标存在 |
| `git diff --check 44bb27a` | 通过 | 无新增空白错误 |
| `git diff 44bb27a -- LICENSE NOTICE` | 空 | 保持许可证和 NOTICE 原样 |

当前系统 apt 候选 GStreamer 开发包仅 1.24.2，而源码 voice-host 需要 >= 1.28。没有为让检查变绿而降低依赖版本、排除默认成员、修改配置或伪造 pkg-config。全工作区尚不能给出“构建通过、无新增失败”的完整结论；doctor 的基线对比提供受影响代码范围的验证。[baseline-comparison.json](release-evidence/baseline-comparison.json)确认 134 个测试的名称与逐项结果完全相同，默认工作区的 3 个失败系统构建脚本集合也相同。

## 提交与遗留问题

按逻辑工作项提交；R6 报告与证据由本报告所在提交交付，其余提交如下：

```text
47c9b03 docs: rewrite QIRI README with verified main capabilities
3eb2702 docs: replace upstream service guides with QIRI documentation
e37915a chore(release): refresh changelog and remove upstream splash
4943c0c chore(release): rename project automation directory to qiri
af169d0 docs: align security and issue reporting with QIRI
618a962 docs: correct doctor test target and source installation references
7f9acb0 chore(release): replace private provider examples with public placeholders
```

未执行 push、merge、checkout 到其他分支或包发布。

仍需后续独立处理：

1. 在具备匹配系统开发库的环境重跑完整默认 workspace 检查。
2. 如要求所有上游服务域名归零，需要另开允许改变底层认证、路由、内置技能和测试契约的专项任务。
3. CLI/SDK 全面改名需要一致的发布命名方案、Node 工具链与平台打包/SDK 构建验证，本次按回退规则保留。
