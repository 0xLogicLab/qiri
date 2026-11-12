# QIRI 非交互执行

`qiri exec` 适合脚本化分析和批处理，使用与 TUI 相同的 provider 配置和 API Key。

```bash
CODEX_HOME="$HOME/.qiri-codex" qiri exec '解释本项目的数据处理流程'
qiri exec --json '检查分析脚本并报告潜在问题' > events.jsonl
qiri exec --help
```

`--json` 输出事件流，适合程序读取。按退出码判断执行是否成功，不要把部分事件或模型文字当作成功标志。审批和沙箱约束仍适用；自动化应显式选择权限，避免授予不必要的访问。

非交互模式默认 `RUST_LOG=error`，诊断会内联输出。参数以当前二进制的 `--help` 为准。
