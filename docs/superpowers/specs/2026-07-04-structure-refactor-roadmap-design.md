# 结构治理路线图设计（Structure Refactor Roadmap）

- 日期：2026-07-04
- 状态：设计定稿
- 范围：热点大文件行为保持拆分、e2e 安全网恢复、components 目录渐进分域

## 1. 背景与目标

本路线图是增量结构治理，不改变行为、接口语义、UI 表现或数据格式。

目标：

1. 恢复 e2e 冒烟，为后续结构调整提供回归保护。
2. 将高频变更的大型前端面板拆到可维护规模。
3. 随批次把 components 目录按业务域渐进整理。
4. 每批独立验收、独立提交，可单独回退。

非目标：

- 不重构状态管理。
- 不在机械拆分中顺手修改业务行为。
- 不做一次性全量目录搬迁。
- 不为满足行数指标制造无职责边界的文件。

## 2. 全局纪律

1. 行为保持：拆分中发现的改良点只记录，不混入当前批次。
2. 验证基线：每批执行 `pnpm typecheck`、`pnpm --filter @lazycat/desktop build:web`、相关单测和 e2e 冒烟；Rust 批次额外执行相关 `cargo test`。
3. 行为清单先行：拆分前列出交互清单，拆分后逐项冒烟。
4. 目录规则：新子组件放 `components/<域>/`；拆哪个域就迁移该域既有组件和 colocated 测试。
5. 共享状态保留在壳层，优先 props down / events up，不引入新的全局状态模式。
6. 每批开工前确认工作区边界，不与无关修改混批。

## 3. 批次路线图

| 批次 | 内容 | 前置条件 | 规模 |
|---|---|---|---|
| 0 | App.vue Tauri 环境守卫，恢复 e2e 冒烟 | 无 | 小 |
| 1 | PM 前端域迁入 `components/pm/` 并拆分 `PmPanel.vue` | 批次 0 | 中大 |
| 后续 | Todo 域及其他热点文件按热度和规模逐批评估 | 对应功能计划已稳定 | 按批次确定 |

## 4. 批次 0：恢复 e2e 安全网

- 使用 `'__TAURI_INTERNALS__' in window` 判断 Tauri 环境。
- `getCurrentWindow()` 从 setup 顶层改为守卫后的惰性获取。
- 非 Tauri 环境下窗口操作降级为 no-op。
- 验证：`pnpm test:e2e`，并确认桌面态主呼出快捷键和 Vault 失焦行为不受影响。

## 5. 批次 1：PM 前端域

### 目录搬迁

- 将 18 个 `Pm*` 文件迁入 `components/pm/`。
- 更新 `tool-registry.ts`、`composables/pmViewRegistry.ts` 和静态引用。
- `InlinePmSelector`、`InlineTodoList` 作为跨域桥组件保留在根目录。

### PmPanel 拆分

| 抽取物 | 职责 |
|---|---|
| `pm/PmSidebar.vue` | 项目列表、今日、总览和底栏 |
| `pm/PmToolbar.vue` | 视图切换、搜索、筛选和创建 |
| `usePmItemFilters.ts` | 搜索防抖与分层筛选 |
| `usePmItemActions.ts` | 工作项 CRUD、状态推进和乐观更新回滚 |
| `usePmContextMenu.ts` | 右键菜单状态与动作构建 |

约束：

- `selectedProjectId`、`items`、`selectedItemId`、拖拽状态和刷新信号继续留在壳层。
- 视图容器、对话框和抽屉编排继续留在壳层。
- 侧栏抽取必须完整透传跨项目拖拽状态。

验证：typecheck、build:web、e2e、相关测试，以及六视图切换、工作项 CRUD、跨项目拖拽、右键菜单、思源抽屉和今日 badge 冒烟。

## 6. 后续批次机制

每完成一批，按近三个月变更次数和文件规模重新排序候选池。初始候选包括 Todo、数据字典、API Mock、Inbox、Network、PmListView 和 Vault 等仍存在的热点模块。

数据字典结构调整必须遵守项目规范中的派生索引、sort_key 和字段值索引不变量，并执行 `cargo test data_dictionary -- --nocapture`。

## 7. 风险与回退

- 每批独立提交，使用 `git revert` 可单批回退。
- 状态或事件漏传由行为清单和手工冒烟兜底。
- import 断链由 typecheck 和 build:web 捕获。
- 不在结构治理批次中混入产品功能变更。

## 8. 完成定义

1. e2e 冒烟恢复并在后续批次保持常绿。
2. `components/pm/`、`components/todo/` 等已实施业务域形成清晰目录边界。
3. Vue 壳层拆分与 Rust 目录化的通用经验沉淀到 `process.md`。
4. 每批均有独立验证记录和可回退提交。
