# 结构治理路线图设计（Structure Refactor Roadmap）

- 日期：2026-07-04
- 状态：设计定稿（三节均经用户逐节确认）
- 范围：热点大文件行为保持拆分 + e2e 安全网恢复 + components 目录渐进分域

## 1. 背景与量化现状

2026-02 已完成一轮六方案重构（App.vue 现 427 行、类型集中、CSS 分层、utils 纯函数层），本次是增量结构治理，不是救火。

当前痛点（2026-07-04 扫描）：

| 类别 | 文件 | 行数 | 近 4 个月变更次数 |
|---|---|---|---|
| 前端 God 面板 | TodoPanel.vue | 2960（快速添加栏落地后） | 39 |
| 前端 God 面板 | PmPanel.vue | 2643 | 42 |
| 前端 God 面板 | VaultPanel.vue | 2542 | 低 |
| 前端 God 面板 | DataDictionaryPanel.vue | 2496 | 12 |
| 前端 God 面板 | InboxPanel.vue / ApiWorkbenchPanel.vue / PmListView.vue / NetworkPanel.vue | 2000-2300 | 中 |
| Rust 巨型模块 | api_workbench.rs | 5249 | 18 |
| Rust 巨型模块 | data_dictionary.rs | 3319 | 中 |
| Rust 巨型模块 | todo.rs | 3222 | 23 |
| Rust 巨型模块 | api_mock.rs | 2594 | 中（近期活跃） |

其他事实：

- e2e 冒烟自 2026-03-07 起必挂：`App.vue:97` 顶层 `const appWindow = getCurrentWindow()` 在纯 web 环境抛错，App 不挂载（引入提交 dada6c8）。所有重构当前无端到端回归保护。
- `components/` 根下约 124 个文件基本平铺（含 colocated `*.test.ts`），但已有分域先例：`api-mock/`、`db/`、`common/`、`settings/`。域聚类明显：Pm* 18 个、Todo* 9 个（含 TodoQuickAddBar）、Api* 6 个、Spotlight* 5 个。
- Rust 侧有成熟拆分先例：pm.rs 拆为 pm / pm_today / pm_calendar / pm_matrix / pm_weekly / pm_siyuan / pm_todo_link 共 7 个模块；子目录先例 `db_drivers/`。

## 2. 目标与非目标

**目标**

1. 把"又大又热"的文件拆到可维护规模，降低后续每次改动的成本与回归风险。
2. 恢复 e2e 冒烟，让后续所有批次有回归保护。
3. components 目录随批次渐进分域（拆哪个域，搬哪个域）。

**非目标**

- 不做任何行为、接口语义、UI 表现、数据格式变更（纯机械拆分）。
- 不重构状态管理、不去重、不补大规模测试（发现的改良点仅记录 process.md）。
- 不做一次性全量目录搬家。
- 不预先详设批次 4+（避免规划腐化）。

## 3. 全局纪律（每批适用）

1. **行为保持**：纯代码搬家。拆分中发现的改良点只记录到 process.md，不顺手做。
2. **每批独立验收、独立提交**：`pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web` + 相关单测；Rust 批次加 `cargo test`；批次 0 完成后，e2e 冒烟纳入之后每一批验收。
3. **行为清单先行**：每批拆前列出必须原样保留的交互点清单，拆后照单手工冒烟。
4. **尺寸软目标**：面板壳层拆后 ≤ 800 行（PmPanel 首批目标 ≤ 1200 行）；子组件/composable 按单一职责，不为凑数硬拆。
5. **目录规则**：新子组件放 `components/<域>/`；拆哪个域就把该域既有文件（含 colocated `*.test.ts`）一并 `git mv` 进去。composables/ 仅 26 个文件，保持平铺。
6. **避让约束**：
   - 每批开工前确认工作区干净，不与进行中改动混批。
   - `ApiWorkbenchPanel.vue` 拆分延后到 API Workbench UX 18 项落地之后（多标签改造会重构该面板）。
   - TodoPanel 拆分排在快速添加栏 plan（`docs/superpowers/plans/2026-07-04-todo-quick-add-plan.md`）实施之后。**更新 2026-07-04：该 plan 已实施完成（提交 21f54c9 / a38ec66 / 0fece60），此前置条件已满足。**

## 4. 批次路线图

| 批次 | 内容 | 前置条件 | 规模 |
|---|---|---|---|
| 0 | App.vue Tauri 环境守卫，恢复 e2e 冒烟 | 无 | 小（1-2 文件） |
| 1 | PM 前端域：`components/pm/` + 拆 PmPanel.vue | 批次 0 | 中大 |
| 2 | api_workbench.rs 目录化拆分 | 无硬依赖，排在 1 后 | 中 |
| 3 | Todo 域：`components/todo/` + 拆 TodoPanel.vue + 拆 todo.rs | 快速添加栏 plan 已实施（2026-07-04 已满足） | 大 |
| 4+ | 候选池逐批评估（见第 9 节） | 按当时热度×规模重排 | — |

