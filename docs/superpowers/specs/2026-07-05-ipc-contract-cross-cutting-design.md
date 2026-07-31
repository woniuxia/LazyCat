# IPC 契约对账与横切面治理设计（Cross-cutting Governance: X1-X4）

- 日期：2026-07-05
- 状态：设计定稿（四节经用户一次性确认）
- 范围：契约对账安全网 + 事件常量集中 + IPC 错误反馈收敛（试点）+ 列表搜索防抖统一（试点）
- 关系：与《结构治理路线图》（`2026-07-04-structure-refactor-roadmap-design.md`）并行的独立 spec；本 spec 是"补盲区体检"的落地产物，处理路线图明确排除的横切面改良

## 1. 背景与量化现状

路线图覆盖"热点大文件拆分 + 目录分域 + e2e 恢复"，明确不做横切面改良。本次体检确认路线图之外存在四个高收益点：

| 编号 | 问题                             | 证据（2026-07-05 核实）                                                                                                                                                                                                                                                                                           |
| ---- | -------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| X1   | 前后端 action 三份手工清单无对账 | CHANNEL_MAP 约 356 条（`src/bridge/tauri.ts:46-403`）；Rust 40 域分发（`src-tauri/src/tools/mod.rs:62-106`）；挂件刷新白名单（`mod.rs:112-147`）。漂移已实际发生：白名单 todo 域含 4 个幽灵词条 `item_reorder` / `item_batch_update` / `item_complete` / `item_undo_complete`，`todo.rs:85-104` 的 match 中不存在 |
| X2   | 事件名两侧全是散落字符串字面量   | Rust emit：`main.rs` 十余处 + `widget/apply.rs`、`widget/pulse.rs`；前端 emit：`spotlight/providers/hosts.ts:79`（跨窗口）；前端 listen 14 处。盘点时常规 grep 即漏掉 `hotkey-navigate` 的 emit 点（`main.rs:791`，事件名单独成行）                                                                               |
| X3   | IPC 错误反馈三套写法并存         | 约 310 处 `ElMessage.error` 各写各的；`useToolInvoke`（55 行）实际仅 PM 域少数组件采用，其余面板裸调 `invokeToolByChannel` + 手写 try/catch                                                                                                                                                                       |
| X4   | 搜索防抖多种手写变体并存         | 已核实三种代表形态：SnippetPanel setTimeout 手写防抖（`SnippetPanel.vue:291,394`）、LauncherPanel 300ms watch+timer（`LauncherPanel.vue:236-239`）、HostsPanel 无防抖纯 computed（`HostsPanel.vue:337-339`）                                                                                                      |

## 2. 目标与非目标

**目标**

1. 三份 action 清单（CHANNEL_MAP、模块 supported_actions、挂件白名单）与两侧事件常量全部有测试对账，漏注册在 `cargo test` 阶段暴露而非运行时。
2. 确立 IPC 错误反馈与列表搜索的规范写法，并在 4 个试点面板落地为样板。

**非目标（体检中评估后明确不做，理由记录第 9 节）**

- 不引入 typeshare / 类型双向生成、Zod / 运行时 schema 校验。
- 不做 DAO / QueryBuilder 层、不做迁移版本化框架、不补外键级联。
- 不统一独立 command（capture / hotkey / reminder 等）与 channel 双体系。
- 不全量改造 310 处错误反馈（仅试点 4 面板，其余随路线图批次顺势收敛）。
- 不动 Rust 参数解析风格（serde 结构体化随路线图批次 2/3 拆分后再评估）。

## 3. 全局纪律

