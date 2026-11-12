# QIRI 沙箱与审批

沙箱控制工具能访问的文件与网络范围，审批策略决定何时需要人工确认。两者共同生效；模型指令不能替代操作系统层面的限制。

常用 `sandbox_mode`：

- `read-only`：只读访问。
- `workspace-write`：允许工作区写入，其他访问受沙箱限制。
- `danger-full-access`：关闭沙箱隔离，只在已评估风险的环境中使用。

用户配置示例：

```toml
sandbox_mode = "workspace-write"
approval_policy = "on-request"
```

实际可用策略受平台和管理配置限制。批准命令前核对路径、命令参数、网络目的地和数据敏感性。数据处理建议保留原始输入，以新文件保存结果。[执行策略](execpolicy.md)可进一步按命令前缀配置审批。
