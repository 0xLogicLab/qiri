# QIRI 执行策略

执行策略为外部命令设置允许、提示审批或禁止的规则，与[沙箱](sandbox.md)共同限制工具执行。

规则使用 `.rules` 文件，用户规则位于 `CODEX_HOME/rules/`。例如要求执行 git push 前审批：

```python
prefix_rule(
    pattern = ["git", "push"],
    decision = "prompt",
    justification = "发布前确认目标与改动",
)
```

可用 CLI 检查匹配结果：

```bash
qiri execpolicy check --rules ~/.qiri-codex/rules/default.rules -- git push
qiri execpolicy check --help
```

检查命令只评估规则，不实际执行 `git push`。规则不应替代对脚本内容、网络访问及数据范围的审查。