1. **避让路线图**：不碰 PmPanel / TodoPanel / ApiWorkbenchPanel（路线图批次 1、3 与 API UX 18 项的落点）；试点面板固定为 SnippetPanel、LauncherPanel、HostsPanel、DnsPanel（均不在路线图批次与候选池）。
2. **每批独立验收、独立提交、可独立 revert**；开工前工作区干净。
3. X1、X2 为后端 + 测试 + 机械替换，不依赖 e2e；X3、X4 涉及面板行为，建议路线图批次 0（e2e 恢复）先行，未先行则以行为清单手工冒烟兜底。
4. **与路线图 Rust 批次的交叠协调**：X1 会触碰 todo.rs 等路线图待拆文件的 execute 入口（每处十行内的机械插入）。先做 X1，则拆分时 supported_actions 与守卫随新 mod.rs 迁移；先做拆分，则 X1 落到拆后新结构。两个方向均兼容，开工时以当时代码为准，不构成硬依赖。

## 4. X1 详设：契约对账安全网（约 1.5-2 天）

### 4.1 模块侧

- 每个工具模块新增 `pub(crate) fn supported_actions() -> &'static [&'static str]`，`execute` 入口前置守卫：action 不在清单即返回 `Err(format!("{domain}: unsupported action '{action}'"))`。
- 直接推广 `api_mock.rs` 已有的 supported-actions 先例，并统一改成同一形态。
- 原 match 的 `_` 兜底臂保留不动（守卫后不可达，作双保险）。
- pm 子模块（pm_today / pm_calendar / pm_matrix / pm_weekly / pm_siyuan / pm_todo_link）经 pm 域分发，其 action 统一收在 `pm::supported_actions()`。
- `mod.rs` 聚合 `pub fn supported_actions(domain: &str) -> Option<&'static [&'static str]>`，域清单与 `dispatch_tool` 的 match 一一对应。

### 4.2 对账测试

位置：`src-tauri` 内单元测试（crate 为 bin 形态，用 `#[cfg(test)]`，路径取 `env!("CARGO_MANIFEST_DIR")` 拼 `../src/bridge/tauri.ts`）。逐行正则解析 CHANNEL_MAP 条目（形如 `"tool:x:y": { domain: "d", action: "a" }`），断言：

- **方向 A**：每个 (domain, action) 必须出现在 `supported_actions(domain)` 中——防前端调用运行时炸。
- **方向 B**：每个 supported action 必须有对应 channel；例外进显式豁免常量 `EXEMPT: &[(&str, &str)]`，每条附注原因——防死代码沉默堆积。
- **白名单**：`pm_or_todo_data_changed` 从 `matches!` 改为两个 const 数组（PM / TODO 各一），断言均 ⊆ 对应域 supported_actions；顺带删除 4 个幽灵词条（dispatch 本就到不了它们，纯清理、无行为影响）。
- 测试失败信息给出修复指引（"新增 action 需同步：模块 supported_actions、CHANNEL_MAP、（写操作）挂件白名单"）。

### 4.3 配套约定

- `tauri.ts` 文件头加注释：本文件被 src-tauri 对账测试逐行解析，CHANNEL_MAP 保持一行一条目格式。
- capture 域走独立 command 不在 channel 体系，不参与对账。

**验收**：`cargo test`（新对账测试 + 既有全部）+ `pnpm typecheck`。

## 5. X2 详设：事件常量与事件对账（约 1 天）

### 5.1 事件清单（2026-07-05 盘点，实施时以全量 grep `emit` / `emit_to` / `listen` 复核）

主窗口/全局：`main-window-toggle`、`hotkey-navigate`、`clipboard-changed`、`todo-reminder-fired`、`reminder-push`、`pomodoro-state-changed`、`pomodoro-prompt-refresh`、`quick-capture-reset`、`spotlight-reset`、`hosts-applied`（前端→前端跨窗口，Spotlight 发、App.vue 收）；挂件：`widget://color-mode`、`widget://dashboard-data`、`widget://navigate`。`tauri://focus` / `tauri://blur` 为框架内置，不入常量表。

### 5.2 改法

