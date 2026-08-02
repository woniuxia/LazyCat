---
name: sol-luna-delivery
description: 使用 GPT-5.6 Sol xhigh 负责复杂思考、复杂实现、可执行详细设计和最终审查；固定由 Sol 先研究、决策并冻结可直接执行的详细设计和任务包，再把简单、低风险且可确定性验收的有界执行交给 GPT-5.6 Luna，最后由 Sol 验收。用户提到 Sol/Luna 分工、子代理执行、模型额度、token 限额或已确认简单任务的委派执行时，都应使用本 skill。
---

# Sol-Luna Delivery

流程固定为 `Sol 研究与详细设计 -> 冻结设计和任务包 -> Luna 执行 -> Sol 审查验收`。Sol 先完成研究、决策和可直接执行的详细设计并冻结任务包，Luna 再承担简单、边界冻结且可确定性验收的执行。目标是按能力路由并降低 Sol 等待期间的 token 消耗，不以提高 Luna 委派率或降低原始 token 总量为目标。Git 工作区是唯一事实源；任务包和执行摘要只用于交接，不能替代真实 diff 和验证结果。

## 固定角色

- Sol 主代理始终使用 `gpt-5.6-sol` 和 `xhigh`，负责调研、歧义处理、方案、可直接执行的详细设计、任务包、复杂实现、风险判断和最终审查；Sol 是设计负责人。
- Luna 执行代理固定使用 `gpt-5.6-luna`，默认 `xhigh`。只有任务满足降级条件时，Sol 才能在派发前显式选择 `high`、`medium` 或 `low`。
- Luna 不处理复杂根因、跨模块不变量、并发、事务、兼容性、安全、解析器或文件系统语义，也不做产品、架构、数据模型和实现方案决策。Luna 必须按 Sol 冻结的详细设计执行，不补齐设计空白；执行中遇到设计与仓库事实不一致或任何新决策点时返回 `blocked`。
- Luna 不提交、不回滚用户改动、不启动产品 UI、不打包或发布，除非任务包逐项明确授权。
- 同一工作区同时只能有一个写入型 Luna。独立的只读分析可以并行，但必须限制并发并由 Sol 综合。
- 当前提示已经声明自己是 Luna 执行代理并包含冻结任务包时，直接执行任务包，不再调用本 skill 的包装脚本或创建下级执行代理。

如果当前运行环境不能选择 Luna 或不接受指定 reasoning effort，显式报告阻塞。不要回退到 Terra、Sol 或默认模型。

仓库根目录的 `.codex/config.toml` 固定主会话为 Sol `xhigh`。Luna 包装脚本通过命令行参数覆盖该项目默认值；不要删除这两个覆盖参数。

## 路由任务

先判断任务是否简单且已冻结、且 Sol 是否已完成可直接执行的详细设计，再判断委派收益。难以判断 Sol/Luna 时选 Sol；难以判断 Luna effort 时选 `high`。不得用 Luna `xhigh` 承接本应属于 Sol 的复杂任务。

每次选择 Luna 时，路由结论必须显式声明 Sol 保留真实 diff 检查、关键验证复跑、最终审查、验收裁决和提交责任；不能只在通用规则中隐含这一点。

| 任务                                                         | 执行者 | Reasoning effort      |
| ------------------------------------------------------------ | ------ | --------------------- |
| 需求取舍、架构设计、复杂根因、跨模块不变量                   | Sol    | `xhigh`               |
| 复杂实现、并发、事务、兼容性、安全、解析器、文件系统语义     | Sol    | `xhigh`               |
| 最终审查、风险判断、验收裁决                                 | Sol    | `xhigh`               |
| 已冻结、低风险、可确定性验收的多文件或耗时简单实施           | Luna   | 默认 `xhigh`          |
| 已冻结的局部简单任务，只有少量明确分支；需阅读源码并归类用途  | Luna   | `high`                |
| 对显式列出的项目执行相同转换，无需理解内容或分类的批量操作    | Luna   | `medium`              |
| 纯计数、格式转换、指定命令，不读取源码内容或推断语义          | Luna   | `low`                 |
| 一两个命令即可完成的小任务                                   | Sol    | `xhigh`，避免交接开销 |

只有以下条件全部满足时才使用 Luna；不满足任一项就由 Sol `xhigh` 处理：

- 没有需求或产品歧义；
- Sol 已完成并冻结可直接执行的详细设计；
- 不涉及架构、数据模型、兼容性、安全或高风险操作；
- 输入、步骤、输出和验收标准均已确定；
- 错误能被测试或确定性规则发现；
- 失败可以安全停止且没有难恢复的副作用。

