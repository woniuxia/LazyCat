# 结构治理路线图首批实施计划（批次 0-2）

- 日期：2026-07-04
- 依据 spec：`docs/superpowers/specs/2026-07-04-structure-refactor-roadmap-design.md`（三轮评审通过）
- 影响范围：批次 0/1 仅 `apps/desktop` 前端；批次 2 仅 `src-tauri/src/tools/api_workbench*`
- 执行约定：**行为保持机械拆分**——不改任何行为、接口语义、文案、样式效果；每阶段验证通过即提交；拆分中发现的改良点只记 `process.md` 不顺手做；每阶段开工前 `git status` 确认工作区干净

## 总览

| 阶段 | 内容 | 提交数 | 核心验收 |
|------|------|--------|----------|
| 0 | App.vue Tauri 环境守卫，恢复 e2e 冒烟 | 1 | `pnpm test:e2e` 通过 |
| 1a | 18 个 Pm* 组件迁入 `components/pm/` | 1 | typecheck + build:web + test + e2e |
| 1b | PmPanel.vue 拆分（3 composable + 2 组件） | 5 | 同上 + 行为清单手工冒烟 |
| 2 | api_workbench.rs 目录化拆分 | 约 6 | `cargo test` 用例数前后一致 + 手工冒烟 |
| 3 | process.md 经验沉淀 | 1 | — |

前端路径均相对 `apps/desktop/src/`，Rust 路径相对 `apps/desktop/src-tauri/src/`。

## 阶段 0：App.vue Tauri 环境守卫

### 0.1 现状（已核实）

- `App.vue:97`：`const appWindow = getCurrentWindow();`（setup 顶层，纯 web 环境抛 TypeError，App 不挂载，e2e 两个冒烟用例全挂）。
- 唯一运行时调用点：`App.vue:375` hotkey-navigate 监听器内 `await appWindow.hide()`（外层已有 try/catch）。
- vault 失焦锁定在 `VaultPanel.vue`（`listen("tauri://blur")`），不经 App.vue，本阶段不动。

### 0.2 改法

1. App.vue 内新增探测（写法参照 `rich/extensions.ts:15` 访问 `__TAURI_INTERNALS__` 的先例）：
   `const isTauriEnv = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;`
2. 97 行改为守卫获取：`const appWindow = isTauriEnv ? getCurrentWindow() : null;`
3. 375 行调用点改为 `await appWindow?.hide();`（保持 hide 后 `return` 的原语义；非 Tauri 环境该事件本不会触发，可选链仅兜底）。

不抽公共 util、不动 rich/extensions.ts（机械最小改动；统一 isTauri 工具属改良点，记 process.md）。

### 0.3 验证与提交

1. `pnpm test:e2e`（`apps/desktop/e2e/smoke.spec.ts` 两用例应过）。若出现 webServer 冷启动 `net::ERR_ABORTED`：先手动起 vite 再跑（playwright 配置 `reuseExistingServer: true`）确认主因已修；ERR_ABORTED 单独记录，不混入本批。
2. `pnpm typecheck`
3. 桌面态回归（需运行桌面应用，与用户协调，不自动起 dev server）：主呼出快捷键二次触发隐藏窗口正常；vault 失焦锁定正常。

**提交**：`fix(app): Tauri 窗口 API 增加环境守卫，恢复 e2e 冒烟`

## 阶段 1a：PM 域目录搬迁

### 1a.1 `git mv` 清单（18 个，全部 → `components/pm/`）

PmCalendarView / PmContextMenu / PmDetailPanel / PmGanttView / PmImportDialog / PmItemDialog / PmKanbanView / PmListView / PmMatrixQuadrant / PmMatrixView / PmPanel / PmProjectDialog / PmSettingsDrawer / PmSiyuanDrawer / PmTodayCard / PmTodaySection / PmTodayView / PmViewSwitcher（各 `.vue`）

`InlinePmSelector.vue` / `InlineTodoList.vue` 是跨域桥组件，**留在根目录**（spec 6 节决策）。

### 1a.2 引用更新（外部引用已核实仅两处）