1. 前端新增 `src/bridge/events.ts`：`as const` 事件名常量对象 + 各事件 payload 类型引用（类型仍留在 `types/`，不搬家）；每条注明发端与收端（Rust / 前端窗口）。替换 App.vue、useClipboardSuggestion、InboxPanel、PomodoroPanel、QuickCapture、ReminderPopup、SpotlightPanel、TodoPanel、WidgetCanvas、spotlight providers 等处字面量。
2. Rust 新增 `src-tauri/src/events.rs`：`pub const EVENT_*: &str` 常量，替换 `main.rs` 与 widget 模块字面量。
3. X1 解析式测试延伸一个用例：解析 `events.ts`，断言 **Rust 常量集合 ⊆ 前端常量集合**（前端是超集：含 `hosts-applied` 等纯前端事件，在 events.ts 中标注"仅前端"）。
4. 处置不对称事件：`pomodoro-prompt-refresh` 有 emit、未见 listen——实施时确认，确属死事件则删除 emit（行为无关清理）；其余 listen 无 emit 的情况同理逐一确认。

**验收**：`cargo test` + `pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web` + 手工冒烟四项：Todo 提醒弹窗、剪贴板建议、挂件导航、Spotlight 应用 hosts 方案。

## 6. X3 详设：IPC 错误反馈收敛（试点制，约 2 天）

### 6.1 规范（写入本 spec，落 process.md，后续路线图各批次拆分时照做）

1. 用户直接触发的操作失败 → 统一经 `useToolInvoke` 弹错，带操作语境前缀（如"保存失败：..."）。
2. 后台静默操作（轮询、预加载、可降级读取）失败 → 允许静默或 console.warn，但必须留注释说明为何静默。
3. 新代码禁止新增裸 `invokeToolByChannel` + 手写 try/catch。

### 6.2 useToolInvoke 纯增量升级（现有签名不动，PM 域零改动）

- `invokeWithLoading<T>(channel, payload, opts?: { errorPrefix?: string })`：失败弹 `${errorPrefix}${message}`。
- 新增 `invokeSilent<T>(channel, payload): Promise<T | undefined>`：失败返回 undefined，不弹错（配合规范第 2 条的注释要求）。
- 已知改良点本次不做、记 process.md：boolean loading 在并发调用下互相覆盖（计数式 loading 留待后续）。

### 6.3 试点改造

SnippetPanel、LauncherPanel、HostsPanel、DnsPanel 四面板的全部 IPC 调用点收敛到 useToolInvoke 方法；手写 try/catch 除"静默 + 注释"场景外全部移除；错误文案按规范补语境前缀。

**验收**：`pnpm test` + `pnpm typecheck` + `build:web` + 各面板行为清单手工冒烟（列表加载、增删改、失败路径至少各验一条）。

## 7. X4 详设：列表搜索防抖统一（约 1.5 天）

### 7.1 `composables/useListSearch.ts`（修订 2026-07-05：分层双 API，适配本地过滤与后端搜索两种形态）

```ts
// 基础层：关键字 + 防抖（后端搜索面板直接用它，watch debouncedKeyword 触发重查）
function useDebouncedKeyword(options?: { debounceMs?: number }): {
  // 默认 300
  keyword: Ref<string>; // 绑定输入框
  debouncedKeyword: Readonly<Ref<string>>; // 已 trim
};

// 过滤层：在基础层之上叠加本地过滤（本地过滤面板用）
function useListSearch<T>(
  source: () => readonly T[],
  matcher: (item: T, keyword: string) => boolean,
  options?: { debounceMs?: number },
): {
  keyword: Ref<string>;
  debouncedKeyword: Readonly<Ref<string>>;
  filtered: ComputedRef<T[]>; // 空关键字返回全量
};
```

- 两个 API 同文件导出，`useListSearch` 内部复用 `useDebouncedKeyword`，防抖逻辑只有一份。
- composable 内做 trim 与防抖；大小写归一等匹配细节由 matcher 纯函数负责（可沉淀到 `utils/` 并单测）。
- 对齐 CLAUDE.md 05.5 分层筛选模式：composable 只管关键字层，面板在 `filtered` 之上叠加类型/状态等专用筛选层。
- 配套 fake-timer 单测（防抖时序、trim、空关键字、卸载清理）。