## 5. 批次 0 详设：恢复 e2e 安全网

- **改法**：以 `'__TAURI_INTERNALS__' in window` 做 isTauri 探测；`getCurrentWindow()` 从 setup 顶层改为守卫后的惰性获取；无 Tauri 环境时相关调用降级为 no-op。
- **涉及**：App.vue 顶层 `appWindow` 常量（97 行）及其唯一运行时调用点——hotkey-navigate 监听器内的 `appWindow.hide()`（约 375 行，已有 try/catch）。注意：vault 失焦锁定实现在 `VaultPanel.vue`（`listen("tauri://blur", ...)`，约 1478 行），不经由 App.vue 的 `getCurrentWindow()`，本批不动它。
- **验收**：`pnpm test:e2e` 冒烟通过；桌面态回归两项——主呼出快捷键二次触发隐藏窗口正常、vault 失焦锁定正常（后者为 dada6c8 引入该行的历史动机，回归以确认无意外耦合）。
- **注记**：playwright webServer 冷启动 `ERR_ABORTED` 为已知次要现象，修复主因后若仍复现再单独排查，不混入本批。

## 6. 批次 1 详设：PM 前端域（两个提交）

### 提交 1a：目录搬迁

- `git mv` 18 个 `Pm*` 文件至 `components/pm/`；同步更新两个动态 import 枢纽——`tool-registry.ts` 与 `composables/pmViewRegistry.ts`（6 个 `../components/Pm*View.vue` 的 `defineAsyncComponent`）——及其他静态引用点。
- 跨域桥组件 `InlinePmSelector` / `InlineTodoList` 留在根目录（被 Todo/PM 双侧消费，归属不清不硬归类）。
- `components.d.ts` 为自动生成，无需手改。
- 验收：typecheck + build:web + e2e 冒烟。

### 提交 1b：PmPanel.vue 拆分（2643 行 → ≤ 1200 行软目标）

| 抽取物 | 内容 | 来源（2026-07-04 版行号） |
|---|---|---|
| `pm/PmSidebar.vue` | 左侧栏（项目列表/今日/总览/底栏） | 模板 6-86 |
| `pm/PmToolbar.vue` | 工具栏（视图切换/搜索/筛选/创建） | 模板 90-160（`pm-toolbar` 外层） |
| `composables/usePmItemFilters.ts` | 搜索防抖 + baseFilteredItems / statusFilteredItems 两层筛选 | script 筛选簇（约 413-650） |
| `composables/usePmItemActions.ts` | 工作项 CRUD、状态推进、置顶、乐观更新回滚 | script 约 825-1091 |
| `composables/usePmContextMenu.ts` | 右键菜单状态与动作构建 | script 约 1093-1207 |

**约束**

- 跨块共享状态（`selectedProjectId`、`items`、`selectedItemId`、拖拽状态 `draggingItemId`/`dropTargetProjectId`、`todayRefreshSignal`）保留在壳层，props down / events up，不引入 provide/inject 新模式。
- 视图容器 v-if 链、对话框/抽屉编排保留在壳层。
- PmSidebar 需透传拖拽放置状态（`dropTargetProjectId`），这是侧栏抽取的主要风险点。

**验收**：typecheck + build:web + e2e 冒烟 + 手工冒烟清单（六视图切换、工作项 CRUD、跨项目拖拽、右键菜单、思源抽屉、今日 badge 刷新）。PM 域无现成组件单测，本批不补测（机械拆分约束），靠四重验收兜底。

## 7. 批次 2 详设：api_workbench.rs 模块化

### 目标结构（复用 db_drivers/ 子目录先例）

```text
tools/api_workbench/
  mod.rs        execute 入口 + action 分发（签名不变，tools/mod.rs 零改动）
  types.rs      KeyValueRow / RequestDraft / ExecutedRequestSnapshot / ResponseBodyPayload 等
  helpers.rs    模板解析 / URL 构建 / HTTP 工具 / 编码 / 常量（pub(crate)）
  collection.rs 集合 4 action
  folder.rs     文件夹 5 action
  request.rs    请求 5 action + request_save_example_response
  environment.rs 环境 3 action + 全局变量 2 action
  executor.rs   send 执行链（send_with_conn / prepare / execute_http_request）
  history.rs    历史 6 action + 缓存清理（cleanup_unreferenced_history_cache_files）
  export.rs     export_curl / export_markdown + 渲染函数
  response.rs   响应缓存 2 action（cache_open / cache_reveal）+ response_preview_office
```

共 **33 个 action**（以 `is_supported_api_workbench_action` 为对账基准）：32 个按上表迁移进子模块，`list` 保留在 mod.rs；与现有 match 分发点（原文件 3131 行起）一一对应。

