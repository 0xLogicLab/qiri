# QIRI Skills

Skill 用 `SKILL.md` 描述可重复的工作方法，并可附带脚本和参考资料。QIRI 根据已发现的技能说明决定何时使用，也可以在提示中按名称请求。

项目技能可放在 `.agents/skills/<name>/SKILL.md`，用户技能可放在 `~/.agents/skills/<name>/SKILL.md`。内置技能缓存在配置目录的 `skills/.system` 下；避免直接修改生成的缓存。

```markdown
---
name: research-checklist
description: 核验科研分析的输入、代码、随机种子和引用来源。
---
检查数据来源，记录运行命令，对照原始资料核验结论。
```

在 TUI 中用 `/skills` 查看可用技能。技能中的脚本仍受审批和沙箱控制；执行前核对来源与权限。项目长期约定见 [AGENTS.md](agents_md.md)。
