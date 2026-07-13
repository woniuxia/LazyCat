# IPC 契约对账与横切面治理实施计划（X1-X4）

- 日期：2026-07-05
- 依据 spec：`docs/superpowers/specs/2026-07-05-ipc-contract-cross-cutting-design.md`（含同日 X4 分层双 API 修订）
- 影响范围：`apps/desktop` 前后端；无数据库结构变更、无数据迁移
- 执行约定：每阶段独立验证、独立提交、可独立 revert；行为保持（明确标注的两处微变除外）；composable 先写失败测试再实现（TDD）；严格按 spec，不顺手扩散
- 行号均为 2026-07-05 版本，开工时以当时代码为准
- **前置条件**：开工前工作区必须干净。当前工作区有路线图批次 0 的未提交 App.vue 改动（Tauri 环境守卫），须先完成并提交该批次；阶段 6（面板批）强烈建议在批次 0 恢复 e2e 冒烟之后执行

## 总览

| 阶段 | 产出 | 新增/修改文件 |
|------|------|---------------|
| 1 | X1a：38 个工具模块显式声明 supported_actions + 前置守卫 | `src-tauri/src/tools/*.rs` |
| 2 | X1b：mod.rs 聚合、白名单 const 化、契约对账测试 | `tools/mod.rs`、新增 `tools/contract_tests.rs`、`src/bridge/tauri.ts` 头注释 |
| 3 | X2：事件名双侧常量化 + 纳入对账 | 新增 `src/bridge/events.ts`、`src-tauri/src/events.rs`；替换约 13 个文件的字面量 |
| 4 | X3 前置：useToolInvoke 升级（TDD） | `composables/useToolInvoke.ts`、新增 `.test.ts` |
| 5 | X4 前置：useDebouncedKeyword / useListSearch（TDD） | 新增 `composables/useListSearch.ts`、`.test.ts` |
| 6 | 试点面板批（X3+X4 捆绑，4 个独立提交） | Snippet / Launcher / Hosts / Dns 四面板 |
| 7 | 收尾沉淀 | `process.md` |

前端路径相对 `apps/desktop/src/`，Rust 路径相对 `apps/desktop/src-tauri/src/`。

## 阶段 1：X1a 模块 supported_actions（机械改造，零行为变更）

### 1.1 统一模式（以 launcher.rs 为例）

```rust
const ACTIONS: &[&str] = &[
    "scan", "list", "add", "add_manual", "update", "remove", "reorder",
    "launch", "open_folder", "list_groups", "create_group", "rename_group", "delete_group",
];

pub(crate) fn supported_actions() -> &'static [&'static str] {
    ACTIONS
}

pub fn execute(action: &str, payload: &Value) -> Result<Value, String> {
    if !ACTIONS.contains(&action) {
        return Err(format!("launcher: unsupported action '{action}'"));
    }
    match action {
        // 原 match 臂与 `_` 兜底臂全部保持不动（守卫后不可达，作双保险）
    }
}
```

ACTIONS 内容一律照抄该模块 execute 的现有 match 臂，不增不减。

### 1.2 模块清单（38 个，对应 `mod.rs` dispatch 臂）

api_mock、attachments、browser_profiles、convert、cron、crypto、data_dictionary、dns、encode、env、file、format、gen、hosts、hotkey、image、inbox、jwt、launcher、manuals、maven、mybatis、network、nginx、pdf、pm、pomodoro、port、regex、schema、settings、snippets、system、text、time、todo、vault、widget。

capture 走独立 command 不在清单内。

### 1.3 特例处理

- **api_mock**：沿用既有 supported-actions 先例并统一为通用 `ACTIONS` const + `supported_actions()` 形态；内嵌测试语义不变。
- **pm**：ACTIONS 含全部经 pm 域分发的 action（pm.rs:18 起的 match 臂，含委托给 pm_weekly / pm_siyuan / pm_todo_link 的条目，共 45 个，以 match 臂为准照抄）。
- **settings / widget**：存在双入口（settings.rs:9 `execute` 与 :25 `execute_with_app`；widget/mod.rs:25 与 :44）。先读两入口确认 action 分工，ACTIONS 取**两入口并集**，守卫在两个入口都插入。
- **todo**：ACTIONS 以 todo.rs:85-104 现有 20 个 match 臂为准（不含白名单幽灵词条）。
- **data_dictionary**：只加守卫与清单，不碰任何 CLAUDE.md 04.9 不变量；本阶段完成后跑 `cargo test data_dictionary -- --nocapture` 确认。