原文件含两个 `#[cfg(test)]` 模块（1363 行、3171 行至文件尾，合计约 2400 行、占全文件约 40%）：**内嵌测试随被测函数迁移至对应子模块**，迁移前后 `cargo test` 用例数必须一致。

### 实施顺序

先抽 types / helpers / response（边界最清晰）→ executor / export → 各域 CRUD；一批内小步多提交。

### 风险与对策

- 共享私有函数（模板解析、URL 构建、HTTP 工具）跨 executor/request/export：统一收入 helpers.rs，`pub(crate)`。
- `_with_conn` 模式与 `Result<Value, String>` 错误形态保持不变。
- 历史缓存清理需感知全表引用：保留在 history.rs。

**验收**：`cargo test`（api_workbench 内嵌测试迁移后全部保持通过，用例数与迁移前一致）+ 接口调试面板手工冒烟（发送/历史/环境/导出/响应预览）+ typecheck + e2e。

**协调注记**：后续实施 UX 18 项 plan 时，plan 中指向 `api_workbench.rs` 的路径引用按本节新结构对号入座。

## 8. 批次 3 概要：Todo 域

前提：快速添加栏 plan 已实施——**2026-07-04 已满足**（提交 21f54c9 / a38ec66 / 0fece60），TodoPanel 现为 2960 行。**批次启动时按当时代码出行号级接缝清单**，本节只定框架：

- 目录搬迁：9 个 `Todo*` 组件（含 TodoQuickAddBar）及 5 个 colocated 测试文件（TodoPanel.edit-focus / TodoPanel.title-enter / TodoPanel.quick-add / TodoQuickAddBar / TodoDetailView.layout 的 `.test.ts`）一并迁入 `components/todo/`。
- TodoPanel.vue 抽取方向（按 2026-07-04 探索）：编辑器状态机（编辑/创建/dirty 检测）、调度字段（日期/提醒/重复）、PM 关联、筛选分组、CRUD 操作五个 composable。
- 有利条件：TodoPanel 已有 3 个组件测试（edit-focus、title-enter、quick-add），拆分全程必须保持通过。
- todo.rs（3222 行）按批次 2 相同模式目录化拆分，接缝届时探查。

## 9. 批次 4+ 候选池机制

每完成一批，按"近 3 个月变更次数 × 行数"重排候选池，避开当时未实施 spec 的落点。

初始候选池：data_dictionary.rs（3319）、api_mock.rs（2594）、DataDictionaryPanel.vue（2496）、InboxPanel.vue（2260）、NetworkPanel.vue（2050）、PmListView.vue（2177）、VaultPanel.vue（2542，热度低靠后）、ApiWorkbenchPanel.vue（UX 18 项落地后）。

特别注记：data_dictionary 拆分受 CLAUDE.md 04.9 强不变量约束（派生索引、sort_key、字段值索引等），只动代码组织、不碰任何不变量，验收必跑 `cargo test data_dictionary -- --nocapture`。

## 10. 风险与回退

- 每批独立提交，回退粒度 = `git revert` 单批。
- 机械拆分最大风险是搬移时漏传状态/事件造成行为细变：以"行为清单 + 照单手工冒烟"兜底（见第 3 节纪律 3）。
- 目录搬迁 import 断链由 typecheck + build:web 全量捕获。
- 每批开工前确认工作区干净，不与进行中改动混批。

## 11. 完成定义

1. 批次 0-3 完成，e2e 冒烟每批常绿。
2. PmPanel ≤ 1200 行；api_workbench 子模块平均 ≤ 600 行（软目标）；`components/pm/`、`components/todo/` 建立。
3. 拆分模式（Vue 壳层拆分四步、Rust 目录化拆分三步）沉淀进 process.md，供批次 4+ 复用。

## 12. 决策记录

| 决策 | 结论 | 备选与否决理由 |
|---|---|---|
| 收益方向 | 拆热点大文件 + e2e 安全网 + 目录整理 | "为待实施 spec 铺路"未选为主目标，但排序上全程避让 |
| 拆分深度 | 行为保持机械拆分 | 顺手改良被否：无 e2e 保护期风险高、批次周期变长 |
| TodoPanel 顺序 | 快速添加栏 plan 先行 | 拆分先行会作废已三审定稿的 plan；该 plan 已于 2026-07-04 实施完成，前置条件现已满足 |
| 目录策略 | 渐进分域（方案 B） | 一次性大搬家（方案 A）与 3 个待实施 plan 路径引用全面冲突；双线并行（方案 C）回归定位难 |
| Rust 拆分形态 | 目录化 `tools/api_workbench/` | 平铺 `api_workbench_*.rs` 会加剧 tools/ 根下 48 个 .rs 的膨胀；db_drivers/ 已有子目录先例 |