- `tool-registry.ts:60`：`./components/PmPanel.vue` → `./components/pm/PmPanel.vue`
- `composables/pmViewRegistry.ts`：6 处 `../components/PmXxxView.vue` → `../components/pm/PmXxxView.vue`（17/23/29/35/41/47 行）
- 移动集合**内部**的 `./Pm*` 相对引用随整体平移继续有效，不用改。
- 移动文件对**集合外**的相对引用统一加一级：`./InlineTodoList.vue` → `../InlineTodoList.vue`、`./common/...` → `../common/...`、`../composables|utils|types|bridge|rich/...` → `../../...`。以 typecheck + build:web 全量捕获漏网。
- `components.d.ts` 自动生成，不手改。

### 1a.3 验证与提交

`pnpm typecheck` → `pnpm --filter @lazycat/desktop build:web` → `pnpm test` → `pnpm test:e2e`

**提交**：`refactor(pm): PM 域组件迁入 components/pm 子目录`

## 阶段 1b：PmPanel.vue 拆分（2643 行 → ≤ 1200 行软目标）

五个抽取物**各自独立提交**，每次提交前跑 typecheck + build:web。抽取顺序从耦合最小开始；行号为 2026-07-04 版锚点，实施时以当时代码复核。

### 1b.1 `composables/usePmContextMenu.ts`

- 搬移 script 约 1093-1207 行：`ctxMenuVisible/X/Y/ctxMenuActions`、`buildItemContextActions`、`openItemContextMenuAt`、`closeCtxMenu`。
- 输入：`findNextStatus`、`advanceItemStatusFor`、`toggleItemPinFor`、`deleteItemRecord` 等操作回调；输出：菜单状态 + open/close/build。
- **提交**：`refactor(pm): 抽取 usePmContextMenu`

### 1b.2 `composables/usePmItemFilters.ts`

- 搬移搜索防抖（`searchInput` → `searchText`）与 `baseFilteredItems` / `statusFilteredItems` 两层派生（script 约 413-650 内筛选簇）。
- 筛选条件 ref（`filterType/filterPriority/selectedStatuses` 等）**留在壳层**（模板工具栏绑定），composable 只接收 refs、返回派生 computed；分层筛选口径不变（spec/项目 05.5 分层筛选约定）。
- debounce 计时器在 composable 内用 `onScopeDispose` 清理，行为与现状一致。
- **提交**：`refactor(pm): 抽取 usePmItemFilters`

### 1b.3 `composables/usePmItemActions.ts`

- 搬移 script 约 825-1091 行：`submitItem`、`editItem`、`deleteItem(Record)`、`advanceItemStatusFor`、`toggleItemPinFor`、乐观更新与回滚。
- 输入：`items`、`editingItem`、对话框状态 refs、`loadItems`/`loadTodayCounts` 等刷新回调；`ElMessage/ElMessageBox` 调用随函数原样搬移。
- **提交**：`refactor(pm): 抽取 usePmItemActions`

### 1b.4 `components/pm/PmSidebar.vue`

- 搬移模板 6-86 行（今日入口/总览/项目列表/底栏）及仅侧栏使用的局部状态。
- props：`projects`、`projectItemCounts`、`selectedProjectId`、`todayBadgeCount`、`dropTargetProjectId`（拖拽放置高亮）；emits：`select-project`、`show-today`、`open-settings`、`create-project`、拖拽 drop 事件。
- 拖拽状态本体（`draggingItemId`/`dropTargetProjectId`）留壳层，props down / events up（spec 6 节约束）。
- **提交**：`refactor(pm): 抽取 PmSidebar 组件`

### 1b.5 `components/pm/PmToolbar.vue`

- 搬移模板 90-160 行（`pm-toolbar` 外层：项目信息/视图切换/创建/搜索/筛选）。
- props：`selectedProject`、`viewId`、`searchInput`、`filterType`、`filterPriority`、`selectedStatuses`；emits：对应 `update:*` 与 `create-item`（受控组件模式，项目 05.5）。
- **提交**：`refactor(pm): 抽取 PmToolbar 组件`

### 1b.6 行为清单手工冒烟（1b 全部完成后整体执行）

六视图切换与记忆、工作项创建/编辑/删除、状态推进与置顶、看板跨项目拖拽到侧栏、右键菜单全部动作、搜索与三类筛选联动、今日 badge 刷新、思源抽屉、详情面板开合与点击外部关闭。发现任何行为差异：停下修复或 revert 对应提交。

## 阶段 2：api_workbench.rs 目录化拆分

