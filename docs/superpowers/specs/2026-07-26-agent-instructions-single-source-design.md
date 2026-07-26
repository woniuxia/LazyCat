# Agent 指令单一事实源设计

## 背景

仓库当前同时维护 `AGENTS.md` 与 `CLAUDE.md`，两份文件除标题和互指外完全同构。每次规则调整都需要重复修改、重复验证，并存在内容漂移风险。

## 目标

- 以 `AGENTS.md` 作为跨 Agent 项目规则的唯一事实源。
- 保留 Claude Code 可自动发现的 `CLAUDE.md` 入口。
- 消除双文件内容复制、同步规则和归一化比较。
- 保持现有规则内容和 Agent 使用行为不变。

## 方案

`AGENTS.md` 保留完整项目规范。`CLAUDE.md` 缩减为 Claude Code 适配文件，通过 `@AGENTS.md` 导入唯一事实源：

```markdown
# CLAUDE.md

@AGENTS.md
```

跨 Agent 的项目规则只修改 `AGENTS.md`。`CLAUDE.md` 不复制项目规则，也不承载普通 Claude 专属说明；确有平台专属且无法共享的规则时，才在导入语句之后增加最小补充。

## 修改范围

- `AGENTS.md`：删除双文件同步、同构检查和重复验证要求。
- `CLAUDE.md`：替换为导入适配文件。
- `process.md`、`docs/experience/README.md`：将执行入口统一描述为 `AGENTS.md`，说明 Claude Code 通过适配入口加载。
- `docs/experience/agent-workflow.md`：将“双文件同步”经验改为“单一事实源与薄适配层”。
- `apps/desktop/src/utils/windowsPackagingCommand.test.ts`：业务规则只检查 `AGENTS.md`；独立检查 `CLAUDE.md` 导入 `AGENTS.md`。

历史设计、计划和迁移审计保留原文，不批量改写。

## 验证

1. 定向运行 Windows 打包命令守卫测试。
2. 检查当前执行文档中不再要求双文件同步或同构比较。
3. 检查 `CLAUDE.md` 只包含标题和 `@AGENTS.md` 导入。
4. 运行 `git diff --check`。

## 完成标准

- 项目规则只在 `AGENTS.md` 维护一次。
- Codex 直接读取 `AGENTS.md`。
- Claude Code 通过 `CLAUDE.md` 导入相同规则。
- 现有打包规则测试继续覆盖唯一事实源，并对适配入口提供防回归检查。