**验证**：`cargo test`（在 `apps/desktop/src-tauri` 下）+ `pnpm typecheck`
**提交**：`refactor(tools): 工具模块显式声明 supported_actions 并前置守卫`

## 阶段 2：X1b 聚合与契约对账测试

### 2.1 `tools/mod.rs` 改造

1. 新增聚合函数（40 臂，与 dispatch_tool 同序）：

```rust
pub fn supported_actions(domain: &str) -> Option<&'static [&'static str]> {
    match domain {
        "api_mock" => Some(api_mock::supported_actions()),
        // 其余 38 个域按同一结构列出
        _ => None,
    }
}
```

2. 白名单 const 化：`pm_or_todo_data_changed` 的两个 `matches!` 改为

```rust
const PM_WIDGET_REFRESH_ACTIONS: &[&str] = &[ /* 现有 16 条照抄 */ ];
const TODO_WIDGET_REFRESH_ACTIONS: &[&str] = &[
    "item_create", "item_update", "item_change_status",
    "item_toggle_pin", "item_delete",
    // 删除幽灵词条：item_reorder / item_batch_update / item_complete / item_undo_complete
    // （todo.rs 无这些 action，dispatch 不可达，纯清理零行为差）
];
```

函数体改 `.contains(&action)`。

### 2.2 新增 `tools/contract_tests.rs`（`#[cfg(test)]`，在 mod.rs 注册 `#[cfg(test)] mod contract_tests;`）

- `parse_channel_map()`：读 `Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/bridge/tauri.ts")`（用 PathBuf join，勿硬编码分隔符）；逐行正则：
  `^\s*"tool:[^"]+":\s*\{\s*domain:\s*"([a-z_]+)",\s*action:\s*"([a-z0-9_]+)"\s*\},?\s*$`
- **哨兵断言**：解析条目数 ≥ 300（防 tauri.ts 改格式后解析为空、测试假绿）。
- **方向 A**：每条 (domain, action)，`supported_actions(domain)` 必须为 Some 且含该 action —— 防前端运行时炸。
- **方向 B**：每个域的每个 supported action 必须出现在解析集合或 `EXEMPT: &[(&str, &str)]` 豁免表中；EXEMPT 初始为空，实施中发现的每条必须附注释理由 —— 防死代码沉默堆积。
- **白名单**：两个 `*_WIDGET_REFRESH_ACTIONS` 分别 ⊆ `supported_actions("pm" / "todo")`。
- 断言失败信息统一带指引：`新增 action 需同步：模块 supported_actions、bridge/tauri.ts CHANNEL_MAP、（写操作）mod.rs 挂件白名单`。

### 2.3 `src/bridge/tauri.ts` 头部加注释

说明本文件被 src-tauri 契约对账测试逐行解析，CHANNEL_MAP 保持一行一条目格式。

**验证**：`cargo test` + `pnpm typecheck`
**提交**：`test(tools): 前后端 action 契约对账测试与挂件白名单清理`

## 阶段 3：X2 事件常量双侧集中

### 3.1 全量复核事件清单

`grep -rn "\.emit(\|emit_to(\|listen(" apps/desktop/src apps/desktop/src-tauri/src`，与 spec 5.1 的 13 事件清单比对补漏（注意 emit 参数换行形态，如 main.rs:791 的 hotkey-navigate）。

### 3.2 新增 `src/bridge/events.ts`

```ts
// 本文件被 src-tauri 契约对账测试逐行解析：常量保持 `NAME: "value",` 一行一条目。
export const APP_EVENTS = {
  /** Rust → 主窗口：托盘/快捷键切换主窗口显隐 */
  MAIN_WINDOW_TOGGLE: "main-window-toggle",
  // ...其余事件，每条 JSDoc 注明发端 / 收端 / payload 类型出处
  /** 仅前端：Spotlight 窗口 → 主窗口（payload { name: string }） */
  HOSTS_APPLIED: "hosts-applied",
} as const;
```

`tauri://focus` / `tauri://blur` 为框架内置，不入表。

### 3.3 前端字面量替换（各处单行替换）