对账基准（已三轮核实）：33 个 action（`is_supported_api_workbench_action`，原 3086 行）；match 分发点原 3131 行；`#[cfg(test)] mod tests` 原 3171 行至文件尾约 2080 行。

### 2.1 迁移前基线

`cargo test api_workbench -- --list`（在 `apps/desktop/src-tauri` 下）记录用例清单与数量，作为全程对账基线。

### 2.2 文件转目录（纯移动，独立提交）

`git mv tools/api_workbench.rs tools/api_workbench/mod.rs`。`tools/mod.rs` 的 `pub mod api_workbench;` 对目录模块同样有效，零改动。验证 `cargo check`。

**提交**：`refactor(api-workbench): 模块转为目录形态`

### 2.3 逐模块抽取（每步 `cargo test` 通过后提交）

按 spec 第 7 节结构与顺序：

1. `types.rs`（KeyValueRow/RequestDraft/ExecutedRequestSnapshot/ResponseBodyPayload 等共享结构体）+ `helpers.rs`（模板解析/URL 构建/HTTP 工具/编码/常量，`pub(crate)`）。
   **提交**：`refactor(api-workbench): 抽取共享 types 与 helpers`
2. `response.rs`（cache_open/cache_reveal/preview_office + 缓存文件管理 + Office 预览）。**cfg 函数对随本步搬移**：`get_api_workbench_response_cache_dir` 的 `#[cfg(test)]` / `#[cfg(not(test))]` 两个版本（原 1363-1378 行）连同文件头 `#[cfg(not(test))] use super::helpers::get_data_dir;` 导入一并迁入，拆散会导致测试构建失败或测试写入真实数据目录。
   **提交**：`refactor(api-workbench): 抽取 response 子模块`
3. `executor.rs`（send 执行链）+ `export.rs`（curl/markdown 导出）。
   **提交**：`refactor(api-workbench): 抽取 executor 与 export 子模块`
4. `collection.rs` / `folder.rs` / `request.rs` / `environment.rs`（含全局变量）。
   **提交**：`refactor(api-workbench): 抽取 CRUD 域子模块`
5. `history.rs`（历史 6 action + `cleanup_unreferenced_history_cache_files`）。
   **提交**：`refactor(api-workbench): 抽取 history 子模块`

每步同时把对应的内嵌测试从 `mod tests` 搬到该子模块自己的 `#[cfg(test)] mod tests`。`list`（`action_list_with_conn`，跨三表聚合树查询）与 action 分发 match、`is_supported_api_workbench_action`、`ensure_api_workbench_history_columns` 留在 `mod.rs`。

### 2.4 收尾对账与验收

1. `cargo test api_workbench -- --list` 用例数与 2.1 基线一致；`cargo test` 全量通过。
2. `pnpm typecheck` + `pnpm test:e2e`（无前端改动，跑基线确认无误伤）。
3. 接口调试面板手工冒烟：发送请求、历史列表/重放、环境切换与变量、导出 curl/markdown、响应缓存打开/定位、Office 预览。
4. Windows 注意：若 `cargo` 报文件锁，先结束运行中的 lazycat 进程（项目 01.2）。

## 阶段 3：经验沉淀

- 按项目 07.3 在 `process.md` 记录：Vue 壳层拆分步骤（composable 先行、组件后行、行为清单冒烟）与 Rust 目录化步骤（转目录 → types/helpers → 按域抽取 → 用例数对账），供批次 3+ 复用。
- 核对 spec 完成定义：PmPanel 行数（`wc -l`）、api_workbench 各子模块行数。
- **提交**：`docs(process): 记录结构治理批次 0-2 拆分经验`

## 风险与注意

- **每阶段开工前** `git status` 确认干净；一个阶段未验收不开下一阶段。
- 1b 每个抽取物提交后如发现行为差异，优先 `git revert` 该提交再重做，不带病前进。
- 阶段 2 每步抽取保持函数体零改动（仅移动 + 可见性调整 + `use` 路径），diff 应呈现"删一块/加一块"形态；出现逻辑改动即违反机械拆分纪律。
- 手工冒烟需运行应用：与用户协调时机，不自动启动 dev server（项目 07.1）。
- 批次 3（Todo 域）不在本计划内：启动时按届时代码出接缝清单（spec 第 8 节）。
