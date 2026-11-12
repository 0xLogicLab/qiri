# 安装与构建 QIRI

建议在 Linux、macOS 或 Windows 的 WSL2 环境构建。使用 [`rust-toolchain.toml`](../codex-rs/rust-toolchain.toml)固定的 Rust 版本；具体系统库需求取决于平台及所选 workspace 成员。

```bash
git clone https://github.com/0xLogicLab/qiri.git
cd qiri/codex-rs
cargo build --release --locked -p codex-cli --bin codex
mkdir -p "$HOME/.local/bin"
install -m 755 target/release/codex "$HOME/.local/bin/qiri"
```

`codex-rs`、`codex-cli` crate 和 `codex` 构建目标是保留的内部兼容名称；用户安装后的二进制为 `qiri`。若设置了 `CARGO_TARGET_DIR`，请从该目录安装产物。

按照[配置示例](example-config.md)设置模型，再启动：

```bash
CODEX_HOME="$HOME/.qiri-codex" "$HOME/.local/bin/qiri"
```

启动器用法见[快速开始](getting-started.md)。当前文档不承诺上游安装脚本或发布包适用于 QIRI。

## 开发验证

```bash
# 在 codex-rs 中运行；不使用 --all-features 作为例行验证。
cargo check --locked
cargo test --locked -p codex-cli --bin codex doctor::
# 从仓库根运行维护工具（需要 just、cargo-nextest 等依赖）。
just fmt
just fix -p <crate-you-touched>
just test -p codex-tui
```

QIRI 使用 Rust 的 `RUST_LOG` 设置日志级别。TUI 默认保存有限本地诊断，设置 `log_dir` 可启用文本日志：

```bash
qiri -c log_dir=./.qiri-log
tail -F ./.qiri-log/codex-tui.log
```

日志文件名保留内部命名。详见 [`RUST_LOG` 文档](https://docs.rs/env_logger/latest/env_logger/#enabling-logging)。
