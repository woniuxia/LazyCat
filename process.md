# Process Log

本文件是经验库总索引，不再承载经验正文。经验正文位于 [docs/experience/](docs/experience/README.md)。

## 使用顺序

1. 先读根目录 `AGENTS.md` 的核心规则和开工闸门；Claude Code 通过 `CLAUDE.md` 自动导入同一份规则。
2. 按任务类型进入下方领域文件。
3. 使用 `rg -n "关键词" docs/experience` 查找具体经验。
4. 经验与当前代码冲突时，以当前代码、脚本、配置和测试为准；较新的已验证记录覆盖旧记录。

## 领域索引

| 任务 / 关键词 | 经验文件 |
|---|---|
| 产品边界、架构、IPC、Tauri、capabilities、SQLite 迁移、结构治理、删除功能 | [architecture.md](docs/experience/architecture.md) |
| UI、响应式、滚动、scoped CSS、Element Plus、Teleport、Dropdown、弹层 | [ui-and-styling.md](docs/experience/ui-and-styling.md) |
| 数据字典、JSON、FTS、sort_key、关系、导入 | [data-dictionary.md](docs/experience/data-dictionary.md) |
| Todo、提醒、eventAt、displayAt、逾期、周期系列 | [todo.md](docs/experience/todo.md) |
| PM、甘特、看板、状态筛选、思源 | [pm.md](docs/experience/pm.md) |
| Spotlight、快捷键、浏览器身份、provider | [spotlight-and-launcher.md](docs/experience/spotlight-and-launcher.md) |
| API Mock、访问链路、Cron、连通性 | [api-and-network-tools.md](docs/experience/api-and-network-tools.md) |
| 请求转发、预检、运行态、日志、恢复 | [request-forward.md](docs/experience/request-forward.md) |
| 上线包、归档、并行目标、终态 | [release-package.md](docs/experience/release-package.md) |
| Windows、portable、NSIS、WebView2、Release | [windows-build-and-release.md](docs/experience/windows-build-and-release.md) |
| Vault、Inbox、剪贴板采集 | [vault-and-inbox.md](docs/experience/vault-and-inbox.md) |
| 离线手册、资源、本地预览、壁纸 | [manuals-and-resources.md](docs/experience/manuals-and-resources.md) |
| Agent 文档、只读边界、计划、续作、交接、主动提交、验证、dirty worktree | [agent-workflow.md](docs/experience/agent-workflow.md) |
| JSON 树、Base64、片段、番茄钟、SQL 生成 | [other-tools.md](docs/experience/other-tools.md) |

完整迁移对账见 [docs/experience/README.md](docs/experience/README.md)。

## 经验维护规则

- 新经验直接写入对应领域文件顶部，并更新该文件目录；不要重新追加到本文件。
- 新条目保留日期、场景、问题、解决、关键点、涉及文件、验证和使用次数；没有实际内容的字段省略。
- 使用次数初始为 0；后续复用时 +1 并追加引用日期。
- 使用次数达到 3 次后，只评估是否固化到根规范；只有高频、稳定且会改变 agent 决策的内容才固化。
- 过期、重复或被新实现替代的经验应合并或删除，并在迁移审计中写明依据。
- 未运行的命令不得写成已验证；历史记录中的验证结果保持原样，不向当前状态做无依据延伸。

## 新经验模板

```markdown
## YYYY-MM-DD：标题

**场景**：...

**问题**：...

**解决**：...

**关键点**：...

**涉及文件**：
- `path/to/file`

**验证**：
- `command`

**使用次数**：0
```