### 7.2 试点改造（修订 2026-07-05：按各面板实际搜索形态对号入座）

- **LauncherPanel、HostsPanel**（本地过滤）：接 `useListSearch`。HostsPanel 由无防抖 computed 变 300ms 防抖，属本 spec 允许的行为微变，写入行为清单并冒烟确认搜索体感。
- **SnippetPanel**（后端搜索：防抖后重新 `loadSnippets()`）：接 `useDebouncedKeyword` + watch，替换手写 `searchTimer`（260ms → 300ms，行为微变入清单）。
- **DnsPanel**：经核实无搜索框，不参与 X4，仅参与 X3。
- 与 X3 同批：每个面板一次提交同时落 X3 + X4（共 4 个提交），避免反复动同一文件。

**验收**：同 X3，另加 useListSearch 单测。

## 8. 顺序、风险与完成定义

**顺序**：X1 → X2 →（X3+X4 按面板捆绑 4 个提交）。合计约 6 天。

**风险与对策**

- 解析式测试对 tauri.ts / events.ts 格式敏感 → 文件头注释约定格式；测试失败信息含修复指引。
- e2e 缺位期间试点面板改造无端到端保护 → 建议路线图批次 0 先行；每面板行为清单先行、照单冒烟。
- 试点面板是日常高频工具（Hosts / Launcher）→ 每面板独立提交，出问题单面板 revert。

**完成定义**

1. 三份 action 清单与两侧事件常量全部纳入 `cargo test` 对账，幽灵词条清零。
2. `src/bridge/events.ts` 与 `src-tauri/src/events.rs` 建立，全仓无应用级事件名裸字面量。
3. 4 个试点面板成为错误反馈与搜索防抖的规范样板；规范沉淀 process.md。

## 9. 决策记录

| 决策                                                     | 结论                                                              | 备选与否决理由                                                                                                                                        |
| -------------------------------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| 推进方式                                                 | 方案 B：四项打包为独立横切面 spec，与路线图并行                   | 方案 A（仅对账+事件小批次）覆盖不足；方案 C（只记账）会让路线图批次 2/3 的 Rust 大拆分缺契约保护                                                      |
| 对账机制                                                 | Rust 侧 supported_actions + 守卫，测试解析 tauri.ts               | 备选"vitest 正则解析 Rust match 臂"被否：Rust 侧格式多样（多行、委托调用），解析脆弱；tauri.ts 一行一条目，反向解析稳健，且 is_supported 已有仓内先例 |
| 对账执行方式                                             | 纯静态集合比对，不执行 action                                     | 备选"逐 action 空 payload 试执行"被否：有副作用与耗时风险（launcher scan、port usage 等）                                                             |
| 事件对账方向                                             | Rust 集合 ⊆ 前端集合                                              | 双向相等不成立：存在纯前端跨窗口事件（hosts-applied）                                                                                                 |
| 试点面板                                                 | Snippet / Launcher / Hosts / Dns                                  | 与路线图批次、候选池、待实施 spec 零交集；VaultPanel（2542 行）体量过大不适合试点                                                                     |
| typeshare / Zod / DAO / 迁移框架 / 外键级联 / 双体系统一 | 均不做                                                            | 单人维护的离线桌面应用，工具链与框架的持续成本大于收益；对账测试 + 手工纪律已覆盖主要风险面；外键级联动用户数据结构，风险大于孤儿记录实际痛感         |
| 错误反馈改造范围                                         | 仅 4 试点 + 规范                                                  | 全量 310 处一次性改造会大面积触碰路线图待拆文件，混批风险高                                                                                           |
| X4 API 形态                                              | useDebouncedKeyword + useListSearch 分层双 API（2026-07-05 修订） | 单一 useListSearch 被否：写计划时核实 SnippetPanel 为后端搜索（防抖后重查 IPC），matcher 形态不适配；DnsPanel 无搜索框，仅参与 X3                     |