在 Luna 资格成立后，只有局部任务且分支少而明确时才降到 `high`；代码盘点如果需要追踪封装、理解上下文、归类用途或判断遗漏，也必须至少使用 `high`。只有对显式输入执行相同转换、无需理解内容和分类时才降到 `medium`；只有纯计数、格式转换或指定命令，且不读取源码内容、不推断语义时才降到 `low`。任务使用低于 `xhigh` 的 effort 时，在调用脚本时提供具体 `DowngradeReason`。不要让 Luna 自行改变 effort，也不要在失败后自动提高 effort 重跑。

## Sol 详细设计与冻结任务包

委派前读取 [references/task-template.md](references/task-template.md)。Sol 必须先完成研究和决策，再把可直接执行的详细设计写入一个自包含但紧凑的冻结任务包；完整意味着执行代理不需要补需求或选择方案，紧凑意味着引用本地文件，不复制完整对话或大段源码。

任务包必须说明：

- `read-only-analysis` 或 `implementation`；
- 目标、上下文和已确认决策；
- `Executable Design`：由 Sol 冻结的可直接执行详细设计。`implementation` 必须写明精确目标文件及相关符号/区段、必要改动、适用时的行为/数据/控制流契约、边界/失败/兼容性约束，以及测试改动或验证顺序；`read-only-analysis` 必须写明精确证据源、搜索或检查方法、适用时的分类/去重规则，以及完整性检查。
- 非目标、允许范围和禁止范围；
- `Allowed Changed Paths` JSON 数组；实施任务逐项列出允许修改的仓库相对文件，只读任务固定为 `[]`；不得使用“对应测试”“相关文件”等占位文本，路径未知时由 Sol 先查明再派发；
- 可直接执行的步骤及优先级；`Steps` 只表示依据 `Executable Design` 的执行顺序，不能替代详细设计；仅可把明确点名的机械选择留给 Luna。
- 验收标准和需要实际运行的验证；
- 遇到歧义、冲突、失败或范围变化时的停止条件；
- 任务特有的输出证据。

脚本会检查所有必需章节非空，并校验任务类型。只有写明 `Executable Design` 的冻结任务包才可委派；仅陈述目标、要求 Luna 自行检查仓库并选择方案的模糊任务包不得委派。Luna 发现设计与仓库事实不一致，或实现需要改变/补完设计时，必须返回 `blocked`，由 Sol 收回设计责任。

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
- 默认 3600 秒总硬超时（所有尝试共享），每 60 秒输出心跳，并在超时后终止执行进程树；
- 持久化 Codex session（不使用 `--ephemeral`），实时从 `thread.started.thread_id` 捕获 session ID，并在仓库外保存 state manifest；本阶段不提供包装器重启后的恢复；
- 仅当非零退出且未得到有效结构化结果时才恢复一次，默认等待 15 秒；有 session ID 时只允许 `codex exec resume <SESSION_ID>`，无 session ID 时仅在 HEAD 与工作区相对首轮基线完全未变时允许 fresh initial；
- 超时、有效 `failed`/`blocked` 结果、零退出但无效结果、最终验证失败、越界或 HEAD 变化均不重试；第 2 次尝试使用独立 result/event/stderr 文件，再聚合 event/stderr，并仅在成功时覆盖 canonical result；
- 实时保存 Codex JSONL 事件并单独保存 stderr，供进度、失败和 token usage 审计；
- 运行期间只探测事件文件元数据并限量读取日志头捕获 session，进程结束后才完整解析一次事件；
- state manifest 记录每次 attempt 和整轮的 usage、耗时、命令统计、日志字节数与最后事件时间；
- 成功时只向调用方输出紧凑执行摘要和产物路径，不回显完整 JSONL；
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
- 优先精确读取目标范围，限制单次工具输出；不要反复读取完整大文件或广泛扫描依赖仓库。
- 可独立的只读命令一次批量执行；验证先定向、后综合，完整构建最多在最终阶段运行一次。
- 对重复结果分组汇总，同时保留关键文件和行号证据。
- 输出使用结构化 Schema，避免过程叙述。
- 事件中存在 usage 数据时记录真实值；没有时只记录请求模型、effort、耗时和退出码，不估算 token。
- 委派主要用于把明确执行从 Sol 额度转移到 Luna 额度，不宣称一定降低总 token。