App.vue（350、358、369、411）、composables/useClipboardSuggestion.ts:69、components/InboxPanel.vue:1315、components/PomodoroPanel.vue:193、components/QuickCapture.vue:27、components/ReminderPopup.vue:225、components/SpotlightPanel.vue:861、components/TodoPanel.vue:2165、components/WidgetCanvas.vue:109、spotlight/providers/hosts.ts:79。

TodoPanel / InboxPanel 是路线图待拆文件，此处仅单行常量替换，符合 spec 纪律 4 的交叠协调。

### 3.4 新增 `src-tauri/src/events.rs`

```rust
pub const EVENT_MAIN_WINDOW_TOGGLE: &str = "main-window-toggle";
// ...

/// 供契约对账测试使用；由具名常量引用组成，无双写漂移
pub const ALL: &[&str] = &[EVENT_MAIN_WINDOW_TOGGLE /* , ... */];
```

main.rs 加 `mod events;`；替换 main.rs 约 14 处（401、418、437、489、549、641、783、791、1116、1149、1165、1214、1259、1393 附近）与 widget/apply.rs:98、101、widget/pulse.rs:91。

### 3.5 不对称事件处置

`pomodoro-prompt-refresh`（main.rs:489 emit）：全仓 grep 无 listener 则删除该 emit 行（行为无关清理，记入提交说明）；若发现隐蔽 listener 则入常量表。其余 listen 无 emit 的情况同法逐一确认。

### 3.6 contract_tests 增用例

解析 events.ts 的常量值集合（哨兵：≥ 10 条），断言 `events::ALL` ⊆ 前端集合。

**验证**：`cargo test` + `pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web` + 手工冒烟 4 项：Todo 提醒触发弹窗、复制文本出现剪贴板建议、挂件点击导航跳转、Spotlight 应用 hosts 方案后主窗口收到提示。
**提交**：`refactor(events): 应用级事件名双侧常量化并纳入契约对账`

## 阶段 4：X3 前置——useToolInvoke 升级（TDD）

### 4.1 先写 `composables/useToolInvoke.test.ts`

桩模式：mock `../bridge/tauri` 的 `invokeToolByChannel`、mock `element-plus` 的 `ElMessage`。用例：

1. `invokeWithLoading` 成功透传返回值，loading 先 true 后 false。
2. 失败默认 `ElMessage.error(message)`，返回 undefined。
3. 传 `{ errorPrefix: "保存失败：" }` 时弹 `保存失败：<message>`。
4. `invokeSilent` 失败返回 undefined 且不调用 ElMessage。
5. 既有两参调用形态行为不变（兼容回归）。

### 4.2 实现（纯增量，现有签名不动）

- `invokeWithLoading<T>(channel, payload, opts?: { errorPrefix?: string })`。
- 新增 `invokeSilent<T>(channel, payload): Promise<T | undefined>`。
- boolean loading 并发覆盖问题本次不改（spec 6.2），在 process.md 记录。

**验证**：`pnpm --filter @lazycat/desktop test src/composables/useToolInvoke.test.ts` + `pnpm typecheck`
**提交**：`feat(composables): useToolInvoke 支持错误前缀与静默调用`

## 阶段 5：X4 前置——useListSearch（TDD）

### 5.1 先写 `composables/useListSearch.test.ts`（`vi.useFakeTimers`）

1. 输入后 300ms 内 `debouncedKeyword` 不变，到点更新且值已 trim。
2. 连续输入只在最后一次输入 300ms 后生效一次。
3. 空/全空白关键字 → `filtered` 返回全量。
4. matcher 过滤生效；`debounceMs` 可覆盖。
5. 作用域销毁后定时器清理（unmount 后不再更新）。

### 5.2 实现 `composables/useListSearch.ts`

按 spec 7.1（修订版）分层双 API：`useDebouncedKeyword` 持有 keyword/debouncedKeyword 与防抖定时器（`onScopeDispose` 清理）；`useListSearch` 复用它并叠加 `filtered` computed。防抖逻辑只此一份。

**验证**：`pnpm --filter @lazycat/desktop test src/composables/useListSearch.test.ts` + `pnpm typecheck`
**提交**：`feat(composables): 新增 useDebouncedKeyword 与 useListSearch`

## 阶段 6：试点面板批（X3+X4 捆绑，4 个独立提交）

### 6.0 通则（每面板执行顺序：列行为清单 → 改造 → 验证 → 照单冒烟 → 提交）

