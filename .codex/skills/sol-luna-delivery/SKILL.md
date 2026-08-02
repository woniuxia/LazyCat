---
name: sol-luna-delivery
description: 使用 GPT-5.6 Sol xhigh 负责复杂思考、任务设计和最终审查，并把边界明确且耗时的实现或大量简单只读分析交给 GPT-5.6 Luna。用户提到 Sol/Luna 分工、子代理执行、模型额度、token 限额、已确认计划的委派执行，或任务适合复杂规划后批量落地时，都应使用本 skill。
compatibility: Requires Codex CLI with gpt-5.6-sol, gpt-5.6-luna, xhigh reasoning, and PowerShell.
---

# Sol-Luna Delivery

用 Sol 保存复杂推理额度和最终决策质量，用 Luna 承担边界明确但工作量较大的执行。Git 工作区是唯一事实源；任务包和执行摘要只用于交接，不能替代真实 diff 和验证结果。

## 固定角色

- Sol 主代理始终使用 `gpt-5.6-sol` 和 `xhigh`，负责调研、歧义处理、方案、任务包、风险判断和最终审查。
- Luna 执行代理固定使用 `gpt-5.6-luna`，默认 `xhigh`。只有任务满足降级条件时，Sol 才能在派发前显式选择 `high`、`medium` 或 `low`。
- Luna 不做产品、架构、数据模型、兼容性或安全决策。执行中遇到新决策点时返回 `blocked`。
- Luna 不提交、不回滚用户改动、不启动产品 UI、不打包或发布，除非任务包逐项明确授权。
- 同一工作区同时只能有一个写入型 Luna。独立的只读分析可以并行，但必须限制并发并由 Sol 综合。
- 当前提示已经声明自己是 Luna 执行代理并包含冻结任务包时，直接执行任务包，不再调用本 skill 的包装脚本或创建下级执行代理。

如果当前运行环境不能选择 Luna 或不接受指定 reasoning effort，显式报告阻塞。不要回退到 Terra、Sol 或默认模型。

仓库根目录的 `.codex/config.toml` 固定主会话为 Sol `xhigh`。Luna 包装脚本通过命令行参数覆盖该项目默认值；不要删除这两个覆盖参数。

## 路由任务

先判断是否需要复杂思考，再判断委派收益：

| 任务                                       | 执行者 | Reasoning effort      |
| ------------------------------------------ | ------ | --------------------- |
| 需求取舍、架构设计、复杂根因、跨模块不变量 | Sol    | `xhigh`               |
| 最终审查、风险判断、验收裁决               | Sol    | `xhigh`               |
| 方案完整、边界明确且执行耗时               | Luna   | 默认 `xhigh`          |
| 明确但存在少量局部分支                     | Luna   | `high`                |
| 批量分析、常规多文件修改                   | Luna   | `medium`              |
| 机械检索、统计、格式调整、指定命令         | Luna   | `low`                 |
| 一两个命令即可完成的小任务                 | Sol    | `xhigh`，避免交接开销 |

只有以下条件全部满足时才降低 Luna 的 effort：

- 没有需求或产品歧义；
- 不涉及架构、数据模型、兼容性、安全或高风险操作；
- 输入、步骤、输出和验收标准均已确定；
- 错误能被测试或确定性规则发现；
- 失败可以安全停止且没有难恢复的副作用。

任务使用低于 `xhigh` 的 effort 时，在调用脚本时提供具体 `DowngradeReason`。不要让 Luna 自行改变 effort，也不要在失败后自动提高 effort 重跑。

## 冻结任务包

委派前读取 [references/task-template.md](references/task-template.md)，生成一个自包含但紧凑的任务包。完整意味着执行代理不需要补需求；紧凑意味着引用本地文件，不复制完整对话或大段源码。

任务包必须说明：

- `read-only-analysis` 或 `implementation`；
- 目标、上下文和已确认决策；
- 非目标、允许范围和禁止范围；
- `Allowed Changed Paths` JSON 数组；实施任务逐项列出允许修改的仓库相对文件，只读任务固定为 `[]`；
- 可直接执行的步骤及优先级；
- 验收标准和需要实际运行的验证；
- 遇到歧义、冲突、失败或范围变化时的停止条件；
- 任务特有的输出证据。

脚本会检查所有必需章节非空，并校验任务类型。缺少影响结果的信息时由 Sol 补齐，不要把判断责任转嫁给 Luna。

## 执行 Luna

将任务包放在仓库外的临时目录，并为结果、事件日志和 stderr 日志使用新的文件名。调用：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .codex\skills\sol-luna-delivery\scripts\invoke-luna-executor.ps1 -TaskPath <task.md> -TaskType implementation -ReasoningEffort xhigh -ResultPath <result.json>
```

只读分析使用 `-TaskType read-only-analysis`。降级示例：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .codex\skills\sol-luna-delivery\scripts\invoke-luna-executor.ps1 -TaskPath <task.md> -TaskType read-only-analysis -ReasoningEffort low -DowngradeReason "Exact reference inventory with deterministic output" -ResultPath <result.json>
```

脚本负责：

- 固定模型为 `gpt-5.6-luna`；
- 把 reasoning effort 显式传给 Codex；
- 分析任务使用 `read-only`，实施任务使用 `workspace-write`；
- 通过 [references/executor-result.schema.json](references/executor-result.schema.json) 约束最终结果；
- 默认 3600 秒超时并在超时后终止执行进程树；
- 实时保存 Codex JSONL 事件并单独保存 stderr，供进度、失败和 token usage 审计；
- 为同一仓库的实施任务持有非阻塞写入锁；
- 校验 Luna 没有创建提交；
- 对比执行前后的 Git 可见文件状态，校验只读任务没有改动，并校验实施任务的真实改动、路径白名单和 `changedFiles` 一致；
- 对非零退出码、无效结果和角色不一致显式失败。

## 审查结果

Luna 完成后，Sol 必须以仓库事实审查：

1. 读取结果 JSON、事件日志、当前 `git status` 和完整 diff。
2. 对只读分析抽查关键证据，不机械重复全部扫描。
3. 对实施任务检查范围、失败路径、旧行为、测试和用户可见结果。
4. 复跑风险最高且最相关的验证，不能把 Luna 的运行声明当成证据。
5. 审查有明确问题时生成增量任务包，再交给 Luna；机械失败最多重试一次。
6. 出现新设计问题、连续失败或范围变化时收回 Sol，不继续提高 Luna effort。
7. 最终通过后由 Sol 运行 `git diff --check`，只暂存任务文件并按仓库规则提交。

## Token 纪律

- 不传完整线程历史；只传冻结结论、必要路径和验收标准。
- 对重复结果分组汇总，同时保留关键文件和行号证据。
- 输出使用结构化 Schema，避免过程叙述。
- 事件中存在 usage 数据时记录真实值；没有时只记录请求模型、effort、耗时和退出码，不估算 token。
- 委派主要用于把明确执行从 Sol 额度转移到 Luna 额度，不宣称一定降低总 token。