- 用户直接触发的操作失败 → `invokeWithLoading` + `errorPrefix`（如"保存失败："）；后台静默操作 → `invokeSilent` + 注释说明为何静默。
- 多加载态面板用多实例解构：`const { loading: saving, invokeWithLoading: invokeSave } = useToolInvoke();`
- null 歧义逃生舱：若某 action 成功可能返回 null 且调用处需要成功分支判定（如"仅成功才关弹窗"），用组合式的 `invoke()`（抛错）+ 局部 catch + 前缀 toast + 注释。规范禁止的是裸 `invokeToolByChannel` + 手写 try/catch。
- 优先沿用"变更后重新加载列表"的既有模式，失败已 toast、列表如实回显。

### 6.1 SnippetPanel（后端搜索形态）

- 删除本地 `ipc<T>()` 包装（约 310 行），全部调用点收敛到 useToolInvoke。
- `searchTimer`（291、394 行）→ `useDebouncedKeyword` + `watch(debouncedKeyword, () => void loadSnippets())`；260ms → 300ms 为允许微变，入行为清单。
- 行为清单：列表加载、四预设切换（all/favorite/recent7/untagged）、搜索、创建/编辑/删除（含确认）、标签增删、复制、任一失败路径的报错文案。

**提交**：`refactor(snippets): 面板 IPC 反馈与搜索防抖收敛到统一 composable`

### 6.2 LauncherPanel（本地过滤形态，15 个调用点）

- `debouncedQuery` / `debounceTimer`（235-239 行）→ `useListSearch`（matcher 覆盖名称/路径匹配，沿用现有匹配口径）。
- 行为清单：扫描、启动、手动添加、编辑、移除、拖拽排序、分组增删改、搜索匹配口径不变、失败路径。

**提交**：`refactor(launcher): 面板 IPC 反馈与搜索防抖收敛到统一 composable`

### 6.3 HostsPanel（本地过滤形态，16 个调用点）

- `searchKeyword` 无防抖 computed（337-339 行）→ `useListSearch`；**新增 300ms 防抖为允许微变**，冒烟确认搜索体感。
- 6 个加载态（listLoading / saving / activating / deleting / readingSystem / backupListLoading）映射为多实例解构，逐一对应。
- 行为清单：方案 CRUD、激活（含 UAC 提权路径）、读取系统 hosts、备份列表/恢复/删除、排序、搜索、失败路径。

**提交**：`refactor(hosts): 面板 IPC 反馈与搜索防抖收敛到统一 composable`

### 6.4 DnsPanel（仅 X3，4 个调用点）

- 静默 catch（约 240 行"忽略系统 DNS 加载失败"）→ `invokeSilent` + 注释保留原语义。
- 行为清单：域名解析、系统 DNS 读取（含失败静默不打扰）、DNS 对比、失败路径。

**提交**：`refactor(dns): 面板 IPC 错误反馈收敛到统一 composable`

### 阶段验证（每面板）

`pnpm --filter @lazycat/desktop test` + `pnpm typecheck`；批末最后一个面板完成后执行一次 `pnpm --filter @lazycat/desktop build:web`；若批次 0 已恢复 e2e，则每面板提交前跑 `pnpm test:e2e` 冒烟。

## 阶段 7：收尾沉淀

1. `process.md` 记录三条模式：Rust 契约对账测试模式（解析 TS + 哨兵断言）、双侧事件常量模式（ALL 引用聚合防双写漂移）、试点面板收敛样板（多实例解构、errorPrefix、invokeSilent、useListSearch 分层）。
2. 勾稽 spec 第 8 节完成定义三条，逐条确认达成。
3. useToolInvoke 并发 loading 覆盖问题记入 process.md 待办观察。

**提交**：`docs(process): 记录横切面治理 X1-X4 实施经验`

## 风险与回退

- 每阶段独立提交，回退粒度 = `git revert` 单阶段；面板批内单面板独立提交。
- contract_tests 解析对文件格式敏感：哨兵断言（≥300 / ≥10）防假绿；tauri.ts / events.ts 头注释声明格式约定。
- 阶段 1 涉及 todo.rs 等路线图待拆文件，均为每处十行内机械插入；若对应拆分先行，则本计划步骤落到拆后新结构（spec 纪律 4）。
- 中文注释一律 UTF-8；PowerShell 写文件显式 `-Encoding UTF8`（CLAUDE.md 05.2）。
