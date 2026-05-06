# Process Log

本文件记录 LazyCat 项目中重要/复杂操作的处理流程与踩坑经验。

**使用次数规则**：每条记录有 `使用次数` 字段，初始为 0。后续会话遇到相同问题并参考该记录时 +1，并追加引用日期。当使用次数 >= 3 时，固化到 `CLAUDE.md` 对应章节。

---

<!-- 新记录添加在此处，最新的在最上面 -->

## 2026-05-07: Living Wallpaper（合成壁纸）端到端落地

**场景**: 实现 Living Wallpaper 全套：跨 PM/Todo 数据聚合 → hidden WebView 渲染信息层 → CapturePreview 抓 PNG → 与原壁纸合成 → IDesktopWallpaper 设回桌面 → 心跳/事件驱动调度 + 老板键 + 退出策略。Tauri 2.10 + Rust + Vue 3 + windows-rs 0.61，约 17 个 commit。

**问题**:
1. tool_execute 是 `#[tauri::command] fn ... -> ToolResponse` 同步 IPC，但 wallpaper.apply 需要 `AppHandle`（emit + with_webview），且涉及跨线程握手（UI 线程抓帧 vs 工作线程主流程）。如何在不引入 tokio、不破坏现有同步分发的前提下完成？
2. CapturePreview 是 `ICoreWebView2::CapturePreview` + COM stream + 异步回调，PoC 已抽出三段原语（`capture_inner` / `pump_messages` / `read_stream_to_vec`）。要让 PoC 调试入口（`wallpaper-poc-canvas` route）和正式 apply 路径（`wallpaper-canvas` route）共用一份实现，但 PoC 是 `cfg(all(windows, debug_assertions))`、apply 在 release 也要跑。
3. 后端发完 `wallpaper://dashboard-data` 事件后什么时候能抓帧？冷启 hidden WebView ~300ms 创建 + ~200ms Vue 挂载，热路径 ~50ms；如果在 emit 之前监听器还没注册，就丢事件；如果 emit 之后才创建窗口，前端没收到。
4. 心跳调度若没有跳过条件，用户锁屏 / 全屏游戏期间也会强行刷新；空闲降频又涉及 `GetLastInputInfo` + 32 位 tick 回环；锁屏判定用 `WTSRegisterSessionNotification` 需要建 message-only window，太重。
5. PM/Todo CRUD 后想立刷壁纸，但用户连按 3 个完成不能 fire 3 次合成；需要 trailing-edge 5s debounce。
6. 启用壁纸（enable）→ 备份原图 → 合成新图 → 设桌面后，用户退出 LazyCat 默认应恢复原图，否则合成图永久残留在桌面。

**解决**:
1. **AppHandle 路由 + 全程同步握手**。沿 `settings::execute_with_app` 模式扩展：`wallpaper::execute_with_app(action, payload, &AppHandle)` 只在需要 app 的 action 上转发（`apply` / `resume`）；其余仍走 sync `execute`。`tools/mod.rs::execute_tool_with_app` 处只多一行 `"wallpaper" => wallpaper::execute_with_app(...)`。整条 apply 流程不要 async：emit / set_wallpaper / image 编解码本身就是 sync；canvas 握手用 `std::sync::mpsc + tauri::Listener`；CapturePreview 跨 UI/worker 线程用 `Arc<Mutex<Option<Result<Vec<u8>>>>>` + `AtomicBool` done flag + 5ms 轮询。整条流程在工作线程上同步完成，IPC 直接返回结果。
2. **抽到 `tools/wallpaper/capture.rs`，cfg(windows) 真实 + 非 Windows stub，不带 debug_assertions**。PoC 的 `capture_inner` 改为 `crate::tools::wallpaper::capture::capture_inner(webview)` 一行 delegate。public API 用 `pub use imp::capture_inner;` + `cfg(not(windows))` 提供同名 stub fn，调用方完全不感知平台差异。
3. **两阶段握手 + 边沿监听**。`WallpaperCanvas.vue` 挂载完成立即 `emit('wallpaper://canvas-mounted')`；数据到达后等 2 RAF 再 `emit('wallpaper://canvas-ready')`。后端 apply：先 `is_canvas_open(app)` 记录冷启 / 热路径；冷启时 `wait_for_event('wallpaper://canvas-mounted', 2s)`；之后**先注册** `canvas-ready` listener **再 emit** 数据，避免 emit 后立即响应漏接；ready 超时 2.5s 不致命，仍尝试 capture（前端可能渲染慢但已绘制）。
4. **多模块各管一事，all 同步轮询，failures 回退「不锁定」/「不空闲」**。`lock.rs` 用 `OpenInputDesktop + GetUserObjectInformationW` 取桌面名，"Default" 之外都判锁定；`fullscreen.rs` 用 `SHQueryUserNotificationState` 三态判定（`QUNS_BUSY` / `QUNS_RUNNING_D3D_FULL_SCREEN` / `QUNS_PRESENTATION_MODE`）；`idle.rs` 用 `GetLastInputInfo + GetTickCount64`，`u32::wrapping_sub` 处理 tick 回环。所有判定只读不写 state，scheduler::should_skip 串成一条调用链；不再需要后台轮询线程也不需要 message-only window。
5. **trailing-edge 滚动 deadline**。`events.rs::start` 在 std::thread 里 `mpsc::sync_channel(16)` recv → 收到第一条事件后 `deadline = now + 5s`，每来一条新事件就 `deadline = now + 5s` 重置；`recv_timeout(deadline - now)` 超时即触发 apply。`tools/mod.rs::execute_tool` wrap 一层在 PM/Todo 数据变更类 action 成功后 `notify_data_changed` —— 仅 `try_send` 不阻塞业务流，channel 满直接丢（debounce 后只 fire 一次反正等价）。`should_skip` 与 scheduler 同口径，避免事件驱动绕过禁用 / 锁屏。
6. **`RunEvent::ExitRequested` 钩子 + `restore_wallpaper` 复用**。把 `.run(ctx)` 拆成 `.build(ctx).run(|h, ev| match ev { ExitRequested => on_app_exit })`：托盘 Quit / 最后一窗关闭 / `app.exit()` 三个路径都覆盖。`on_app_exit` 读 `wallpaper.exit_behavior`：`keep_last` no-op，`restore_original`（默认）调既有 `restore_wallpaper`。错误只 stderr，不阻塞退出。

**关键点**:
1. **「需要 AppHandle」≠「需要 async」**。Tauri command 本身就是同步包装；`emit` / `with_webview` / `Listener::listen` 都不要求 async runtime。整个 apply 链路用 std::sync::mpsc + Arc<Mutex> 完成跨线程握手，比引入 tokio 简单得多，也避免了 IPC 工作线程上 `block_on` 可能的 reentrancy 风险。
2. **PoC 与正式实现共用底层原语，cfg gate 用 `pub use` + `not(windows)` stub fn 同名互补**。`capture.rs` 是这次最干净的复用：PoC 改为一行 delegate，正式 apply 直接调；release/debug 都可用，跨平台编译过 stub。windows-rs 0.61 有些类型（如 `HDESK` ↔ `HANDLE`）没自动转换，需要手工 `HANDLE(h.0)` 包一次（结构体都是 `*mut c_void`），编译器会精确指出错误位置。
3. **跨 webview 事件握手必须「先注册再 emit」**。Tauri 事件不重放；如果 emit 之后才注册，事件已经派发完毕。两阶段握手（mounted 通知存活 + ready 通知绘制完成）解决冷启动顺序问题；冷启专用 `wait_for_event('canvas-mounted')`，热路径直接跳到 ready 等待。
4. **「跳过判定」是中央枢纽**。心跳 `should_skip` 串 4 个判定（enabled / paused / lock / fullscreen），事件驱动 `should_skip` 串同一组（虽然代码重复一份，函数都很短）；任何新增切净源（Spotlight / 第三方引擎）只改这两处。`lock`/`fullscreen`/`idle` 各自 cfg(windows) 真实 + 非 Windows stub 返回 false（不锁定 / 不空闲），保证「失败回退到不暂停」—— 错误的暂停比错误的刷新更难诊断。
5. **content hash 去重 + force 标记两套**。`apply_with_force(app, force: bool)`：心跳 / 事件驱动 force=false（DefaultHasher 比对 dashboard JSON + base path + base mtime + position + mode，命中跳过整条 compose+persist+set 链路）；手动「立即刷新」/ 老板键 resume / restore force=true。`enable / restore / boss_key.toggle` 三处显式调 `apply::invalidate_input_hash()`，保证桌面切换边界后下一帧必然真渲一次。hash 用 sentinel 0 表示「无效」，结果碰撞回退到 1 防误判。
6. **「事件驱动 + 心跳」双路径但只信 hash**。前者用 trailing-edge 5s debounce 防抖；后者用空闲降频（5min idle → 60min sleep）+ 30s 分块 sleep + 边沿检测「刚回来立刷」。两路径都走 `apply_with_force(force=false)`，靠 hash 去重避免无意义合成；用户感受是「数据一变 5s 内就刷新」「闲着不耗 CPU」「回来立刻看到最新」。

**涉及文件**:
- `apps/desktop/src-tauri/Cargo.toml`（windows features 追加 `Win32_System_StationsAndDesktops` / `Win32_System_SystemInformation` / `Win32_UI_Input_KeyboardAndMouse` / `Win32_UI_Shell_PropertiesSystem` / `Win32_UI_WindowsAndMessaging` / `Win32_Graphics_Gdi`）
- `apps/desktop/src-tauri/src/tools/wallpaper/`（新增模块）：
  - `capture.rs`（CapturePreview 原语）/ `compose.rs`（区域 / 采样 / 缓存 / 合成 / 写盘）/ `desktop.rs`（IDesktopWallpaper + SysParam 双层）
  - `data.rs` + `dashboard_logic.rs`（跨 PM/Todo 聚合）
  - `hidden.rs`（hidden WebView 生命周期）/ `apply.rs`（主流程 + hash 去重）
  - `scheduler.rs`（心跳 + 空闲降频 + burnout）/ `events.rs`（trailing-edge debounce）
  - `lock.rs` / `fullscreen.rs` / `idle.rs`（三个独立检测模块）
  - `boss_key.rs`（Ctrl+Alt+W toggle）
  - `mod.rs`（execute / execute_with_app 路由 + on_app_exit）
- `apps/desktop/src-tauri/src/tools/pm_today.rs`（`priority_rank` 提为 `pub`）/ `tools/todo.rs`（`is_open_status` 提为 `pub`）—— 跨模块复用判定
- `apps/desktop/src-tauri/src/tools/mod.rs`（dispatch_tool 抽出 + PM/Todo CRUD action 白名单 → notify_data_changed）
- `apps/desktop/src-tauri/src/main.rs`（scheduler/events/boss_key 注册；`.run(ctx)` 拆 build+run 接 ExitRequested）
- `apps/desktop/src-tauri/src/wallpaper_poc.rs`（`capture_inner` 改为 delegate）
- `apps/desktop/src/components/`：`WallpaperPanel.vue`（状态卡片 + 三组配置）/ `WallpaperCanvas.vue` + `WallpaperOverviewBlock.vue` + `WallpaperTodoList.vue` + `WallpaperExtensionSlot.vue`（hidden WebView 信息层 360×800）
- `apps/desktop/src/WallpaperCanvasApp.ts`（mount entry）/ `main.ts`（`wallpaper-canvas` 路由）
- `apps/desktop/src/composables/toolCatalog.ts`（"更多工具" 组追加桌面壁纸入口）/ `tool-registry.ts`（注册 WallpaperPanel）
- `docs/superpowers/specs/2026-05-05-living-wallpaper-{design,plan}.md`（设计与实施文档）

**验证**:
- `cargo test --bin lazycat-desktop tools::wallpaper::` → 67 passed（含 4 个 hash 纯函数单测）
- `cargo test --bin lazycat-desktop tools::` → 287 passed（PM/Todo 既有用例不受 dispatch_tool 包装影响）
- `cargo check --bin lazycat-desktop` → ok（仅 pm_siyuan 既有 `private_interfaces` 警告，与本次无关）
- `pnpm typecheck` → ok
- `pnpm --filter @lazycat/desktop build:web` → ok（`WallpaperPanel-*.js` / `WallpaperCanvasApp-*.js` 独立分块）
- 真机端到端实测仍待补：需启动 dev server 在真桌面环境触发一次 `tool:wallpaper:apply` 验证 base 解码 / region 计算 / DPI scale / set_wallpaper 链路

**使用次数**: 0

---

## 2026-04-19: PM 视图扩展与列表渐进式渲染

**场景**: 在 PM 面板上新增「今日 / 列表 / 日历 / 四象限」4 个视图，原本只有看板和甘特两个视图，切换靠 `el-switch`。扩展后需要 6+ 视图共享一个切换器，并对大数据量做响应式与渲染性能兜底。

**问题**:
1. 面板内多视图系统没有注册表；新增一个视图要在 `PmPanel.vue` 的 template、script、watch 里同时改 `v-if === 'xxx'`，扩展性差。
2. 上下文（overview / project-<id>）× 视图正交要求：切项目后视图选择要记住；但 `user_settings` 读写的 key 策略要统一，不然不同入口（侧栏/切换器）切不同步。
3. 列表视图 `el-table` 全量渲染在 1000+ 条数据下首帧明显卡顿；直接迁 `el-table-v2` 会丢失排序、多选、Popover 内联编辑等能力。
4. 切换器响应式降级若用 CSS 媒体查询，面板可能被嵌在不同父布局，阈值不一致；若观察组件自身 `inline-flex` 宽度，又会陷入“自己撑多宽就是多宽”的反馈循环。

**解决**:
1. 新增 `composables/pmViewRegistry.ts`：集中注册 6 个视图（id / label / icon / defineAsyncComponent），`PmPanel.vue` 通过 `<component :is="currentView.component">` 渲染。新增视图只改注册表一处。
2. 新增 `composables/usePmViewMemory.ts` 封装「上下文 → viewId」记忆，`user_settings` key 规则 `pm:view:overview` 与 `pm:view:project-<id>`；侧栏「今日」入口和顶部切换器共用同一个 `setView`，避免两个路径分叉。
3. 列表视图不迁 v2，改为渐进式渲染：当 `groupBy === 'none'` 且过滤后 >500 行时，首批渲染 200 行，滚动到底部 240px 内自动追加 200；排序/筛选/分组变化时重置到首批并回到顶部。滚动监听节流交给浏览器（单次 scroll 事件判断），实现成本低于集成 `vue-virtual-scroller`。
4. 切换器观察 `document.documentElement`（视口尺寸）的 `ResizeObserver`，阈值 1100px；label 在 compact 模式下由 `el-tooltip` 补齐。观察视口等价于「窗口宽度」但不用 CSS 媒体查询，仍保持可被父布局复用的语义。

**关键点**:
1. 面板内的「视图注册表」是 `tool-registry.ts` 的微缩复刻：同样的 `id → async component` 模式，拿到扩展性的同时避免 PmPanel 再膨胀。
2. 渐进式渲染是 el-table 虚拟滚动的廉价替代品：性能目标（首帧 <50ms、滚动不卡）可达，且不触碰成熟组件的多选/排序/内联编辑契约。数据量上到万级再考虑真正虚拟化。
3. `ResizeObserver(document.documentElement)` 是「窗口宽度但不用媒体查询」的稳妥写法；观察组件自身 inline-flex 容器会形成反馈循环，观察父元素又受父布局影响，观察视口最中立。
4. 后端 5 个新 action（`item_today_list / item_today_counts / item_calendar_range / item_matrix_bucket / item_batch_update`）配合 `pm_items` 已有索引 `idx_pm_items_project_status/end_at/status/updated_at/completed_at`，跨项目查询在千行规模下 <5ms，无需额外性能工程。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/pm_today.rs`（新增）
- `apps/desktop/src-tauri/src/tools/pm_calendar.rs`（新增）
- `apps/desktop/src-tauri/src/tools/pm_matrix.rs`（新增）
- `apps/desktop/src-tauri/src/tools/pm.rs`（新增 `item_batch_update`）
- `apps/desktop/src-tauri/src/tools/mod.rs`、`helpers.rs`
- `apps/desktop/src/composables/pmViewRegistry.ts`（新增）
- `apps/desktop/src/composables/usePmViewMemory.ts`（新增）
- `apps/desktop/src/composables/usePmListPrefs.ts`（新增）
- `apps/desktop/src/components/PmViewSwitcher.vue`、`PmKanbanView.vue`、`PmTodayView.vue`、`PmListView.vue`、`PmCalendarView.vue`、`PmMatrixView.vue`、`PmMatrixQuadrant.vue`、`PmTodayCard.vue`、`PmTodaySection.vue`（新增）
- `apps/desktop/src/components/PmPanel.vue`、`src/bridge/tauri.ts`
- `CLAUDE.md`、`AGENTS.md`（新增 04.7 PM 域视图扩展小节）

**验证**:
- `cargo test tools::pm_` → 19 passed
- `pnpm typecheck`
- 后续需补 `pnpm test` / `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-08: PM 侧栏排序口径与项目计数口径必须拆开建模

**场景**: 用户先后确认了两条 PM 规则：`archived` 项目不能再接收工作项，且 `archived` 项目不参与“按任务总数排序”；随后修复 A4 / A6 / A7 时，需要同时解决“总览只看 active”“侧栏排序失真”“总览摘要口径不一致”三类问题。

**问题**:
1. 原实现把 `item_counts()` 同时承担了两个职责：
   - 给项目卡和总览卡提供数字
   - 给侧栏排序提供总任务数依据
2. 一旦后端只统计 active 项目，archived 项目的卡片数字会被误显示成 `0`，总览摘要也会只覆盖 active；但如果粗暴地把 archived 也纳入排序，又会和“archived 不参与排序”的规则冲突。
3. 总览 `item_list()` 原来带 `WHERE p.status = 'active'`，导致 archived 项目下的工作项在总览消失、在单项目视图出现，形成展示分裂。

**解决**:
1. 先把“可见集合”修正为真实全量：
   - 总览 `item_list()` 去掉 `p.status = 'active'`
   - 让 archived 项目的既有工作项也进入总览
2. 再把“计数”和“排序”明确拆开：
   - `item_counts()` 返回所有项目的真实 `total / done`
   - 这样 archived 项目卡数字和总览摘要都基于真实数据
3. 前端 `sortPmProjectsForSidebar()` 改成两段式规则：
   - `active` 项目排在前面，按任务总数降序
   - `archived` 项目统一置后，只按 `sortOrder / name / id` 走稳定兜底
4. 侧栏文案同步改成“活跃项目优先，按任务总数排序”，避免继续把 archived 误描述成参与同一排序口径。

**关键点**:
1. “项目卡要显示真实数字”和“项目列表要怎么排”是两个不同问题，不能继续共用同一口径硬绑在一起。
2. 当产品规则变成“archived 不参与排序”时，最稳的实现不是不给 archived 计数，而是保留真实计数、单独处理排序分支。
3. 总览如果要承担全局入口语义，可见集合和摘要口径必须对齐；否则用户会看到“卡片数和列表内容不是一个世界”的错觉。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/pm.rs`
- `apps/desktop/src/utils/pmVisual.ts`
- `apps/desktop/src/utils/pmVisual.test.ts`
- `apps/desktop/src/components/PmPanel.vue`
- `process.md`

**验证**:
- `cargo test pm::tests -- --nocapture`
- `pnpm test src/utils/pmVisual.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-08: 本周工作面板改为按 PM 计划时间命中本周统计

**场景**: 用户反馈“本周工作”里没有拿到本周进行中的项目管理任务；确认后的口径是：PM 任务是否进入“本周工作”，只看 `startAt / endAt` 是否命中本周，不再依赖 `status` 或 `completedAt`。

**问题**:
1. `pm.rs` 里的 `weekly_work()` 原实现把 PM 数据硬编码为 `status = 'done' AND completed_at >= 最近 7 天起点`，因此只会返回近 7 天已完成任务，进行中或测试中的本周任务会被直接漏掉。
2. 面板标题、空态和工具描述仍写着“最近 7 天工作 / 完成工作汇总”，和“本周工作”的实际入口名称、用户心智以及目标口径不一致。
3. `WeeklyWorkPanel.vue` 的时间列和分组排序都依赖 `completedAt`，即使后端补回 PM 任务，前端也无法正确展示计划时间范围。

**解决**:
1. 在 `pm.rs` 新增纯 helper：
   - `normalize_pm_weekly_range()`
   - `resolve_pm_weekly_window_hit()`
   - `resolve_current_week_window()`
   统一收口“单边日期补全、倒序日期纠正、本周窗口判定和排序日期钳制”。
2. `weekly_work()` 改为：
   - 本周窗口按“周一 00:00 ~ 周日 23:59:59”计算
   - PM 任务只要 `startAt / endAt` 归一化后与本周有交集就返回
   - 不再过滤 `status`
   - Todo 仍按完成时间统计，但窗口同步改为“当前自然周”而不是“最近 7 天”
3. PM 返回体补充 `startAt`、`endAt`、`sortAt`，让前端直接显示计划时间范围并按本周命中时间排序。
4. `WeeklyWorkPanel.vue` 同步改为“本周工作”文案，PM 项展示计划时间范围与状态标签；排序从 `completedAt` 切换为统一 `sortAt`。
5. `App.vue` 的工具描述一并改成“按本周时间范围汇总工作项”，避免入口文案继续误导。

**关键点**:
1. “本周工作”如果要承载 PM 计划视角，不能继续复用“已完成事项”思路；PM 应看排期区间与本周窗口是否相交，而不是看完成态。
2. 对只有单边日期的 PM 任务，周统计也要命中；更稳的做法是把单边日期归一成单日区间，再统一做 overlap 判断。
3. 周窗口如果要避免“23:59:59.xxx”边界遗漏，SQL 过滤尽量用 `[weekStart, nextWeekStart)` 的半开区间，而不是字符串形式的 `<= 周日 23:59:59`。
4. 当统计口径从“完成时间”切到“计划时间范围”后，前端时间列、排序字段和空态文案必须一起切换，否则用户会觉得数据虽然回来了，但展示仍然不对。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/pm.rs`
- `apps/desktop/src/components/WeeklyWorkPanel.vue`
- `apps/desktop/src/types/pm.ts`
- `apps/desktop/src/App.vue`
- `process.md`

**验证**:
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml weekly_ -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-08: Base64 面板自动识别前端收口为纯函数校验 + 手动选择持久化

**场景**: 用户要求按 `2026-04-07-base64-auto-detect-design.md` 实现 Base64 自动识别与类型同步，同时先审校 spec，再按审校后的规则落地前端最小改动方案。

**问题**:
1. 现有 `EncodePanel.vue` 只有一个 `base64UrlSafe` 布尔开关，编码与解码都完全依赖当前按钮状态，粘贴已编码文本时很容易因为类型没切对而直接解码失败。
2. 这次目标不是改后端能力，而是让“输入时自动同步显示 + 解码时按识别结果纠偏”同时成立，因此如果把逻辑直接堆进组件，手动选择、自动识别、切工具恢复三者的优先级会很快变乱。
3. Rust 侧实际用的是 `base64 0.22` 的 `STANDARD` 和 `URL_SAFE_NO_PAD`；如果前端偷懒用 `atob` / `Buffer` 判断，会和后端在 padding、无 padding、trailing bits 上出现细微边界偏差。
4. Base64 输入状态当前通过 `EncodePanel.vue` 顶部普通 `<script>` 里的模块级 `encodeState` 持久化；如果只记显示状态、不记“手动选择”，切换工具回来后歧义输入的解码决策会漂移。

**解决**:
1. 先审校 spec，把实现边界收口为唯一方案：
   - `manualChoice` 必须进入 `encodeState`
   - `detectedKind` 只做运行时派生，不持久化
   - 自动识别同步显示不得反写 `manualChoice`
   - 前端可解码性校验必须自己实现，不能依赖浏览器宽松解码器
2. 新增 `apps/desktop/src/utils/base64.ts`，把识别与解码决策抽成纯函数：
   - `detectBase64Kind`
   - `resolveBase64DecodeKind`
   - 内部按 Standard / URL-safe 两套字母表、padding 规则和 trailing bits 规则做轻量校验
3. `EncodePanel.vue` 中把 Base64 类型切换改成显式 `@update:model-value` 入口，只在用户主动点击时写入 `manualChoice`，避免自动同步污染手动兜底状态。
4. 输入变化时统一走 `watch(base64Input, ..., { immediate: true })`：
   - 明确类型时自动同步显示
   - 歧义类型时按 `manualChoice ?? "standard"` 显示
   - 非 Base64 输入不改当前显示类型
   - 输入清空时重置 `manualChoice`
5. 解码时不直接信当前按钮状态，而是重新识别一次当前输入，再通过 `resolveBase64DecodeKind` 决定最终走 Standard 还是 URL-safe 通道。
6. 新增 `apps/desktop/src/utils/base64.test.ts`，固定明确类型、歧义输入、trailing bits 非法输入、空白输入和解码决策优先级。

**关键点**:
1. 这种“自动识别 + 仍保留手动切换”的交互，最容易错在把自动同步和手动选择混成一个状态；更稳的做法是把“当前显示类型”和“用户手动偏好”分开建模。
2. Vue 表单控件如果既要支持程序性同步，又要精确捕捉“只有用户操作才算手动选择”，优先用 `:model-value + @update:model-value`，不要直接依赖 `v-model` 推断来源。
3. 想和 Rust `base64` crate 保持一致时，前端至少要自己校验最后一个 sextet 的未使用 bits 是否为 0；否则像 `AB==`、`ABC=`、`AB`、`ABC` 这类输入会被前端误判为可解码。
4. Base64 这类共享字符集协议天生会和普通短文本重叠，像 `test` 这样的 4 字符共享字符集文本落入 `ambiguous` 是当前最小改动方案下的接受成本，必须用单测固定，避免后续“优化”把规则漂移掉。
5. 组件级持久化场景下，只恢复显示值不够；凡是会影响歧义分支决策的状态，都要和输入一起持久化，否则切换工具回来后行为会前后不一致。

**涉及文件**:
- `apps/desktop/src/components/EncodePanel.vue`
- `apps/desktop/src/utils/base64.ts`
- `apps/desktop/src/utils/base64.test.ts`
- `docs/superpowers/specs/2026-04-07-base64-auto-detect-design.md`
- `process.md`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/base64.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-07: 代理规范文档按检索场景重构并补 Agent 防错闸门

**场景**: 用户要求同时更新 `AGENTS.md` 和 `CLAUDE.md`，目标不是增加更多规则，而是提升 agent 的实际使用效果，尤其减少漏规则、漏同步、漏验证和误触高风险操作。

**问题**:
1. 原有两份规范虽然内容基本一致，但更像连续说明文；agent 在执行时需要自己从全文拼装“当前任务属于什么场景、该查哪节、该做哪些检查”。
2. 文档中混有不少容易过时的数量型描述，对执行帮助有限，却会增加规范失真概率。
3. 仅做“结构重排”还不够解决 agent 漏项问题；如果没有显式的执行入口，agent 仍会漏掉双文件同步、Element Plus 双文件覆盖、`process.md` 沉淀等约束。
4. 这类规范改动本身又受“双文件同步约束”约束，如果先改一份再手工复制另一份，最容易产生细微漂移。

**解决**:
1. 先走设计闭环：确认目标是“日常检索更快”，再追加“agent 专项优化，以更少犯错为第一优先级”，并将设计固化到 `docs/superpowers/specs/2026-04-06-agent-doc-structure-optimization-design.md`。
2. 正文结构统一重组为 `01-08` 八个一级章节，把原有规则重新收口到：
   - `02.1 问题导向索引`
   - `02.2 Agent 决策闸门`
   - `07.4-07.7` 四组场景化检查清单
3. 将“agent 更少犯错”具体化为两层机制：
   - 顶部 `Agent 决策闸门`：先判断任务场景、同步约束、确认要求、最低验证和经验沉淀
   - 底部场景化检查清单：文档规范改动、普通功能开发、UI / 样式改动、高风险改动
4. 对容易过时但不影响执行的数量型描述做降噪处理，保留路径、文件、机制与流程，不继续保留低收益统计数字。
5. 双文件一致性不靠肉眼，完成后用规范化文本比较：将 `AGENTS.md` / `CLAUDE.md` 的文件名差异抹平后做 `Compare-Object`，确保两份文件除标题与互引对象外完全同构。

**关键点**:
1. 规范文档优化如果目标是“提升 agent 效果”，不能只做目录美化；必须把执行前最常漏的判断前置成显式闸门或 checklist。
2. 对规范类文档，最危险的不是“写少了”，而是“改出语义漂移”；因此设计阶段要明确“只重组，不扩边界”，清单也只能映射已有规则，不能借机发明新规则。
3. 像模块数、通道数、组件数这类统计信息，如果对执行帮助不大又容易过时，宁可删掉数字保留路径与机制，这样 agent 命中率更高、维护成本也更低。
4. 双文件同步场景下，完成后的结构一致性校验最好走文本归一化后比对，不要只凭人工快速扫一遍。

**涉及文件**:
- `AGENTS.md`
- `CLAUDE.md`
- `docs/superpowers/specs/2026-04-06-agent-doc-structure-optimization-design.md`
- `process.md`

**验证**:
- `Compare-Object` 对 `AGENTS.md` / `CLAUDE.md` 做归一化比对，结果为 `IDENTICAL`
- 检查两份文件的 `##` / `###` 标题结构完全一致
- 人工校对关键规则、构建发布规则、`process.md` 规则和 agent 检查清单均已保留

**使用次数**: 0

## 2026-04-06: Windows 正式发版前先处理版本号与已存在 tag 冲突

**场景**: 用户要求直接开始打包编译新版本并推送到 GitHub Release；仓库已存在上一版 `v0.2.6` tag，本地 4 处版本文件仍停留在 `0.2.6`。

**问题**:
1. `scripts/release-all-win.ps1` 会强校验 `package.json`、`apps/desktop/package.json`、`apps/desktop/src-tauri/Cargo.toml`、`apps/desktop/src-tauri/tauri.conf.json` 四处版本完全一致，并要求传入 tag 必须等于 `v<version>`。
2. 远端如果已经存在同名 tag，继续沿用旧版本号会在发布阶段直接撞上旧 tag，无法当成“新版本”重新发。
3. 发布脚本在上传 GitHub Release 前会要求当前分支为 `main` 且工作区干净，因此版本更新、经验记录这类改动必须先提交，不能留到脚本执行中途。

**解决**:
1. 先用 `git tag --list` / `git ls-remote --tags origin` 确认目标版本 tag 是否已存在，再和用户确认新的发布版本号。
2. 把新版本统一写入上述 4 个文件后，先执行：
   - `pnpm typecheck`
   - `pnpm --filter @lazycat/desktop build:web`
   - `pnpm test`
3. 校验通过后，先提交版本变更，再执行 `pnpm release:all:win -- -Tag vX.Y.Z`，让脚本完成构建、推送 `main`、推送 tag 和 GitHub Release 上传。

**关键点**:
1. 发布前先查 tag 是否已存在，比等脚本跑到最后再失败更省时间。
2. 这个项目的正式发版版本源只有 4 处；只改其中一部分会被脚本直接拦下。
3. 只要要走 GitHub Release，发布脚本就要求干净工作区，因此“版本号修改”和“提交版本号”是正式发版链路的一部分。

**涉及文件**:
- `package.json`
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/src-tauri/tauri.conf.json`
- `scripts/release-all-win.ps1`
- `process.md`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`

**使用次数**: 0

## 2026-04-06: 项目管理甘特图周末日期坐标增加红色圆底

**场景**: 用户要求在项目管理甘特图的时间轴坐标里，为周末日期加一个红色圆底；同时明确不改已有周末整列淡色提示，只增强顶部日期数字本身。

**问题**:
1. `frappe-gantt` 的日期坐标不是 SVG 文本，而是渲染在 `.gantt-container` 内的绝对定位 `.lower-text` HTML 节点；如果按 SVG 选择器去做，会直接挂错层。
2. PM 甘特图存在 `render()`、`change_view_mode()`、`refresh()` 三条重建/重绘链路；如果只在首次渲染后补类，切视图或数据刷新后高亮会丢。
3. `Week` / `Month` 视图下的 `lower_text` 分别是周范围和月份文本，不能简单按类名里带日期就一律标红，否则会把“周起始日期”误当成周末日期。
4. `.lower-text` 默认宽度是列宽的 80%，直接给节点本身加背景会得到宽胶囊，而不是用户要的圆底。

**解决**:
1. 在 `pmGantt.ts` 新增 `shouldHighlightPmGanttWeekendLabel()` 纯函数，统一收口：
   - 仅 `viewMode === 'Day'` 时生效
   - 从类名中解析 `date_YYYY-MM-DD`
   - 非法日期或缺失日期类名时直接返回 `false`
2. `PmGanttView.vue` 新增 `syncGanttWeekendDateClasses()`，统一扫描 `.lower-text` 并切换 `pm-gantt-weekend-date` 类。
3. 将该同步逻辑接入三条链路：
   - 新实例 `renderGantt()` 后
   - `changeViewMode()` 后
   - `ganttInstance.refresh()` 后
4. 样式层使用 `::before` 伪元素在日期文本正中绘制固定 `24x24` 红色圆底，并通过 `isolation + z-index` 保证圆底在字后、不会扩成整格背景。

**关键点**:
1. `frappe-gantt` 头部日期坐标的可定制入口优先看 `.lower-text` / `.upper-text` 这类 HTML 节点，不要先入为主按 SVG 文本处理。
2. 时间轴头部装饰如果要跨 `render / refresh / change_view_mode` 稳定存在，最好抽成独立同步函数，和条目选中态一样走统一补丁链路。
3. 做“圆底数字”这类视觉增强时，不要直接给整块 header cell 上背景；更稳的是给文本节点本身加类，再用伪元素绘制固定尺寸圆底。
4. 周末识别规则最好放在纯函数里做单测，避免日期解析散在组件 DOM 代码中。

**涉及文件**:
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/utils/pmGantt.ts`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/pmGantt.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-06: 项目管理甘特图首次进入定位改为项目层无动画接管

**场景**: 用户要求按 `2026-04-06-pm-gantt-initial-scroll-design.md` 实现项目管理甘特图首次进入定位：保留“默认看今天附近”，去掉每次进入时从左向右的平滑滑动，并让 today 落在视口左侧约三分之一处。实现后又追加收口：虽然视口不再滚动，但任务条本身仍会从左往右长出来，也需要一起去掉。

**问题**:
1. `PmGanttView.vue` 每次进入都会新建 `frappe-gantt` 实例；若不覆盖配置，库会沿用默认 `scroll_to: 'today'`，重新触发一次 smooth scroll。
2. 现有组件已经在 `refresh()` 和 `change_view_mode(..., true)` 链路上做了“保持当前滚动位置”，如果把首次定位逻辑混进这些链路，容易误伤用户手动滚动后的视口。
3. 仅靠 PM 侧自己镜像日期差值算法并不稳，因为 `frappe-gantt` 在不同 view mode / infinite padding 下本身就有内部时间轴换算与 DOM 偏移。
4. 初次渲染时 `.gantt-container` 和 `.current-highlight` 可能晚一帧才稳定，若直接同步计算，容易出现“没定位到 today”或反复重试。
5. 即使关掉了视口滚动，`frappe-gantt` 仍会在 `bar.js` 里通过 SVG `<animate>` 把 `bar` / `bar-progress` 的宽度从 `0` 补到最终值，因此用户还会看到项目条目“从左往右出现”。

**解决**:
1. 新实例创建时显式传入 `scroll_to: 'start'` 和 `infinite_padding: true`，先确定性绕开库内部 `scrollTo({ behavior: 'smooth' })` 的 today 默认路径。
2. 在 `pmGantt.ts` 新增纯函数 `computePmGanttInitialScrollLeft`，只负责“today 左侧三分之一落点 + 0/max 边界钳制”，不感知 DOM。
3. `PmGanttView.vue` 中新增一次性初始定位状态：
   - 新实例创建后立即尝试读取 `.gantt-container` 与 `.current-highlight`
   - 若尺寸或高亮未就绪，仅补一次 `requestAnimationFrame`
   - 第二次仍不可用则直接放弃，保留 `scroll_to: 'start'` 的起始落点
4. `today` 坐标统一按真实 DOM 计算：
   - `currentX = (highlightRect.left - viewportRect.left) + viewport.scrollLeft`
   - 不再使用 `offsetLeft` 或手写日期差值镜像
5. 初始定位只在新实例创建后执行一次；`refresh()` 和 `change_view_mode(..., true)` 继续沿用现有滚动保持逻辑，不读取也不重置首次定位标记。
6. 对条目本身的默认 SVG 宽度动画，不去 patch 第三方包，而是在 `PmGanttView.vue` 渲染后统一移除 `.bar-group animate` 节点；新建实例、切视图和 `refresh()` 后都要执行一次，确保条目直接以最终宽度出现。

**关键点**:
1. 只要目标是“保留 today 语义但取消动画”，优先用 `scroll_to: 'start'` 把第三方默认滚动关掉，再由项目层直接写 `scrollLeft`，比 patch 依赖更稳。
2. 对第三方时间轴组件做定位时，优先读它已经渲染出来的真实锚点 DOM，而不是在业务层复制一套坐标算法。
3. “首次定位”必须和“同实例刷新/切视图的滚动保持”分开建模；前者是一次性初始化，后者是用户上下文保持，不能混在一个状态里。
4. 对首帧未稳定场景只补一帧重试即可，再多会引入闪动和状态复杂度；失败时安静降级到起始位置比持续纠偏更稳。
5. 如果用户反馈“已经不滚动了，但任务条还会自己展开”，优先检查第三方是否注入了 SVG `<animate>`，这和 CSS transition 不是一类问题。

**涉及文件**:
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/utils/pmGantt.ts`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/pmGantt.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-05: 项目管理状态筛选从甘特专用迁移为共享工具栏筛选

**场景**: 用户要求把项目管理里“当前甘特图至少筛选状态进行任务显示”的能力拓展到看板视图，同时把入口改成顶部工具栏里的多选下拉，并要求同步把“可视化辅助默认开启”写进 `AGENTS.md` / `CLAUDE.md`。

**问题**:
1. 仓库里已经存在一套 `pmGanttFilter.ts + PmGanttView.vue` 的甘特专用状态筛选实现，如果直接在看板里复制一套逻辑，会形成两份状态源和两套默认值。
2. `PmPanel.vue` 的看板列是按 `PM_STATUS_COLUMNS` 全量渲染，再靠列内数据为空显示空态；用户这次明确要求“未选中的状态列直接隐藏”，这会影响列渲染、`Sortable` 初始化和空态判断。
3. `PmGanttView.vue` 当前既负责甘特图自身交互，也负责状态筛选 UI；如果不把接口收口，后续看板和甘特会持续被“谁才是状态筛选入口”这个边界问题拖累。
4. `el-select multiple` 默认会在闭合态显示已选标签，但本次产品要求固定只显示“状态筛选”，不能把选中状态名露在工具栏里。

**解决**:
1. 将旧的 `pmGanttFilter.ts` 迁移为 `pmStatusFilter.ts`，把以下能力集中成 PM 共享 helper：
   - 默认状态集合
   - 多选切换 / 全选 / 清空
   - 稳定顺序数组输出
   - 未知状态按 `todo` 兜底
   - 看板可见列与状态分组
2. `PmPanel.vue` 中把 `ganttSelectedStatuses` 提升并改名为 `selectedStatuses`，然后拆出：
   - `baseFilteredItems`：搜索 / 类型 / 优先级
   - `statusFilteredItems`：在 `baseFilteredItems` 上叠加共享状态筛选
   - `visibleStatusColumns`：只渲染当前选中的状态列
3. `PmGanttView.vue` 移除内部状态 chip 与 `全选 / 清空`，只保留甘特自己的视图切换和统计信息；甘特图只接收父层已经筛好的 `items`。
4. 工具栏里的固定文案多选下拉通过“覆盖式标签”实现：
   - 外层放固定 `状态筛选` 文案
   - 内层 `el-select multiple` 继续负责真实选择
   - 用局部 CSS 隐掉默认标签和 placeholder
5. `Sortable` 初始化改为只遍历当前 `visibleStatusColumns`，并让 `setColumnRef()` 在列被隐藏时同步删除旧 ref，避免隐藏列残留拖拽实例。

**关键点**:
1. 这类“从单视图筛选升级为共享筛选”的任务，先迁 helper、再迁状态源、最后收口子组件接口，比直接在组件里局部打补丁稳得多。
2. 看板列一旦允许隐藏，`Sortable` 的初始化集合不能再硬编码为全部状态列，否则很容易绑定到已经卸载的 DOM。
3. 固定文案的多选下拉不一定要自定义整个浮层；如果只是想隐藏已选标签，给 `el-select` 做一层覆盖式标签和局部 CSS 就够了。
4. 未知状态兼容不能只做到“筛选命中”，还要做到“看板归列”；否则工作项会通过筛选但在列分组时消失。

**涉及文件**:
- `AGENTS.md`
- `CLAUDE.md`
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/utils/pmStatusFilter.ts`
- `apps/desktop/src/utils/pmStatusFilter.test.ts`
- `process.md`

**验证**:
- `pnpm test src/utils/pmStatusFilter.test.ts src/utils/pmGantt.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-04: 项目管理视觉统一规划的后半程收尾

**场景**: 用户要求继续执行 `2026-04-04-pm-visual-unification-design.md`，仓库里已经存在一半未提交实现：`PmPanel.vue` 模板和 `pmVisual.ts` / `pmVisual.test.ts` 已经开始改，但整体视觉还没有真正统一落地。

**问题**:
1. 当前断点不是“功能没接上”，而是“模板结构先改了，样式层还停留在旧时代”；`typecheck` 和 `build:web` 都能过，但新类名大量缺少配套 CSS，实际界面会呈现半成品状态。
2. `PmPanel.vue` 同时包含 scoped 样式和全局样式两段，侧栏/看板/详情在第一段，弹窗和 Teleport 相关样式在第二段；如果只在一处补样式，很容易出现“主页面好了，弹窗还是旧壳”。
3. 工作项编辑弹窗由 `el-dialog` Teleport 到 `body`，不能依赖只挂在 `.pm-panel` 上的局部 CSS 变量；弹窗样式若直接复用 scoped 区块里的变量，实际渲染时可能拿不到。
4. 这类 UI 收尾任务最容易被“编译通过”误导，必须额外检查“新模板类名是否真的有样式定义”，不能只看 TS 和构建结果。

**解决**:
1. 保留已有模板和前端派生逻辑，继续复用 `pmVisual.ts` 中的排序和标签摘要 helper，不回退已有中间成果。
2. 在 `PmPanel.vue` 的 scoped 样式中追加一整套视觉统一覆盖，集中补齐：
   - 左侧项目空间卡片化样式
   - 看板列与卡片的冷白蓝灰视觉
   - 详情侧栏的主信息卡、时间轨迹网格、资源关联行卡片
3. 在 `PmPanel.vue` 的全局样式块中补齐 Teleport 弹窗相关样式：
   - 顶部项目身份卡
   - 分区表单卡片
   - 三列核心信息栅格
   - 链接输入尾部动作区
   - 移动端收口
4. 验证时不能只跑新增 `pmVisual.test.ts`，还要把 PM 既有的日期、甘特、思源辅助测试一起跑一遍，避免 UI 收尾顺手打断现有 PM 派生逻辑。

**关键点**:
1. “规划执行一半”的 UI 任务，先看 `git diff` 和模板类名，再看编译结果；很多时候真正没完成的是样式层，而不是逻辑层。
2. 同一个组件里如果既有 scoped 样式又有 Teleport 场景，要先分清“谁负责页面内样式，谁负责弹窗/全局样式”，否则补丁会落在错误位置。
3. Teleport 内容不要依赖父容器上的局部 CSS 变量；更稳的做法是对弹窗区块使用硬编码设计色或全局变量。
4. 做视觉统一时，优先用“末尾覆盖样式”补齐新骨架，比大面积清洗旧 CSS 更稳，尤其适合已有一半中间改动的工作区。

**涉及文件**:
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/utils/pmVisual.ts`
- `apps/desktop/src/utils/pmVisual.test.ts`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop test src/utils/pmVisual.test.ts src/utils/pmDate.test.ts src/utils/pmGantt.test.ts src/utils/pmSiyuan.test.ts`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-04: 项目管理甘特图新增状态多选筛选

**场景**: 用户要求给项目管理甘特图增加状态筛选，状态可多选；最终确认不新增项目筛选，继续复用左侧项目 / 总览切换。

**问题**:
1. `PmPanel.vue` 里原本只有一层 `filteredItems`，同时服务看板和甘特；如果直接把状态筛选叠上去，会误伤看板四列数据。
2. `PmGanttView.vue` 之前把工具栏放在“有甘特条才显示”的分支里；一旦用户点击 `清空`，工具栏会跟空态一起消失，用户也就失去了恢复筛选的入口。
3. 状态筛选虽然只有 4 个按钮，但仍有三个容易被实现带偏的细节：
   - 未知状态运行时兼容
   - `selectedStatuses` 顺序稳定
   - 清空后保持空选择，而不是自动回填全选
4. 仓库当前没有现成的 `PmPanel` / `PmGanttView` 组件测试基础，如果把全部行为都压在组件内，回归只能靠手工点。

**解决**:
1. 新增 `pmGanttFilter.ts`，把甘特状态筛选规则抽成纯函数，集中处理：
   - 默认全选
   - 单个切换
   - 全选 / 清空
   - 未知状态按 `todo` 兜底
   - 输出稳定顺序数组
2. `PmPanel.vue` 将原 `filteredItems` 拆成：
   - `baseFilteredItems`：搜索 / 类型 / 优先级，继续供看板使用
   - `ganttFilteredItems`：在 `baseFilteredItems` 之上叠加状态筛选，只供甘特使用
3. `PmGanttView.vue` 新增 4 个状态按钮和 `全选 / 清空` 事件，并把工具栏从空态分支里拆出来，保证空态下仍可恢复筛选。
4. 测试策略优先纯函数：
   - `pmGanttFilter.test.ts` 负责筛选状态行为和未知状态兼容
   - `pmGantt.test.ts` 继续守住单边日期 / 非法日期的甘特排期语义

**关键点**:
1. 像“只作用于某个视图”的筛选，不要直接塞进所有视图共用的 `filteredItems`；最好先拆出共享基础筛选，再叠视图专用筛选层。
2. 允许用户 `清空` 筛选时，**工具栏绝不能挂在“有结果才显示”的分支里**，否则产品行为会自相矛盾。
3. 状态筛选如果对外用数组，必须稳定顺序、去重输出；否则 Vue diff、按钮选中态和测试断言都容易抖。
4. 在缺少组件测试基础时，优先把易错规则抽成纯函数单测，比直接堆在 SFC 内更稳。

**涉及文件**:
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/utils/pmGanttFilter.ts`
- `apps/desktop/src/utils/pmGanttFilter.test.ts`

**验证**:
- `pnpm test src/utils/pmGanttFilter.test.ts src/utils/pmGantt.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-04: 项目管理工作项新增外部链接字段与打开动作

**场景**: 用户要求在项目管理工作项中新增一个通用链接字段，可在编辑弹窗里维护，并支持从详情区与右键菜单直接打开。

**问题**:
1. `pm_items` 只有标题、描述、时间和思源页面缓存，没有独立的外部链接字段，无法承接 Jira、禅道、本地服务地址这类通用跳转目标。
2. 仓库里虽然已有 `vault:open-url` / `todo:open_link` 能力，但如果 PM 直接复用其他域的 action，会让领域边界变得模糊，后续维护时难以理解“为什么 PM 要走 Vault/Todo”。
3. 新增字段后，前端 `PmItem` 类型、弹窗草稿态、详情展示和测试 fixture 必须同步补齐；否则最容易在 `typecheck` 阶段被遗漏的测试桩卡住。
4. URL 输入允许用户只填 `localhost:3000` 这类地址，但又必须拒绝 `ftp://` 等非 `http/https` 协议，不能简单靠前端字符串拼接放过异常输入。

**解决**:
1. 在 `helpers.rs` 中为 `pm_items` 新增 `link_url TEXT DEFAULT NULL`，并追加 `ALTER TABLE pm_items ADD COLUMN link_url TEXT DEFAULT NULL` 迁移，兼容现有数据库。
2. 在 `pm.rs` 内新增 `normalize_item_link_url()`、`parse_item_link_url_value()` 和 `open_link()`：
   - 空值统一落库为 `NULL`
   - 未带协议时自动补 `http://`
   - 已带其他协议（如 `ftp://`）时直接拒绝
   - 打开动作仍由 PM 域自己暴露 `tool:pm:open-link`
3. `item_list` / `item_create` / `item_update` 同步接入 `link_url`，前端 `PmItem` 增加 `linkUrl`，`PmPanel.vue` 的工作项弹窗新增“链接”输入和“打开”按钮。
4. 右侧详情面板补“链接”展示行，右键菜单增加“打开链接”；这样用户既可以在编辑态测试，也可以在浏览态快速打开。
5. `pmGantt.test.ts` 的 `PmItem` fixture 需要同步补 `linkUrl: null`，否则 `PmItem` 新字段会直接打断全局 `typecheck`。

**关键点**:
1. 给已有业务表补字段时，前端类型、表单草稿、后端 CRUD、测试 fixture 要一口气补齐；否则 `build` 可能过了，`typecheck` 仍会被测试文件拦住。
2. “自动补协议”只能对无协议输入生效；如果用户已经输入 `xxx://`，必须先判断协议是否合法，不能盲目前缀成 `http://xxx://...`。
3. PM 这类业务域新增动作时，优先在本域增加 `open_link`，不要为了省几行代码直接跨域借用别的工具通道。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/pm.rs`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/types/pm.ts`
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path "E:/Projects/LazyCat/apps/desktop/src-tauri/Cargo.toml"`
- `cargo test --manifest-path "E:/Projects/LazyCat/apps/desktop/src-tauri/Cargo.toml" normalize_item_link_url -- --nocapture`
- `pnpm test src/utils/pmGantt.test.ts`

**使用次数**: 0

## 2026-04-02: 项目管理工作项弹窗切换为时间范围 + 思源紧凑列表

**场景**: 用户要求按 spec 落地项目管理工作项弹窗刷新，只改“新建/编辑工作项”弹窗，不扩散成整页 PM 重构，同时要把历史单边日期、带时间部分字符串和思源关联区的大卡片布局一起收敛。

**问题**:
1. `PmPanel.vue` 里时间输入还是两个独立 `el-date-picker`，并且直接用 `new Date('YYYY-MM-DD')` 做禁用与逾期判断，带来隐含时区解析和历史脏数据兼容问题。
2. PM 日期消费散落在看板卡片、详情区和甘特图，部分逻辑只会 `slice(0, 10)`，部分逻辑直接 `new Date(...)`，导致“表单、展示、逾期、甘特”难以保持同一套本地日期语义。
3. 思源关联区还是“主页面 / 附加页面”双分段卡片结构，纵向占位大；如果只在旧结构上删按钮，很难达到 spec 要求的“摘要头 + 统一紧凑列表”。
4. “关联页面”入口与“更换主页面 / 设为主页面”其实是两种不同意图：前者遇到已存在页面应提示并保持顺序不变，后者则允许页面提升为主页面；如果仍共用同一分支，很容易把重复选择错误地变成主页面提升。

**解决**:
1. 新增 `apps/desktop/src/utils/pmDate.ts`，集中提供：
   - `normalizePmDateString`
   - `parsePmDateAtLocalStart`
   - `parsePmDateAtLocalEnd`
   - `normalizePmDateRangeForDraft`
   - `formatPmDateRangeForDisplay`
   - `isPmItemOverdue`
2. `PmPanel.vue` 的工作项弹窗改成：
   - 标题单行
   - 类型 + 优先级同排
   - 状态单行
   - 单个 `daterange` 作为“时间安排”
   - 思源关联改为摘要头 + 统一行列表
3. 编辑历史数据时先在弹窗初始化层归一：
   - 单边日期映射为同日范围
   - 倒序日期自动升序
   - 非法日期按空处理
   保存时始终显式提交 `startAt/endAt`，避免继续把脏值带回后端。
4. 思源关联区引入“对话框意图”状态：
   - `link`：顶部 `关联页面`
   - `replace-primary`：主页面行的 `更换主页面`
   这样重复页面选择时才能正确区分“提示已存在”与“提升为主页面”。
5. 甘特图切到同一套 `pmDate` helper，确保未排期统计、历史带时间部分日期和逾期判断都走统一本地日期语义。
6. 新增 `pmDate.test.ts`，并补充 `pmGantt.test.ts` 场景，覆盖非法值、时间部分、单边日期、倒序日期与逾期边界。

**关键点**:
1. 只要输入语义是“本地日期”，就不要再写 `new Date('YYYY-MM-DD')`；必须先把值归一到 `YYYY-MM-DD`，再用 `new Date(year, monthIndex, day, ...)` 构造本地时间。
2. “关联页面”与“更换主页面”不能共用同一条选择后处理逻辑，否则重复选择时会把原本应该 no-op 的操作变成状态迁移。
3. 弹窗里的日期归一应只发生在草稿态；如果用户取消，不写回历史单边日期或脏数据的归一结果。
4. 统一列表想做到 44~52px 的紧凑高度，标题和路径都必须收敛成单行省略，不能继续沿用旧卡片的多行文本与分段标签。

**涉及文件**:
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/utils/pmDate.ts`
- `apps/desktop/src/utils/pmDate.test.ts`
- `apps/desktop/src/utils/pmGantt.ts`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/pmDate.test.ts src/utils/pmGantt.test.ts src/utils/pmSiyuan.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-02: Brainstorming 本地预览在 Windows 仓库内补齐桥接脚本

**场景**: brainstorming 过程中需要使用 visual companion 做本地浏览器预览，但仓库根目录没有 `scripts/start-server.sh` 入口，Windows 环境里也没有可直接用的 `bash`，导致技能文档里的默认启动方式无法在当前项目里直接落地。

**问题**:
1. 真正的预览服务脚本位于 `C:\\Users\\huahua\\.codex\\skills\\brainstorming\\scripts\\`，而不是仓库 `scripts/`；如果只按仓库相对路径调用，会误判成“能力缺失”。
2. Windows 环境里 `bash --version` 直接失败，不能假设 skill 里的 `.sh` 脚本能在本机无缝执行。
3. Node 预览服务默认随机高位端口时，Windows 可能命中不可绑定端口段，触发 `listen EACCES: permission denied 127.0.0.1:<port>`。
4. 仓库当前 `.gitignore` 忽略 `.superpowers/`，但历史上又有被跟踪的 brainstorming 产物；如果提交前不核对暂存区，容易把旧 session 删除误带进 commit。

**解决**:
1. 在仓库 `scripts/` 下补齐桥接入口：
   - `start-server.ps1`
   - `stop-server.ps1`
   - `start-server.sh`
   - `stop-server.sh`
2. `ps1` 桥接脚本负责：
   - 从 `$CODEX_HOME` 或 `~/.codex` 定位真正的 skill 脚本
   - 在项目 `.superpowers/brainstorm/` 下创建 session 目录
   - 先探测一个可实际监听的空闲端口，再把 `BRAINSTORM_PORT` 注入给 `server.js`
   - 读取 `.server-info` 作为启动成功判据
3. Windows 下优先使用 `powershell -ExecutionPolicy Bypass -File .\\scripts\\start-server.ps1 -ProjectDir .`；`.sh` 入口只保留给兼容 skill 约定的调用方。
4. 提交前必须显式执行：
   - `git status --short`
   - `git diff --cached --name-status`
   确认没有把 `.superpowers/` 里的历史跟踪文件删除误带进提交；若误删已进入提交，需要先从上一提交恢复，再用 `git add -f` 明确把这些跟踪文件补回。

**关键点**:
1. PowerShell 参数不能命名为 `Host`，否则会和只读的 `$Host` 变量冲突；桥接脚本里应使用 `BindHost` 之类的名字。
2. 预览服务是否真的可用，不能只看进程有没有启动；至少要同时检查 `.server-info` 和 `Invoke-WebRequest http://localhost:<port>` 是否返回 200。
3. 对依赖关系明确的 Git 操作（`add -> status -> commit`）不要并行执行，容易因为时序问题误判暂存区状态。

**涉及文件**:
- `scripts/start-server.ps1`
- `scripts/stop-server.ps1`
- `scripts/start-server.sh`
- `scripts/stop-server.sh`

**验证**:
- `powershell -ExecutionPolicy Bypass -File .\\scripts\\start-server.ps1 -ProjectDir .`
- `Invoke-WebRequest -UseBasicParsing http://localhost:<port>`

**使用次数**: 0

## 2026-04-02: 项目管理思源页面关联弹窗切换为默认位置列表优先

**场景**: 用户要求按新 spec 重做项目管理中的“关联思源页面”弹窗，默认打开就展示当前有效位置下的全部文档，输入改为本地过滤，只有手动点击“扩展到全库”时才发起远端搜索。

**问题**:
1. `PmPanel.vue` 之前把页面关联弹窗建模成“搜索框 + 搜索结果”，打开时会预填标题并自动走远端搜索，和当前位置列表优先的交互目标冲突。
2. 默认位置列表其实已经能从思源目录树缓存派生，但仓库里缺少“按位置定位目录子树并拍平成页面列表”的纯函数，导致组件层很难稳定处理根目录、父文档、位置失效和空目录等状态。
3. 目录刷新与全库搜索是两条不同链路，如果仍复用同一套 `searchResults` 状态，很容易出现“全库结果覆盖当前位置状态”或“输入变化后还停留在旧全库结果”的串态问题。

**解决**:
1. 在 `pmSiyuan.ts` 中新增 `collectPmSiyuanPagesForLocation()` 与 `filterPmSiyuanPages()`，把“位置定位 + 子树拍平 + 本地过滤 + 无效位置/空目录判定”都下沉到纯函数。
2. `PmPanel.vue` 的页面关联弹窗改成双数据源状态：
   - `location`：当前位置完整列表 + 本地过滤
   - `all`：一次性手动触发的全库搜索结果
3. 打开弹窗时固定重置为 `location` 模式并清空输入；若已有目录缓存则先直接派生默认列表，再后台静默刷新目录；若无缓存则先加载目录再展示。
4. 输入框不再触发远端请求；只有点击“扩展到全库”时才调用 `tool:pm:siyuan-search-pages`，且 `all` 视图下继续修改输入会立即退出回 `location`。
5. 增加“返回当前位置列表”入口、位置状态空态文案和目录刷新失败的轻量提示，同时在 `pmSiyuan.test.ts` 补齐根目录、父文档、回退位置、本地过滤、位置失效与空目录测试。

**关键点**:
1. 这种“默认列表 + 手动扩展搜索”的弹窗，不要再把当前位置列表和远端搜索结果塞进同一组状态；至少要明确区分当前展示源，否则输入、刷新和返回操作很容易互相污染。
2. 思源目录树到页面列表的派生必须走纯函数，组件只保存“最新位置状态 + 当前展示源”，这样目录静默刷新时才能在 `all` 模式下继续后台更新 `location` 快照。
3. 目录刷新失败时，如果已经有上一次成功解析的位置结果，应该保留旧列表并给轻量提示；只有首次加载且没有任何位置结果时，才进入主体错误空态。

**涉及文件**:
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/utils/pmSiyuan.ts`
- `apps/desktop/src/utils/pmSiyuan.test.ts`

**验证**:
- `pnpm test src/utils/pmSiyuan.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-30: 项目管理思源存储位置选择器树节点错乱与轻量目录选择器重构

**场景**: 用户反馈项目管理里“绑定项目专属存储位置”弹窗显示很乱，树节点标题与路径重叠、层级难看清，希望排查原因并把交互一起优化。

**问题**:
1. `PmPanel.vue` 的位置选择弹窗给 `el-tree` 自定义节点塞了“标题 + 路径”两行内容，但没有同步适配 `el-tree-node__content` 高度和对齐，导致长目录树下出现文本重叠和错位。
2. 目录树默认 `default-expand-all`，笔记本、文档、路径、数量一次性铺开，信息密度太高，导致即使没有 CSS bug 也很难快速定位目标。
3. “当前选择”只有一行文案，没有独立选中卡片和搜索辅助，用户点击树节点后仍然很难快速确认自己到底选中了哪里。

**解决**:
1. 在 `pmSiyuan.ts` 中补充位置选择器辅助纯函数：目录树过滤、搜索态展开 key 计算、位置目标文案和路径文案格式化。
2. `PmPanel.vue` 的位置选择弹窗重做为“搜索 + 当前选择卡片 + 默认折叠树”的轻量目录选择器：
   - 顶部增加搜索框和当前选择卡片
   - 树默认只展开笔记本一级
   - 搜索时仅保留命中的节点和祖先路径，并自动展开必要分支
   - 完整路径从树节点主体中移走，集中在当前选择卡片里展示
3. 树节点改为单行标题优先展示，并通过 `:deep(.el-tree-node__content)` 覆盖最小高度、hover、current 状态和标签区宽度，彻底消除原来的重叠。
4. 顺手补齐思源配置卡片、页面关联列表和详情区的样式，让 PM 内思源相关 UI 视觉上统一，不再是多块临时样式拼接。
5. 在 `pmSiyuan.test.ts` 中补充过滤树、展开 keys 与位置展示文案测试，避免后续再改树结构时回归。

**关键点**:
1. 给 `el-tree` 做自定义多行节点时，不能只改 slot 模板，必须同步检查 `el-tree-node__content` 的高度和对齐方式，否则很容易出现“节点内容高度没长，文本却长了”的重叠 bug。
2. 目录选择场景不要把路径信息塞到每个树节点里。更稳的做法是“树里看标题，选中区看完整路径”，让信息按任务拆层。
3. 搜索目录树时，前端直接派生一棵过滤树比依赖组件内部 filter 更容易控制“祖先保留 + 搜索态展开 + 清空后恢复默认折叠”这整套行为。

**涉及文件**:
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/utils/pmSiyuan.ts`
- `apps/desktop/src/utils/pmSiyuan.test.ts`

**验证**:
- `pnpm test src/utils/pmSiyuan.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-29: 项目管理接入思源配置与目录树预览首版

**场景**: 用户希望在项目管理面板内新增思源设置，先完成第一版：配置本地思源地址和 API Token，验证连接，并读取“笔记本 + 文档树”目录数据。

**问题**:
1. 项目管理当前只有本地 PM 数据，没有任何外部知识库/文档系统接入入口。
2. 配置层已有 `user_settings`，但缺少 PM 场景内的局部设置 UI 和对应后端 action。
3. 思源 API 虽有官方接口可查版本、列笔记本和执行 SQL，但需要在 Rust 侧统一处理地址归一化、鉴权失败、超时、标准响应解析和目录树构建，不能把这些细节丢给前端。

**解决**:
1. 在 `PmPanel.vue` 顶部工具栏新增“思源设置”按钮，使用 `el-drawer` 承载地址、Token、保存、测试连接、加载目录和树预览；配置通过 `useSettings` 持久化到 `user_settings`。
2. 在 `bridge/tauri.ts` 增加 `tool:pm:siyuan-test` 与 `tool:pm:siyuan-directory` 两条通道，在 `types/pm.ts` 中补齐目录树类型定义，前端只负责展示和局部状态管理。
3. 在 `pm.rs` 中新增思源 helper：地址归一化、设置兜底读取、HTTP POST 封装、思源标准响应解析、401/403 与业务错误分类、`lsNotebooks + query/sql` 查询，以及按 `hpath` 构造树。
4. 目录树第一版使用 `blocks` 表中 `box / path / hpath / content` 查询文档节点，并在 Rust 内存中按路径分段逐层插入，最终返回前端可直接渲染的树结构。
5. 补充 Rust 单测覆盖 URL 归一化与目录树嵌套构建，减少后续重构时的回归风险。

**关键点**:
1. 思源配置虽然属于“设置”，但第一版放在 PM 面板内更贴近业务场景；后端 action 仍应归在 `pm` 域，而不是塞进通用 `settings` 域。
2. `user_settings` 足够承接这类轻量配置；若只是地址和 Token，不需要为了第一版额外建表。
3. 思源标准响应的错误不能只看 HTTP 状态码，还要看 JSON 中的 `code/msg`；连接失败、鉴权失败和业务错误要分开提示。
4. 前端刷新目录失败时应保留上一次成功树，避免一次请求失败把整个预览区清空。
5. 第一版目录树基于 `query/sql` 的字段假设构建，后续若思源版本字段结构变化，优先在 Rust 侧兜底或报出明确兼容性错误。

**涉及文件**:
- `docs/superpowers/specs/2026-03-29-pm-siyuan-integration-v1-design.md`
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/types/pm.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src-tauri/src/tools/pm.rs`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path "E:/Projects/LazyCat/apps/desktop/src-tauri/Cargo.toml"`
- `cargo test --manifest-path "E:/Projects/LazyCat/apps/desktop/src-tauri/Cargo.toml" siyuan_`

**使用次数**: 0

## 2026-03-29: 项目管理甘特图悬浮卡越界与右键视口重置修复

**场景**: 用户反馈项目管理甘特图在底部任务上悬浮详情时会被容器裁切，且右键任务打开菜单后甘特图视口会跳回默认位置。

**问题**:
1. `frappe-gantt` 默认 popup 只按 `left = x + 10`、`top = y - 10` 粗放定位，不会根据容器当前滚动视口做边界翻转，底部任务的详情卡容易被 `gantt-container` 裁掉。
2. `PmGanttView.vue` 里 `ganttTasks` 之前把 `selectedItemId` 也作为依赖，右键选中任务时会触发一次整图 `refresh()`。
3. `frappe-gantt` 的实际滚动容器是内部 `.gantt-container`，不是外层 `ganttRef`；之前即便尝试保留滚动位置，也读写错了元素。

**解决**:
1. 在 `utils/pmGantt.ts` 新增 `clampPmGanttPopupPosition()`，统一计算 popup 在当前视口内的左右/上下翻转与边距钳制，并补充单测覆盖底部和右侧越界场景。
2. `PmGanttView.vue` 改为通过 `MutationObserver` 观察 `.popup-wrapper` 的显隐与内容变化，在每次显示后按内部 `.gantt-container` 的 `scrollLeft/scrollTop/clientWidth/clientHeight` 重算 popup 位置。
3. 将甘特条“选中态”从 `ganttTasks` 计算链路里剥离，改为单独 watch `selectedItemId` 并只同步 class，避免右键时无谓刷新。
4. 甘特图数据确实需要 `refresh()` 时，先读取内部 `.gantt-container` 的视口位置，并临时关闭 `frappe-gantt` 的 `scroll_to` 自动定位，再在下一帧恢复滚动条。
5. 内部滚动事件改为直接绑定 `.gantt-container`，确保滚动时能及时关闭 popup 和右键菜单。

**关键点**:
1. `frappe-gantt` 的外层宿主元素只是挂载点，真正滚动的是内部自建容器；凡是涉及视口恢复、滚动监听、popup 可见区判断，都必须基于 `.gantt-container`。
2. “选中态变化”不能混进“任务数据变化”刷新链路，否则右键、点击切换详情都会触发整图重绘，带来滚动抖动和定位回跳。
3. 第三方库的 popup 若无法直接配置碰撞检测，优先在项目层补一层纯函数定位和 DOM 观察，不要急着 fork 依赖。

**涉及文件**:
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/utils/pmGantt.ts`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/pmGantt.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-29: 项目管理甘特图交互增强与甘特条右键菜单

**场景**: 用户希望优化项目管理甘特图视图，并为甘特图条目补上右键菜单，让甘特图和现有看板卡片的快捷操作能力对齐。

**问题**:
1. `PmGanttView.vue` 之前只有 `点击选中 + 拖动改日期`，没有悬浮信息、双击编辑，也没有甘特条自己的右键菜单。
2. `frappe-gantt` 暴露给项目的类型声明很薄，缺少 `popup`、`on_double_click`、`change_view_mode(..., maintain_pos)` 等当前实现会用到的钩子描述。
3. `PmPanel.vue` 之前只给看板卡片和左侧项目列表做了右键菜单，菜单定位还是硬编码近似值；甘特图如果直接复用旧入口，目标选中态和菜单目标容易不同步。

**解决**:
1. 在 `PmGanttView.vue` 中增加悬浮详情卡、双击编辑、甘特条 `contextmenu` 事件代理、滚动时关闭浮层，并把总览模式下的项目元信息一并透给 popup。
2. 新增 `utils/pmGantt.ts` 与 `pmGantt.test.ts`，把甘特任务映射、未排期统计、逾期/置顶/选中 class 组装、悬浮卡 HTML 生成收敛成纯函数。
3. 将菜单坐标钳制能力抽为通用 `utils/contextMenu.ts`，`PmPanel.vue` 和 `TodoPanel.vue` 共用；`PmPanel.vue` 的项目管理右键菜单统一接入 `Esc / 外部点击 / scroll / resize / 再次右键` 关闭规则。
4. `PmPanel.vue` 中抽出统一的工作项菜单动作构造逻辑，甘特条和看板卡片共用 `编辑 / 置顶或取消置顶 / 推进状态 / 删除`，并在打开菜单前同步切换当前选中工作项。
5. 补齐 `frappe-gantt.d.ts` 的 popup / 双击 / 视图切换类型，避免甘特图实现继续依赖隐式 `any`。

**关键点**:
1. 甘特图库的右键菜单不要强依赖原生 `MouseEvent` 透传；对子组件发 `{ item, anchorX, anchorY }` 这类稳定 payload，更利于父层复用菜单定位逻辑。
2. 甘特图条目的“选中态”最好作为任务 class 的一部分统一进入 `refresh()` 链路，不要在父层和子层各维护一套视觉状态。
3. 菜单定位不要继续靠 `actions.length * 34` 这类散落硬编码；抽成纯函数后，Todo/PM 两边能共享同一组边界测试。

**涉及文件**:
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/types/frappe-gantt.d.ts`
- `apps/desktop/src/utils/contextMenu.ts`
- `apps/desktop/src/utils/contextMenu.test.ts`
- `apps/desktop/src/utils/pmGantt.ts`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop test src/utils/contextMenu.test.ts src/utils/pmGantt.test.ts`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-03-20: 密码库解锁顺滑度优化首轮落地

**场景**: 用户希望密码库在输入正确主密码后更快出现可用主界面，同时首轮只做前端体感优化与后端低风险性能优化，不调整 PBKDF2 参数、不改现有 vault IPC 协议。

**问题**:
1. `VaultLockScreen.vue` 之前使用 500ms debounce + 按密码长度去重，自动解锁会产生固定空等，且自动/手动请求状态彼此割裂。
2. `VaultPanel.vue` 之前要等 `loadEntries()` 和 `loadTagStats()` 串行完成才真正显示主界面，首屏还叠加 `out-in` 过渡和列表逐项延迟动画，放大了“解锁后还在等”的体感。
3. `helpers.rs` 每次开库都会重新解析 `config.json` 且执行完整 schema 初始化；`vault.rs` 的 `cmd_list()` 对 tags 存在 N+1 查询，`cmd_touch()` 在高频续活路径里重复开库读取配置。

**解决**:
1. 在 `VaultLockScreen.vue` 中保留 `onUnlock()` / `attemptAutoUnlock()` 入口，内部统一走 `runUnlockAttempt()`；自动解锁改为 150ms debounce + 按密码值去重，并允许真并发请求，只有最近一次手动失败才回写错误。
2. 在 `VaultPanel.vue` 中把“已解锁”和“数据已加载”拆成两个阶段：先切 unlocked 主界面，再后台 `loadEntries({ phase })`，列表成功后 fire-and-forget 触发 `loadTagStats({ phase })`；首屏补了轻量 loading / 最小重试 UI，并用 generation + request token 丢弃旧结果。
3. 去掉锁屏到主界面的 `mode="out-in"` 和列表逐项 `animationDelay`，保留必要但不阻塞首屏可用性的轻量动效。
4. 在 `helpers.rs` 中把 `get_data_dir()` 做进程级缓存，并把数据库初始化拆成“每连接执行 `PRAGMA foreign_keys = ON`”与“进程首次连接执行 schema/FTS/seed 初始化”。
5. 在 `vault.rs` 中为 `cmd_list()` 增加批量 tags 查询映射，避免逐条查 tags；`cmd_touch()` 只更新 session 的 `last_activity`，不再走热路径数据库 I/O。

**关键点**:
1. 真并发 unlock 的收敛规则要清晰：任意一次成功即可进入 unlocked，旧失败不能覆盖新状态，自动失败继续静默。
2. `VaultPanel.vue` 的首次加载和普通刷新必须分开：首次加载允许显示占位与最小错误 UI，普通刷新保留旧列表和旧标签，不回退首屏态。
3. 旧异步结果一定要做代际保护，否则锁定后晚到的 `list/tag-stats` 或连续刷新里的旧结果会回写新页面。
4. `PRAGMA foreign_keys = ON` 属于连接级设置，不能跟 schema 一次化一起粗暴搬走；schema 初始化失败也不能缓存失败结果。
5. 已解锁会话内的锁定策略要和后端 session 保持同一口径：当前会话冻结既有策略，下一次 setup/unlock/change_password 再刷新。

**涉及文件**:
- `apps/desktop/src/components/VaultLockScreen.vue`
- `apps/desktop/src/components/VaultPanel.vue`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/vault.rs`
- `docs/superpowers/specs/2026-03-20-vault-unlock-smoothness-design.md`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path "E:/Projects/LazyCat/apps/desktop/src-tauri/Cargo.toml"`


## 2026-03-20: 主呼出快捷键优先按剪贴板路径打开资源管理器

**场景**: 用户希望按主呼出快捷键后，如果当前剪贴板内容本身是本地文件/目录路径，则优先直接打开系统文件浏览器；文件要定位选中，目录要直接打开，且不能影响 snippets / vault / todo 等命名快捷键导航。

**问题**:
1. 现有主快捷键 `toggle` 只负责呼出窗口，没有给前端一个仅属于主呼出场景的处理入口。
2. 前端现有 `clipboard-detect.ts` 只做 JSON/JWT 等内容识别，没有独立的保守路径归一化能力。
3. `inbox.rs` 的 `action_open_path()` 在 `reveal=true` 时仅打开父目录，Windows 下不会在资源管理器中选中文件。

**解决**:
1. 在 `main.rs` 的主快捷键 Reveal 分支中，仅对 `toggle` 额外发出 `main-window-toggle` 事件；命名快捷键仍继续走 `hotkey-navigate`，避免语义串线。
2. 在 `clipboard-detect.ts` 新增 `detectClipboardPath()`，只接受单行绝对路径 / UNC / file URI，支持一层外层引号，拒绝环境变量、多行和明显命令片段。
3. 在 `App.vue` 监听 `main-window-toggle`，读取 `navigator.clipboard.readText()` 后优先调用 `invokeToolByChannel("tool:inbox:open-path", { path, reveal })`；命中成功即短路，失败则静默回退原行为。
4. 在 `inbox.rs` 中保留现有存在性校验与目录打开逻辑，仅对 Windows + `reveal=true` + 文件路径改为 `explorer.exe /select,...`，实现真正的文件定位。
5. 增加 `clipboard-detect.test.ts`，覆盖文件路径、目录路径、引号、file URI、UNC 与拒绝样例。

**关键点**:
1. 主呼出与命名快捷键必须分事件处理，不能把 `toggle` 混进现有导航语义，否则容易误触发工具切换。
2. 前端路径检测应保持保守，只做格式归一化；路径是否真实存在仍交给 Rust 侧统一校验，避免误判普通文本。
3. 判断 `reveal` 不应依赖“是否含扩展名”，否则像 `C:\Windows\System32\drivers` 这类无扩展名文件会被误当目录；更稳妥的做法是“非根路径且不以斜杠结尾则 reveal=true”，再由后端基于真实文件系统类型分流。
4. Windows 文件定位优先走资源管理器原生 `/select,` 语义，目录和非 Windows 平台继续复用 `open::that(...)`，改动最小且兼容性最好。

**涉及文件**:
- `apps/desktop/src/App.vue`
- `apps/desktop/src/utils/clipboard-detect.ts`
- `apps/desktop/src/utils/clipboard-detect.test.ts`
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src-tauri/src/tools/inbox.rs`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`

**使用次数**: 0

---

## 2026-03-20: 本地待办最近一周已办改为真实完成时间 + 过去 7 天口径

**场景**: 用户反馈“任务清单”里的“最近一周已办”不符合预期，要求按过去 7 天滚动窗口计算，且已办归类必须基于真实完成时间，不能被完成后的再次编辑影响。

**问题**:
1. `TodoPanel.vue` 当前走 `item_list -> groupTodoItemsByBucket` 的前端分桶链路，但 `todoBuckets.ts` 之前用 `updatedAt || createdAt` 近似完成时间，完成后再编辑会把事项错误挪进/挪出“最近一周已办”。
2. 旧“最近一周”口径是“从最近周五开始”，不是严格过去 7 天窗口，导致边界日期与用户直觉不一致。
3. `todo_items` 表没有独立 `completed_at` 字段，后端列表接口也就无法给前端提供稳定的真实完成时间。

**解决**:
1. 在 `helpers.rs` 增加 migration 27：为 `todo_items` 新增 `completed_at`，给历史 `status='completed'` 数据用当前 `updated_at` 回填一次，并创建索引。
2. `todo.rs` 的 `item_change_status()` 改为：状态切到 `completed` 时仅在 `completed_at` 为空时写入 `CURRENT_TIMESTAMP`；切回非完成态时清空 `completed_at`，同时保持原有 `updated_at` 刷新语义。
3. `todo.rs` 的 `item_list()` 新增返回 `completedAt`，前端 `TodoItem`、`normalizeTodoItem()`、`relativeDoneTimeLabel()` 全链路改为消费真实完成时间。
4. `todoBuckets.ts` 改为以 `completedAt` 做已办分桶与倒序排序，“最近一周”起点收口为“今天向前 6 天的零点”；缺少 `completedAt` 的旧数据统一落到较早已办，避免继续被 `updatedAt` 污染。
5. `todoBuckets.test.ts` 补充“编辑已完成事项不会改变分桶”“缺少 completedAt 的已办不进入最近一周”等回归用例。

**关键点**:
1. `updated_at` 仍然应该保留“最后编辑时间”语义，不要再把它混用成完成时间，否则后续任何已完成事项编辑都会再次破坏分桶稳定性。
2. 对历史数据无法恢复真实完成时刻时，宁可保守回填一次并让后续新数据准确，也不要继续在前端做 `updatedAt` fallback。
3. 这类时间口径修正优先沿用现有列表接口和前端分桶入口，只替换真源字段与边界规则，能显著降低无关回归。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/todoBuckets.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml todo:: -- --nocapture`（本次被与改动无关的既有 `vault.rs` 测试编译错误阻塞）

**使用次数**: 0

## 2026-03-18: 本地待办卡片右键菜单落地

**场景**: 用户要求给本地待办的待办卡片补上右键菜单，支持 `置顶/完成/删除/编辑任务时间`，并保持与现有右侧详情区行为一致。

**问题**:
1. `TodoPanel.vue` 原本只有点击选中、双击编辑，置顶/完成/删除都集中在右侧详情区，列表卡片没有任何上下文菜单状态。
2. 待办右键菜单如果直接写死坐标，靠近窗口右下角时会出屏；仓库里虽然有多处自绘菜单，但 Todo 没有复用层。
3. “编辑任务时间”不能新开一条临时保存链路，否则会和现有 `ensureDetailCanLeave()`、右栏编辑态、重复事项删除范围确认产生交叉行为。

**解决**:
1. 在 `TodoPanel.vue` 的待办卡片补 `@contextmenu.prevent`，右键时先走 `ensureDetailCanLeave()`，再切到目标事项的详情查看态并打开菜单。
2. 新增 `apps/desktop/src/utils/todoContextMenu.ts`，抽出纯函数 `clampContextMenuPosition()` 统一处理菜单坐标钳制；`todoContextMenu.test.ts` 覆盖正常位置、右下出屏、左上边距和菜单大于视口四个核心场景。
3. 菜单浮层通过 `Teleport to="body"` 渲染，统一支持点击外部关闭、再次右键关闭、`Esc` 关闭、列表滚动关闭和组件卸载清理监听。
4. 菜单动作全部复用现有链路：`toggleItemPin()`、`changeItemStatus()`、`deleteItem()`，`编辑任务时间` 新增 `enterEditTimeMode()`，内部继续复用 `enterEditMode()`，并强制展开“日期与时间”区域后滚动定位和聚焦首个输入框。

**关键点**:
1. 右键目标切换前必须先处理脏编辑态；否则从创建态/编辑态直接右键其它事项，会把旧草稿和新菜单操作混在一起。
2. “编辑任务时间”最好只是**进入现有编辑态并定位字段**，不要额外做轻量弹窗或独立保存逻辑，这样可以继续复用现有校验、5 分钟刻度和保存语义。
3. Todo 的右键菜单不值得先抽公共组件；先在面板内局部落地，真正抽象的只有坐标钳制纯函数，避免把 UI 生命周期和业务动作耦到一起。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoContextMenu.ts`
- `apps/desktop/src/utils/todoContextMenu.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/todoContextMenu.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-18: 本地待办 meta-time 跨自然周文案修复

**场景**: 用户反馈任务清单中 `meta-chip / meta-time` 的相对日期文案有误。对于跨到下一自然周的事项，当前仍显示为 `周X`，例如本周设置下周的事项时应显示 `下周X`。

**问题**:
1. `TodoPanel.vue` 的 `relativeDateTimeLabel()` 写在组件内部，只按 `diffDays` 判断，没有按“周一开始”的自然周边界判断。
2. 当前逻辑会把“下周但不足 7 天”的日期误显示为 `周X`，而“刚好 +7 天”的日期又会直接退回绝对日期，表现不稳定。
3. 相对日期逻辑没有独立测试，涉及 `今天 / 明天 / 昨天 / 周X` 的文案调整时容易回归。

**解决**:
1. 新增 `apps/desktop/src/utils/todoRelativeDate.ts`，把相对日期文案提炼为纯函数 `formatTodoRelativeDateTimeLabel()`，统一处理 `今天 / 明天 / 昨天 / 周X / 上周X / 下周X / 绝对日期`。
2. 周边界改为按周一开始的自然周判断；相邻日计算使用 `setDate()`，避免继续依赖固定 `86400000` 毫秒偏移。
3. 对纯日期字符串 `YYYY-MM-DD` 做本地日期解析，避免被 `new Date()` 当作 UTC 导致跨天、跨周误判。
4. `TodoPanel.vue` 改为复用新 util，并删除未使用的 `itemTimeLabel()` 死代码。
5. 新增 `apps/desktop/src/utils/todoRelativeDate.test.ts`，覆盖同周、上周、下周、跨年回退和纯日期字符串等边界。

**关键点**:
1. 这类文案问题本质不是“差几天”，而是“是否跨自然周”；只看 `diffDays` 很容易把周三到下周一这种场景判错。
2. `今天 / 明天 / 昨天` 要高于 `上周 / 下周`，否则周日看次日周一会被展示成 `下周一`，不符合直觉。
3. 相对日期逻辑适合抽成 Todo 专用 util，不要塞进 `todoSchedule.ts` 或 `todoBuckets.ts`，否则职责会混在一起。

**涉及文件**:
- `apps/desktop/src/utils/todoRelativeDate.ts`
- `apps/desktop/src/utils/todoRelativeDate.test.ts`
- `apps/desktop/src/components/TodoPanel.vue`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/todoRelativeDate.test.ts`
- `pnpm --filter @lazycat/desktop typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-17: 收纳箱图片预览、右键菜单与图片回采抑制

**场景**: 用户要求在收纳箱图片详情里支持点击放大预览，并在右键菜单中提供复制图像、打开图像、打开图像位置、复制图像路径等常用操作。

**问题**:
1. 收纳箱详情里的图片已有 `payloadDataUrl` 和 `openPath`，但正文区只有静态展示，没有预览层和局部菜单交互。
2. 复制图像如果直接写回系统剪贴板，后台采集线程会把这张图再次录回历史流，形成“自复制回采”。
3. 前端右键菜单需要挂在图片正文和预览图上，同时还不能破坏现有三栏布局、滚动和详情切换体验。

**解决**:
1. 在 `InboxPanel.vue` 内直接加 `Teleport to="body"` 的预览层与自定义右键菜单，正文图和预览图共用一套菜单状态；关闭规则统一为遮罩点击、`Esc`、点击空白、滚动和窗口 resize。
2. 菜单动作固定收敛为 4 项：`复制图像 / 打开图像 / 打开图像位置 / 复制图像路径`；文件类动作统一走现有 `tool:inbox:open-path`，路径复制先走 `suppressClipboardCapture` 再写文本剪贴板。
3. Rust 侧在 `inbox.rs` 新增 `copy_image` action，Windows 下把图片文件解码后按 `CF_DIB` 写入系统剪贴板；抑制逻辑从“只压文本”升级为“按内容哈希压制”，文本和图片都共用同一份一次性抑制队列。

**关键点**:
1. 收纳箱图片详情不需要额外改表或改详情接口，现有 `payloadDataUrl + openPath + canOpenPath + metaJson.width/height` 已经够支撑预览和右键操作。
2. 图片回采抑制不能直接拿原文件字节做哈希；要按写入剪贴板后的像素内容重新编码 PNG 再算哈希，才能和后台 `read_image()` 重新采样出的 `content_hash` 对上。
3. 自定义右键菜单最稳妥的复用方式是沿用 `TabBar/VaultPanel` 那套 `reactive({ visible, x, y }) + Teleport + document click` 模式，不要额外引入新菜单依赖。

**涉及文件**:
- `apps/desktop/src/components/InboxPanel.vue`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src-tauri/src/tools/inbox.rs`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`
- `cargo check`

**使用次数**: 0

## 2026-03-17: 收纳箱（Inbox Hub）首版打通与跨工具草稿联动

**场景**: 需要按设计稿一次性落地收纳箱首版，包括 Rust 端剪贴板采集、SQLite 落库/搜索、前端三栏面板，以及转 Todo / Vault 的跨工具草稿联动。

**问题**:
1. Windows 剪贴板读取链路涉及 `windows-sys` 的 `HANDLE/HGLOBAL`、GDI 位图和注册格式，常量和句柄类型一旦用错，`cargo check` 会直接卡死在编译层。
2. 收纳箱面板需要同时承载筛选、分页、详情、元数据编辑和跨工具转入，如果直接把全部列表节点渲染到 DOM，历史量一大就会拖垮前端滚动体验。
3. Todo / Vault 不能只接字符串草稿；Todo 需要“首行标题、剩余正文进描述”，Vault 需要保守启发式预填并在复制敏感内容前抑制剪贴板回流。

**解决**:
1. Rust 端新增 `inbox` 工具域、migration 26 和主线程剪贴板轮询；Windows 剪贴板层统一改用正确的 `HANDLE`/`HGLOBAL` 调用签名，并用固定 `CF_*` 值避开 `windows-sys` feature 差异。
2. 前端新增 `InboxPanel.vue`，采用左栏筛选 + 中栏虚拟滚动摘要列表 + 右栏详情/动作的三栏布局，列表分页 50 条并通过固定行高虚拟化控制渲染节点数。
3. `useClipboardSuggestion` 的结构化 `PendingToolInput` 被接到 Todo / Vault：Todo 复用显式 `todoDraft` 并兜底文本拆分；Vault 在面板层做 URL/地址/端口/数据库关键词/显式标签行的保守解析，原文完整保留到备注；Vault 所有复制动作在写入系统剪贴板前先调用 `suppressClipboardCapture`。
4. 为避免验证噪音，顺手清理了 `vault.rs` 里既有的未使用 `Duration` warning。

**关键点**:
1. Windows 剪贴板 API 的 `GetClipboardData` 返回 `HANDLE`，后续 `GlobalLock/GlobalSize/GlobalUnlock` 必须按 `HGLOBAL` 指针语义使用，不能再把它当 `isize`。
2. 虚拟列表不必一开始就引完整库；对固定高度摘要卡片，用 `scrollTop + slice + spacer` 就能把 DOM 节点稳定压在可控范围内。
3. Vault 预填要宁缺毋滥：账号/密码优先取显式标签行，URL/主机/IP/端口/数据库类型可以推断，但原始全文必须完整进 `notes`，避免信息损失。
4. 新增异步组件后，`components.d.ts` 会在构建时自动补全，属于预期变更，不要误删。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/inbox.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src/components/InboxPanel.vue`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/components/VaultPanel.vue`
- `apps/desktop/src/components/VaultEntryDialog.vue`
- `apps/desktop/src/composables/useClipboardSuggestion.ts`

**验证**:
- `cargo check`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-16: release-all-win 正式发版校验、恢复路径与兼容性补强

**场景**: 在发布 `v0.2.5` 时，需要把“版本统一、正式发版、失败补跑”三类动作整理成稳定流程，并修复脚本里暴露出的兼容问题。

**问题**:
1. 旧脚本会在完整构建之后才暴露版本号不一致、tag 不匹配、工作区未提交等问题，失败成本高。
2. 便携包阶段默认从 `target/release/lazycat_lib.dll` 取 DLL，但当前 Tauri/Rust 产物实际可能输出到 `target/release/deps/lazycat_lib.dll`。
3. 某些 PowerShell 环境没有 `Get-FileHash`，会导致四个产物已经构建完成，却在 `SHA256SUMS.txt` 阶段失败。
4. 文档里只有零散打包说明，缺少“正式 GitHub Release 只能从 main 干净工作区发布”以及“失败后用 `-SkipBuild` 恢复”的明确规则。

**解决**:
1. `release-all-win.ps1` 增加正式发版前置校验：统一读取根 `package.json`、桌面端 `package.json`、`Cargo.toml`、`tauri.conf.json` 的版本，要求完全一致，且 `Tag` 必须等于 `v<version>`。
2. 正式上传路径增加 Git 约束：仅允许从 `main` 的干净工作区发布，发 tag 前先执行 `git push origin main`，并校验已存在 tag 必须指向当前 `HEAD`。
3. 便携包复制逻辑改为同时兼容 `target/release/lazycat_lib.dll` 和 `target/release/deps/lazycat_lib.dll`；哈希生成优先用 `Get-FileHash`，缺失时回退到 .NET `SHA256`。
4. 在 `finally` 中统一清理临时 stage 目录和临时离线配置；文档同步补充正式发版命令、本地出包命令和 `-SkipBuild` 恢复命令。

**关键点**:
1. “正式发版”与“本地出包”是两条不同路径：`-SkipUpload` 可以放宽 GitHub 上传约束，但不能绕过版本一致性校验。
2. 发布 tag 本质上是版本号的一部分，必须和 `tauri.conf.json` 对应版本一致，否则 Release 名称、产物名和源码版本会错位。
3. 构建已成功时不要重跑 10 多分钟的完整流程，优先使用 `pnpm release:all:win -- -Tag vX.Y.Z -SkipBuild` 补哈希或补上传。
4. 对仓库规范做增量补强时，要同步更新 `AGENTS.md` 与 `CLAUDE.md`，避免两份规则再次分叉。

**涉及文件**:
- `scripts/release-all-win.ps1`
- `AGENTS.md`
- `CLAUDE.md`

**验证**:
- `pnpm release:all:win -- -Tag v0.2.5 -SkipBuild -SkipUpload`
- `pnpm release:all:win -- -Tag v0.2.5 -SkipBuild`

**使用次数**: 0

## 2026-03-16: Windows 发版前先统一多处版本号，再走 release 脚本

**场景**: 需要发布 `v0.2.5` 到 GitHub Release，并产出 Windows 安装包与绿色包。

**问题**:
1. 本仓库版本号分散在根 `package.json`、`apps/desktop/package.json`、`apps/desktop/src-tauri/Cargo.toml`、`apps/desktop/src-tauri/tauri.conf.json`，历史上可能出现不一致。
2. `scripts/release-all-win.ps1` 会读取 `tauri.conf.json` 的 `version` 作为产物命名基础，如果只改 tag 不改配置，最终 Release 包名和 Git tag 会错位。
3. release 脚本只负责推送 tag 和上传 Release 资产，不会替代版本提交本身；版本变更必须先进入 Git 提交，再发版。

**解决**:
1. 发版前先统一桌面应用相关版本号到目标版本，至少同步根 `package.json`、桌面端 `package.json`、`Cargo.toml`、`tauri.conf.json`。
2. 先执行 `pnpm typecheck`、`pnpm --filter @lazycat/desktop build:web`、`pnpm test` 做基础校验，再执行 `pnpm release:all:win -- -Tag vX.Y.Z` 产出安装包、绿色包和 `SHA256SUMS.txt`。
3. 先提交版本变更并推送分支，再运行 release 脚本，让 tag、Release 和源码提交保持一致。

**关键点**:
1. 产物文件名跟随 `tauri.conf.json` 的版本，而不是 Git tag。
2. 如果要让远端 `main` 与 Release 对齐，不能只推 tag，还要推送当前分支提交。
3. `release-all-win.ps1` 已内置 slim/full 安装包、portable zip、SHA256 和 `gh release create/upload` 流程，优先复用，不要手工拼装发布步骤。

**涉及文件**:
- `package.json`
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/src-tauri/tauri.conf.json`
- `scripts/release-all-win.ps1`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`
- `pnpm release:all:win -- -Tag v0.2.5`

**使用次数**: 0

## 2026-03-08: 本地待办清空日期时间后仍显示时间的修复

**场景**: 用户反馈新建单次事项时已经手动清空日期和时间，但保存后列表中仍然显示时间，且时间相关统计也可能受影响。

**问题**:
1. Rust `row_to_task_json()` 之前会把任务的 `displayAt` 从 `eventAt` 回退到 `updatedAt`，导致“无日程事项”被伪装成有时间事项。
2. 前端 `TodoPanel.vue` 的时间列、今日到期和逾期判断都直接使用 `eventAt || displayAt`，一旦 `displayAt` 被污染，就会把更新时间当成日程时间展示。
3. `todoBuckets.ts` 的活跃项排序也依赖 `eventAt || displayAt`，会让无时间的单次事项因为伪 `displayAt` 提前排序。

**解决**:
1. Rust 侧收紧 `displayAt` 语义：普通任务/周期实例的 `displayAt` 只等于真实 `eventAt`，不再回退到 `updatedAt`；`item_sort_time()` 也移除 `updatedAt` fallback。
2. 前端新增统一 helper 区分真实日程时间：普通任务/周期实例只看 `eventAt`，周期根记录才看 `displayAt`，并让时间列、今日到期、逾期判断统一复用这层语义。
3. `todoBuckets.ts` 同步按同一规则排序：单次事项没有 `eventAt` 就按“无时间”处理排到末尾，周期根仍可按 `displayAt` 排序。
4. 为避免回归，补充 Rust 定向测试与 `todoBuckets.test.ts`，覆盖“无 `eventAt` 的单次事项不应产生伪 `displayAt`”和“周期根仍可按 `displayAt` 排序”。

**关键点**:
1. `displayAt` 应该只承担“真实可展示的日程时间”语义，不能混入 `updatedAt` 这类元数据字段。
2. 周期根记录是例外：它没有 `eventAt`，但需要用 `displayAt` 展示下一次发生时间；修复时要只收紧单次事项/实例语义，不能误伤周期根。
3. 一旦前后端都存在 `eventAt || displayAt` 这类松散回退，修复必须同时收口映射、展示和排序，否则很容易出现“显示改对了但排序还错”的半修状态。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`

**验证**:
- `pnpm test src/utils/todoBuckets.test.ts`
- `pnpm test`
- `cargo test item_sort_time_should_ignore_updated_at_fallback -- --nocapture`
- `cargo test task_row_without_event_at_should_not_emit_display_at -- --nocapture`
- `cargo check`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-08: 本地待办新增/编辑体验收口

**场景**: 用户要求按既定方案优化本地待办新增/编辑弹窗，包括默认日期时间、日期/时间清空联动、提醒排序、分类排序，以及放开内置分类删除限制。

**问题**:
1. `TodoPanel.vue` 里默认日期、默认时间、提醒状态和重复规则时间分别散落在 `resetItemDraft()`、时间拆分 computed 与保存校验中，直接局部改动很容易出现“UI 能清空、保存却报错”的状态撕裂。
2. 时间输入当前通过 hour/minute 双 `el-select` 拼接，`splitDraftEventTime()` 对空值默认回填 `09:00`，如果不先改这里，任何“清空时间”的交互都会被马上回弹。
3. 分类下拉排序之前已经有一份 `sortedTypes` 计算属性，新增同名实现会直接让 `build:web` 在 SFC 编译阶段报重复声明。
4. Rust `type_delete()` 之前把 builtin 限制和“类型是否存在”校验绑在一起，放开删除时要只移除 builtin 拦截，不能把不存在校验一并丢掉。

**解决**:
1. 在 `todoSchedule.ts` 新增 `getCreateDraftDefaultDateTime()`，统一返回“明天日期 + 下一档 5 分钟时间”；`TodoPanel.vue` 的 `itemDraft` 初始值和 `resetItemDraft()` 都只从这个 helper 取默认值。
2. `TodoPanel.vue` 中将日期清空和时间清空统一收口到 `clearEventSchedule()`：同时清空 `eventDate/eventTime`，并把提醒重置为 `none`；时间选择器保留双下拉，但改为显式“清空”按钮。
3. `splitDraftEventTime()` 先识别空字符串，再由 `composeDraftEventTime()` 负责重组 hour/minute，避免清空后被 `09:00` 兜底；保存时新增“日期和时间必须同时填写或同时清空”的校验。
4. 复用已有 `sortedTypes` 计算属性，只调整模板消费和排序规则；构建报重复声明时先全局搜同名符号，再删除重复块，比盲目重写安全。
5. `todo.rs` 的 `type_delete()` 改为先用 `SELECT 1` 校验类型存在，再继续执行解绑任务/模板与删除流程，从而放开 builtin 删除但保留原有错误语义。

**关键点**:
1. “默认时间”要先明确是“向上取整”还是“下一档 5 分钟”；这次按方案示例收口为“下一档”，因此 `14:55 -> 15:00`。
2. 清空日期/时间时必须同步处理提醒，否则保存校验会因为残留提醒而继续要求填写日程。
3. 前端 build 报 `Identifier ... has already been declared` 时，优先搜同名 computed / const，通常比模板本身更快定位。
4. Rust 定向测试失败时要区分“编译是否通过”和“现有红测是否无关本次改动”；本次通过 `cargo check` 单独确认了编译状态。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoSchedule.ts`
- `apps/desktop/src/utils/todoSchedule.test.ts`
- `apps/desktop/src-tauri/src/tools/todo.rs`

**验证**:
- `pnpm test src/utils/todoSchedule.test.ts`
- `pnpm test`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check`
- `cargo test todo:: -- --nocapture`（存在 1 个与本次改动无关的既有失败：`convert_one_off_task_to_recurring_should_bind_existing_task_without_duplicate`）

**使用次数**: 0

## 2026-03-08: 本地待办置顶排序与已办倒序收口

**场景**: 用户要求参照现有方案，给本地待办补上事项置顶能力，并同步收敛待办/已办列表的排序和操作列行为。

**问题**:
1. `TodoPanel.vue` 的待办列表里混有普通事项与周期根记录，若直接给所有行都接“置顶/取消”或“取消”动作，会把周期根记录误走到 `todo_tasks` 更新链路。
2. `groupTodoItemsByBucket` 之前只负责活跃项按时间升序分桶，已办项没有显式排序，状态切换后展示顺序容易和“最近完成优先”预期不一致。
3. 前后端都各自有一层排序：Rust `task_list/sort_item_rows` 与前端 `todoBuckets.ts`。如果只改一侧，刷新前后顺序会抖动。

**解决**:
1. `types/todo.ts` 为 `TodoItem` 新增 `pinned` 字段，`TodoPanel.vue` 的 `normalizeTodoItem` 接入该字段，待办标题补“置顶”标签，待办操作列新增“置顶/取消置顶”。
2. 前端只对可执行事项显示置顶与取消按钮；周期根记录继续保留编辑/删除路径，避免误调用任务状态或置顶接口。
3. `todoBuckets.ts` 将待办排序改为“`pinned` 优先，其次 `eventAt || displayAt` 升序”，已办改为按 `updatedAt` 倒序；`todoBuckets.test.ts` 补上置顶优先与已办倒序断言。
4. `helpers.rs` 增加 migration 19，为 `todo_tasks` 增加 `pinned` 列；`todo.rs` 新增 `item_toggle_pin` action，并让 `task_list` / `row_to_task_json` / `sort_item_rows` 全链路识别置顶状态。

**关键点**:
1. 周期根记录来自 `todo_templates` 映射，不应复用 `todo_tasks` 的置顶更新语义；前端按钮显隐要按“是否可执行”而不是只看是否在待办区。
2. 已办倒序直接复用 `updatedAt` 作为完成时间代理，前提是状态变更 SQL 始终同步刷新 `updated_at`。
3. 列表排序需要前后端同时收口：后端保证初始返回稳定，前端保证筛选/分桶后顺序仍符合产品规则。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/todo.rs`

**验证**:
- `pnpm test src/utils/todoBuckets.test.ts`
- `cargo test todo:: -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-08: 本地待办提醒改为独立弹窗窗口

**场景**: 用户要求把本地待办的系统通知替换为右下角置顶的自定义提醒弹窗，并支持完成、知道了、稍后提醒等直接操作。
**问题**:
1. 旧链路是 Rust 调度后直接发系统通知，再向主窗口发 `todo-reminder-fired`，主窗口里又会弹一次 `ElNotification`，提醒展示分散且无法直接操作。
2. 独立 Tauri 窗口首次创建时，如果只依赖 `emit("reminder-push")` 推送数据，前端监听尚未挂载时容易丢掉首屏提醒。
3. 生产态弹窗复用同一个前端入口时，不能只依赖 URL query 判断视图，需要给前端一个稳定的“当前就是 reminder-popup”信号。
**解决**:
1. 在 `main.rs` 新增 `show_reminder_popup`、`position_reminder_popup` 与 3 个 popup command，scheduler 改为复用/创建 `reminder-popup` 窗口，并继续向主窗口发 `todo-reminder-fired` 仅用于刷新。
2. `ReminderDispatch` 增加 `priority` 字段，前端弹窗直接消费完整提醒 payload，展示优先级徽章与稍后提醒菜单。
3. 前端 `main.ts` 按 `view=reminder-popup` 或 `window.__LAZYCAT_VIEW__` 切换到独立的 `ReminderPopupApp` 入口，主应用 `App.vue` 删除旧的 `ElNotification` 逻辑。
4. 为避免首屏事件抢跑，Rust 在创建弹窗时通过初始化脚本写入 `window.__LAZYCAT_REMINDER_BOOTSTRAP__`，弹窗组件挂载后先吃这份初始队列，再监听后续 `reminder-push` 事件。
5. capability 为 `reminder-popup` 开窗并补上 `core:window:allow-close`，最后用定向 Rust 单测加 `pnpm typecheck` / `build:web` 完成联调验证。
**关键点**:
1. “替换系统通知”不等于只删掉通知调用；还要把提醒展示、操作命令、首屏补偿和主窗口刷新链路一起收口。
2. 独立弹窗首次打开时，初始化脚本 + 队列去重比单纯依赖页面加载完成后的事件更稳妥。
3. 主窗口的 `todo-reminder-fired` 在这次改造后只承担刷新职责，前端如果还保留旧 toast 监听，就会和新弹窗形成重复提醒。
**涉及文件**:
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/capabilities/default.json`
- `apps/desktop/src/main.ts`
- `apps/desktop/src/ReminderPopupApp.ts`
- `apps/desktop/src/components/ReminderPopup.vue`
- `apps/desktop/src/App.vue`
- `apps/desktop/src/types/todo.ts`

**验证**:
- `cargo test dispatch_due_reminders_should_include_priority_in_payload -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-08: 本地待办改为双区块展示并前端判定逾期

**场景**: 用户要求把本地待办从“超期事项 / 待办事项 / 已办事项”三段简化为“待办事项 / 已办事项”两段，同时把逾期判断前移到前端，并用复选框替代“完成”按钮。

**问题**:
1. `TodoPanel.vue` 的列表模板最初依赖 `itemSections` 循环渲染三段，若只改文案不改分桶结构，很容易留下旧列、旧按钮和重复规则副文案等残留 UI。
2. 后端 `isOverdue` 的语义只覆盖 `pending / in_progress + eventAt < now`；如果前端直接按“任意早于当前时间”打标，已办项也会误显示“逾期”。
3. `todoBuckets.ts` 旧实现依赖 `item.isOverdue` 分桶，但已有单测又要求“周期根事项仍归待办”，改结构时若不顺手修契约，相关回归会继续红。

**解决**:
1. `TodoPanel.vue` 改为两个固定 section：待办区直接展示 `activeItems`，已办区默认折叠，点击标题切换展开/收起。
2. 列表统一改成 7 列：复选框、事项、时间、分类、优先级、执行人、操作；删除状态列、时间副文案和“完成”按钮，改用复选框切换 `completed/pending`。
3. 前端新增 `isItemOverdue()`，仅对 `pending / in_progress` 且 `eventAt || displayAt < now` 的事项显示“逾期”，避免已办项误标红。
4. `todoBuckets.ts` 改为返回 `{ activeItems, doneItems }`，`activeItems` 统一按 `eventAt || displayAt` 升序排序、无时间项排最后，并保留周期根事项进入 activeItems 的 helper 契约。
5. `todoBuckets.test.ts` 同步改为断言新返回结构，并补上时间排序、`displayAt` 回退、无时间排尾和周期根事项归 activeItems 的覆盖。

**关键点**:
1. “前端接管逾期判断”不等于扩大逾期语义；要先锁定状态口径，否则视觉标记会和已有业务含义脱节。
2. 如果某个 helper 已被单测和历史经验共同约束，哪怕当前 UI 层暂时过滤了相关数据，也更适合保留 helper 契约，再由 UI 自己决定是否展示。
3. 计划文案里出现“6 列”但实际列举出 7 列时，应以明确列清单为准，避免把笔误实现成产品行为。
4. 清理展示辅助函数前，先确认它们是否还被弹窗编辑逻辑等其他路径复用；这次 `getItemRecurrence` 仍需保留，不能按列表清理思路直接删除。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`

**验证**:
- `pnpm test src/utils/todoBuckets.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-16: Tauri 自定义 manifest 不要与 embed-resource 并用

**场景**: 用户执行 `pnpm dev`，Rust/Tauri 在 Windows 链接阶段报 `link.exe failed: exit code: 1123`。
**问题**:
1. 表面错误是 `LNK1123`，但真正的首个致命错误是 `CVTRES : fatal error CVT1100: duplicate resource. type:MANIFEST, name:1`。
2. `build.rs` 手工用 `embed-resource` 生成了 `embed_manifest.lib`，同时 `tauri_build::build()` 在 Windows 下也会生成包含 manifest 的 `resource.lib`。
3. 两份 `MANIFEST` 资源同时链接进 exe，会导致资源转换阶段失败，最终表现成 `LNK1123`，容易被误判为 `link.exe` 或 OpenSSL 警告问题。
**解决**:
1. 删除 `build.rs` 中手工生成 `.rc` / `embed_manifest.lib` 的逻辑，不再直接调用 `embed-resource`。
2. 保留自定义 `lazycat.manifest` 内容，但改为通过 `tauri_build::WindowsAttributes::app_manifest(...)` 注入，让 Tauri 成为唯一的 Windows 资源编译入口。
3. 同步移除 `Cargo.toml` 里的 `embed-resource` build dependency，避免后续再次走回旁路方案。
**关键点**:
1. 遇到 `LNK1123` 时，先往前找 `CVTRES` 的第一条 fatal error，不要只盯着最后一行。
2. Tauri 2 在 Windows 下默认就会通过 `tauri-build` 生成资源文件；要自定义 manifest，应扩展它，而不是额外再编一份资源库。
3. `LNK4099` 这类 OpenSSL PDB 警告通常不是主因，先区分 warning 和真正 fatal error。
**涉及文件**:
- `apps/desktop/src-tauri/build.rs`
- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/src-tauri/lazycat.manifest`

**验证**:
- `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --no-default-features -vv`
- `pnpm dev`

**使用次数**: 0

## 2026-03-08: 本地待办移除提醒中心并改为超期/待办/已办三段

**场景**: 用户要求移除本地待办中的“提醒中心”功能，并将事项页面固定拆成“超期事项、待办事项、已办事项”三段展示。

**问题**:
1. `TodoPanel.vue` 之前通过“事项 / 提醒中心 / 基础数据”三个页签组织内容，提醒中心并不是独立工具，而是同一组件内的第二个视图，直接删除页签后需要同步清理前端状态和刷新链路。
2. 事项主列表原本是“单表 + 状态筛选”的模式，如果只是把表格视觉上复制三份，容易让筛选条件与三段口径互相打架，尤其是 `completed / canceled` 的归类口径。
3. 三段分组逻辑若继续写死在 `TodoPanel.vue` 模板内，后续很难单测，也容易在周期根事项是否算待办这类规则上再次回归。

**解决**:
1. 删除 `TodoPanel.vue` 中的提醒中心页签、未读提醒列表状态和已读操作，仅保留事项页与基础数据页；事项提醒、系统通知和“稍后10分钟”能力继续保留。
2. 事项页保留“事项视图”和关键词筛选，但去掉“状态筛选”，改为在过滤后的统一事项集合上固定分出三段：超期事项、待办事项、已办事项。
3. 新增 `src/utils/todoBuckets.ts`，将分段逻辑抽成纯函数 `groupTodoItemsByBucket`，明确规则：超期=`pending/in_progress + isOverdue`，待办=周期根事项或未超期的 `pending/in_progress`，已办=`completed + canceled`。
4. 新增 `src/utils/todoBuckets.test.ts` 覆盖超期、待办、已办、周期根归待办四个核心场景，保证 UI 重构后规则仍可独立验证。
5. `App.vue` 中同步调整本地待办入口文案，去掉“提醒中心”表述，避免功能描述与页面实际结构不一致。

**关键点**:
1. 这类“固定分段”页面最好采用“先统一过滤，再固定分桶”的顺序；如果把筛选与分桶交叉写，会很快出现某一段数据来源和用户预期不一致的问题。
2. 周期根事项虽然没有普通状态字段，但在产品语义上仍属于可管理的待办对象，应该显式纳入待办段，而不是依赖状态枚举自然落桶。
3. 移除提醒中心页签并不等于下线提醒能力；只删 UI、保留调度与系统通知，可以把这次改动控制在前端重排层，避免无谓扩大到后端存储退役。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`
- `apps/desktop/src/App.vue`

**验证**:
- `pnpm test src/utils/todoBuckets.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-08: 本地待办编辑态事项类型互转与 5 分钟步进

**场景**: 用户要求本地待办新增事项的时间选择按 5 分钟步进展示，并允许在编辑已有事项时切换“单次事项 / 周期事项”。

**问题**:
1. 前端 `TodoPanel.vue` 之前只是用 `ElTimePicker` 禁用非 5 分钟分钟项，交互上仍是完整 60 分钟列表，不符合“时间间隔为五分钟”的直观预期。
2. 编辑弹窗里“事项类型”单选在非创建态被禁用，已有事项无法直接在单次 / 周期之间切换。
3. 后端 `todo.rs` 的 `item_update` 同时承担单次事项、周期根记录、周期实例和 future scope 更新，若不先识别“当前持久化类型”和“本次目标类型”，很容易把跨类型变更误走成普通更新。

**解决**:
1. 前端把单次时间与周期时间输入切到 `ElTimeSelect`，统一使用 `00:05` 步进，让时间列表只展示 5 分钟粒度。
2. 编辑态放开“事项类型”单选；单次转周期时自动切到 `future_instances` 语义并展示周期规则区，周期转单次时根据编辑上下文回填单次时间锚点。
3. Rust 端为 `item_update` 增加“当前类型 vs 目标类型”识别；同类型继续走 `task_update` / `template_update`，跨类型则走专门转换分支。
4. 单次转周期时创建周期根记录，并把当前任务绑定成首个实例，同时把 `generated_count` 置为 1、`next_occurrence_at` 推到首个实例之后，避免保存后马上再生成一条重复实例。
5. 周期转单次时复用现有 `source_template_id` / `todo_templates` 结构：根记录或 future scope 场景创建/保留一条独立单次事项，再解绑或删除周期模板；当前实例直接转单次的老分支继续保留给后端兼容测试，但前端不默认暴露该入口。

**关键点**:
1. “5 分钟步进”最好直接落在控件层，而不是只靠保存前校验，否则体验上仍像 1 分钟粒度。
2. 单次转周期时如果当前任务直接保留为首个实例，模板的 `next_occurrence_at` 一定要从该实例之后开始算，否则调度器会补出一条同时刻重复实例。
3. 周期转单次在前端应优先走“根记录 / 此后未发生项”语义，避免用户误以为会重写整条周期历史。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/components.d.ts`
- `apps/desktop/src-tauri/src/tools/todo.rs`

**验证**:
- `cargo test todo:: -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`

**使用次数**: 0

## 2026-03-07: 本地待办多提醒与逐条稍后提醒改造

**场景**: 用户要求调整本地待办新增/编辑弹窗的字段顺序与创建态文案，同时把提醒从单选升级为多选，新增默认勾选的“准时提醒”，并让“稍后10分钟”只影响当前触发的那一条提醒。

**问题**:
1. 现有待办模型从前端类型到 Rust 调度都建立在“单事项仅一个提醒时间”的前提上，`todo_tasks.remind_at` 和 `todo_templates.reminder_offset_minutes` 都只能表达单提醒。
2. 提醒中心事件与系统通知只携带 `taskId`，没有具体提醒记录标识，多提醒下无法精确实现“逐条稍后”。
3. 前端 `TodoPanel.vue` 同时承担表单、列表展示和历史兼容解析，若不先统一提醒数组语义，`typecheck` 虽可能通过，但运行时容易出现“默认值/无提醒/旧数据回填”不一致。

**解决**:
1. 前端把 `reminderPreset` 全量切为 `reminderPresets`，增加 `0m` 准时提醒与互斥的 `none` 哨兵值；创建态默认 `['0m']`，编辑已有无提醒事项时显示为 `['none']`。
2. Rust 端在 `helpers.rs` 增加 migration 18，新增 `todo_task_reminders` 与 `todo_template_reminders` 两张子表，把旧单提醒数据迁移成单元素提醒集合，并为 `todo_reminder_events` 补 `task_reminder_id` 与 `reminder_preset`。
3. `todo.rs` 的任务创建、任务更新、周期模板创建、周期模板更新、周期实例生成、提醒派发与提醒中心列表统一改为围绕提醒子表工作；旧列保留但不再作为主真源。
4. “稍后10分钟”改为优先吃 `taskReminderId`；若列表按钮未显式传入，则后端自动选择该事项最近一条仍可触发的提醒。
5. 前端弹窗字段顺序调整为“提醒 → 事项类型 → 周期规则 → 描述”，创建态标题固定“新增事项”，提交按钮固定“创建事项”。

**关键点**:
1. `none` 只作为前端互斥选项存在，提交到后端时必须转为空数组，不能和真实提醒预设一起落库。
2. 多提醒的 `snooze_until` / `last_notified_at` 必须下沉到提醒子表；若继续复用任务表字段，会导致一条提醒被稍后后误伤同事项的其它提醒。
3. 周期实例生成时不要直接复制旧 `remind_at`，而是基于实例 `event_at` 和模板提醒偏移重新计算，才能保证多个提醒时间都正确。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/App.vue`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`

**验证**:
- `cargo test todo:: -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`

**使用次数**: 0

## 2026-03-07: 本地待办调度区重构为日期/时间/提醒/重复

**场景**: 用户要求把本地待办的新增/编辑弹窗重构为更接近日历应用的调度体验，核心围绕“日期、时间、提醒、重复”，并为周期事项补上显式开始日期。

**问题**:
1. 前端 `TodoPanel.vue` 已经拆出新草稿字段，但模板、调度工具和保存逻辑存在两套接口并存，容易出现导出名不一致与提交流程断链。
2. 后端周期模板此前没有 `start_at`，`next_occurrence_at` 默认从保存当下开始推算，无法表达“从指定日期开始重复”。
3. 工具函数与测试一度处于新旧命名混用状态，若不先收敛为单一真源，`typecheck` 与 `build:web` 很容易反复失败。

**解决**:
1. 前端把 `TodoPanel.vue` 的调度区统一为单次事项 `singleDate/singleTime` 与周期事项 `recurrenceStartDate/recurrenceTime/repeatPreset` 两套草稿，并在 `saveItem` 中映射为 `eventAt` 与 `recurrence.startAt`。
2. 新增 `src/utils/todoSchedule.ts` 作为调度规则单一真源，同时兼容旧测试接口和新面板接口，统一提供重复预设、日期时间拆装、规则摘要与结束条件格式化。
3. Rust 端在 `helpers.rs` 增加 migration 17，为 `todo_templates` 增加 `start_at` 并回填历史数据；`todo.rs` 的模板创建、更新、启停、实例生成全部改为尊重 `start_at`。
4. 周期规则继续沿用 simple/Cron 双轨，但简单月规则允许 31 号；前端对“每周自定义且间隔大于 1”直接提示改用高级 Cron，避免后端 silent ignore。

**关键点**:
1. `start_at` 是周期系列的生效下界，不等于 `next_occurrence_at`；创建时要按 `start_at` 首次计算，更新/启用时则按 `max(now, start_at)` 重算，避免重复补历史实例。
2. 若工具文件已演进过多轮，优先收敛为一个稳定导出面，再回头补 `TodoPanel.vue` 与测试，成本低于在两套接口之间硬凑兼容。
3. 图片里的“时间段”这轮不落数据模型，只保留具体时间；“不重复”在现有双模型里仍由单次事项承担，因此周期编辑态需要阻止直接改成不重复。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoSchedule.ts`
- `apps/desktop/src/utils/todoSchedule.test.ts`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`

**验证**:
- `pnpm typecheck`
- `pnpm test src/utils/todoSchedule.test.ts`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo test todo:: -- --nocapture`

**使用次数**: 0

## 2026-03-07: 本地待办工具（任务+周期+提醒）一体化落地

**场景**: 新增本地待办能力，要求支持任务类型、优先级、执行人、提醒、周期提醒与周期事件，并同时提供系统提醒与应用内提醒中心。

**问题**:
1. 现有仓库没有 `todo` 工具域，前后端通道、数据表、面板均为空白。
2. 需要兼顾单次任务与周期实例，且应用退出后重启要补偿错过提醒。
3. 系统提醒需与应用内提醒状态同步，避免重复提醒与丢提醒。

**解决**:
1. Rust 新增 `tools/todo.rs`，实现类型/执行人/任务/周期模板/提醒中心 action，及 `scheduler_tick` 调度入口。
2. `helpers.rs` 增加 migration 13，创建 `todo_*` 系列表并注入内置类型（待报事项、工作任务、会议安排、个人事项）。
3. `main.rs` 增加调度线程：每 30 秒执行周期实例生成 + 到期提醒派发；同时发送系统通知并 `emit(\"todo-reminder-fired\")` 给前端。
4. 前端新增 `TodoPanel.vue`，提供任务管理、周期管理、提醒中心与基础数据管理；`App.vue` 全局监听提醒事件并弹通知。
5. 通道映射与类型体系扩展：`bridge/tauri.ts` 新增 `tool:todo:*`，`types/todo.ts` 与 `types/index.ts` 新增导出，`tool-registry.ts` 注册 `todo` 面板。

**关键点**:
1. 周期模板统一存储 Cron 表达式，简单规则在保存时转换为 Cron，降低调度复杂度。
2. 提醒触发条件采用 `COALESCE(snooze_until, remind_at)` + `last_notified_at` 去重，支持“稍后提醒”复触发。
3. 为防止离线过久导致单轮阻塞，周期补偿每轮每模板最多生成 500 条实例，后续轮次继续补齐。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/App.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/types/index.ts`

**使用次数**: 0

## 2026-03-07: 密码库移除软锁并改为失焦仅隐藏敏感信息

**场景**: 将密码库从“敏感信息隐藏 → 软锁 → 硬锁”收敛为“敏感信息隐藏 → 硬锁”，同时保留失焦时的安全保护体验。

**问题**:
1. 软锁引入了额外状态、IPC 和元数据列表链路，前后端实现复杂度偏高。
2. 失焦锁定会打断当前上下文，用户更需要的是立即恢复掩码显示，而不是直接改变会话状态。
3. `show-password` 输入框的显隐状态由组件内部维护，仅清理外层状态无法在失焦时自动恢复掩码。

**解决**:
1. 后端移除 `soft_lock`、`list_metadata`、`vault_soft_locked` 和 `SoftLocked` 状态，统一只保留 unlocked / locked 两态。
2. 前端空闲计时器改为“到期隐藏敏感信息 + 到期直接硬锁”，失焦事件只执行敏感信息隐藏，不再触发锁定。
3. 为 `VaultPanel`、`VaultEntryDialog`、`VaultLockScreen` 的密码输入引入 `maskVersion` 重挂载机制，失焦时可恢复掩码显示且不清空已输入内容。

**关键点**:
1. “隐藏敏感信息”与“锁定会话”需要明确分层：前者只影响 UI 展示，后者才影响后端解锁态。
2. 失焦隐藏要覆盖列表明文、复制反馈和 `show-password` 组件内部显隐状态，否则体验会出现保护不一致。
3. 锁定预设继续复用 `vault_lock_profile`，仅保留隐藏时长和硬锁时长，避免再引入新的配置分支。

**涉及文件**:
- `apps/desktop/src/components/VaultPanel.vue`
- `apps/desktop/src/components/VaultEntryDialog.vue`
- `apps/desktop/src/components/VaultLockScreen.vue`
- `apps/desktop/src/components/SettingsPanel.vue`
- `apps/desktop/src/composables/useSettings.ts`
- `apps/desktop/src/utils/vaultLock.ts`
- `apps/desktop/src-tauri/src/tools/vault.rs`
- `apps/desktop/src/bridge/tauri.ts`

**使用次数**: 0

## 2026-03-07: 密码库分级锁定优先复用现有会话与设置通道
**场景**: 为密码管理增加“敏感信息隐藏 → 软锁 → 硬锁”的平衡方案，同时保留主密码为唯一解锁凭据。
**问题**:
1. 原实现只有固定 5 分钟硬锁，前端只有布尔锁定态，缺少软锁与预设配置。
2. `vault` 已经具备通用设置持久化、状态查询和会话内存密钥，不适合再造一套存储模型。
3. 软锁需要保留列表上下文，但现有 `list` 接口会解密并返回账号/摘要，不能直接复用到软锁态。
**解决**:
1. 设置层继续走 `user_settings`，新增 `vault_lock_profile`，前端通过 `useSettings` 提供统一读取与策略换算。
2. 后端会话保持“内存密钥 + 状态枚举”，新增 `soft_lock` / `touch` / `list_metadata`，并让 `status` 返回 `lockState`。
3. 前端在 `VaultPanel` 本地做空闲计时与失焦软锁，后端负责硬锁兜底；软锁时改走 `list_metadata` 仅返回非敏感字段。
4. 关闭到托盘时在 `main.rs` 直接调用 `tools::vault::force_lock()`，避免窗口隐藏后仍保留解锁态。
**关键点**:
1. 分级锁定里，“软锁保留上下文”与“硬锁清空会话密钥”要明确分工：前端保留视图，后端控制密钥生命周期。
2. 锁定预设尽量收敛为 `strict / balanced / convenient`，不要把秒数配置直接暴露给用户。
3. 若前端测试在沙箱内出现 `spawn EPERM`，按规范提权重跑即可，不要因为单次 EPERM 放弃验证。
**涉及文件**:
- `apps/desktop/src/components/VaultPanel.vue`
- `apps/desktop/src/components/SettingsPanel.vue`
- `apps/desktop/src/composables/useSettings.ts`
- `apps/desktop/src/utils/vaultLock.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src-tauri/src/tools/vault.rs`
- `apps/desktop/src-tauri/src/main.rs`

**使用次数**: 0

## 2026-03-07: 命名快捷键二次触发隐藏失败根因为缺少 `core:window:allow-hide`
**场景**: `snippets`、`launcher`、`vault` 通过命名快捷键呼出后，再次按下同一快捷键没有隐藏主窗口。
**问题**:
1. 前端热键监听已经命中隐藏分支，但 `appWindow.hide()` 在 Tauri 权限层被拒绝。
2. 日志报错明确提示缺少 `core:window:allow-hide`，导致看起来像“逻辑无效”，实际是权限不足。
**解决**:
1. 在 `apps/desktop/src-tauri/capabilities/default.json` 为主窗口补充 `core:window:allow-hide`。
2. 保留命名快捷键使用结构化 payload 的隐藏判定逻辑，清理仅用于排查的调试日志和设置项。
**关键点**:
1. Tauri 2 的窗口 API 即使前端调用命中分支，也可能因 capability 缺失而在运行时失败。
2. 这类问题应先看权限报错，再决定是否继续扩大逻辑排查范围。
**涉及文件**:
- `apps/desktop/src-tauri/capabilities/default.json`
- `apps/desktop/src/App.vue`
- `apps/desktop/src-tauri/src/main.rs`

**使用次数**: 0

## 2026-02-21: 添加 MDN JavaScript 中文手册（Puppeteer 抓取方案）

**场景**: 将 MDN JS 中文手册（https://developer.mozilla.org/zh-CN/docs/Web/JavaScript）添加为离线手册

**问题**:
1. MDN 是 React SSR + 客户端水合的 SPA，没有静态构建产物可直接使用
2. Yari（MDN 官方构建系统）整站产物数 GB，不现实
3. 页面路径无 `.html` 扩展名（如 `/zh-CN/docs/Web/JavaScript/Reference/Array`）
4. Windows 文件系统不支持 `*` 字符，5 个路径含星号的页面（如 `async_function*`）无法保存

**解决**:
1. 用 Puppeteer + 系统 Edge（`C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe`）抓取
2. 抓取脚本：`scripts/scrape-mdn-js.mjs`，使用 `createRequire` 导入 pnpm 本地 puppeteer
3. 无扩展名 URL 路径一律保存为 `<path>/index.html`（避免同名文件与目录冲突，ENOTDIR 错误）
4. 注册到 `manuals.rs`：`("mdn-js", "MDN JavaScript 手册", "/zh-CN/docs/Web/JavaScript/")`
5. `tauri.conf.json` 的 `bundle.resources` 已有 `**/*` 通配符，自动覆盖新手册目录

**关键点**:
1. `createRequire(import.meta.url)` 以脚本所在目录为基准解析相对路径，ESM 脚本中导入 CJS 模块的正确方式
2. SPA 路由的无扩展名路径必须保存为目录下 `index.html`，否则子路径写入时报 ENOTDIR
3. HTTP 服务器已处理无扩展名路径（`file_path.extension().is_none()` → 尝试加 `.html` 或 `index.html`），MDN 内链接直接可用
4. 含 `*` 字符的页面在 Windows 下无法保存，属于不可绕过的 OS 限制，影响 5 个页面，可忽略

**涉及文件**:
- `scripts/scrape-mdn-js.mjs`（新建，抓取脚本）
- `apps/desktop/src-tauri/src/tools/manuals.rs`（注册新手册）
- `resources/manuals/mdn-js/`（新建，872 个文件，72.3 MB）

**使用次数**: 0

## 2026-02-20: 六方案全量重构（类型集中化 + Composables + App.vue 拆分 + Rust 模块化 + 构建优化 + CSS 分层）

**场景**: 项目存在巨型 App.vue (1538行)、巨型 main.rs (1341行)、重复接口定义、Element Plus 全量导入、CSS 单文件、Monaco 主题不联动等6个架构问题

**问题**:
1. App.vue 1538行 60+ ref 21个 v-else-if，不可维护
2. Rust main.rs 59分支 match，1341行单文件
3. 9处接口重复定义
4. Element Plus 全量导入导致 index.js 999KB
5. styles.css 1447行单文件
6. Monaco 编辑器硬编码 `theme: "vs"`，不跟随 Dark/Light 切换

**解决**:
1. **类型集中化**: 新建 `src/types/` (tools.ts, hosts.ts, ports.ts, calc.ts, index.ts)，所有组件 import from `../types`
2. **Composables**: 新建 `src/composables/` (useToolInvoke.ts, useLocalStorage.ts, useFavorites.ts)
3. **App.vue 拆分**:
   - 新建 `tool-registry.ts`，用 `defineAsyncComponent` 映射工具ID到组件
   - 模板用 `<component :is="currentComponent" :key="activeTool" v-bind="currentComponentProps" />` 替代 21 个 v-else-if
   - 新建 12 个胖组件: RsaPanel, AesPanel, JsonXmlPanel, JsonYamlPanel, TextProcessPanel, EnvPanel, SplitMergePanel, ImagePanel, TimestampPanel, UuidPanel, CronPanel, SettingsPanel
   - 重写已有薄壳组件 (FormatterPanel, RegexPanel, HostsPanel, PortsPanel, CalcDraftPanel) 为胖组件，内化状态和 IPC 调用
   - App.vue: 1538行 -> 190行
4. **Rust 模块化**: 新建 `src-tauri/src/tools/` (18个文件: mod.rs, helpers.rs, encode.rs, convert.rs 等)
   - main.rs: 1341行 -> 311行
5. **构建优化**: 安装 `unplugin-vue-components` + `unplugin-auto-import`，配置 ElementPlusResolver 按需导入；配置 `manualChunks` 拆分 element-plus 和 monaco-editor
   - index.js: 999KB -> 20KB (element-plus 独立 415KB chunk)
6. **CSS 分层**: 拆分 styles.css 为 9 个文件 (tokens, reset, layout, sidebar, home, panels, element-overrides, responsive, theme-light)
   - MonacoPane: MutationObserver 监听 `data-theme` 切换 `vs`/`vs-dark`
   - 修复硬编码 `#dce3ef` -> `var(--lc-border)`

**关键点**:
1. Vue SFC 中不能对普通对象使用 v-model（SettingsPanel 的 isDarkMode），需要用 `:model-value` + `@update:model-value` 模式
2. `<component :is>` 的 v-bind 中可以传递 `onUpdate:xxx` 事件处理器实现双向绑定
3. Rust 模块化后编译器自动捕获所有错误，风险极低

**涉及文件**: App.vue, main.ts, vite.config.ts, styles.css, MonacoPane.vue, tool-registry.ts, src/types/*, src/composables/*, src/components/*Panel.vue (12新建+5重写), src/styles/* (10文件), src-tauri/src/tools/* (18文件), src-tauri/src/main.rs

**使用次数**: 0

## 2026-02-21: 代码片段页三栏拥挤治理与检索管理迭代（批量能力）
**场景**: 代码片段页在三栏结构下信息密度过高，检索与管理动作分散，缺乏批量处理能力，导致日常整理效率低。
**问题**:
1. 中栏仅有搜索和排序，缺少结果反馈与快速筛选。
2. 列表无法多选，无法批量收藏/移动/打标签/删除。
3. 前后端缺少批量操作接口，管理动作需要逐条执行。
4. 布局拥挤，列表与管理动作缺乏分层。

**解决**:
1. 前端中栏改造：
   - 增加“无标签/最近7天”快速筛选。
   - 增加结果计数与“清空筛选”。
   - 列表支持多选（checkbox）并保留单项点击编辑。
   - 增加底部批量操作条（收藏/取消收藏/移动到当前文件夹/添加标签/删除/清空选择）。
2. 前端状态逻辑增强：
   - 增加 `selectedIds` 多选状态与派生计数。
   - 增加 `quickFilter` 快速筛选状态。
   - 在 `loadSnippets` 中统一应用快速筛选，并同步清理不可见选中项。
3. 后端新增批量接口（事务）：
   - `batch_update`: 支持批量收藏、移动文件夹、添加/移除标签。
   - `batch_delete`: 支持批量删除片段。
4. IPC 通道映射新增：
   - `tool:snippets:batch-update` -> `batch_update`
   - `tool:snippets:batch-delete` -> `batch_delete`

**关键点**:
1. 批量更新必须校验 `ids` 非空且去重，且至少包含一个操作字段。
2. 批量写入使用数据库事务，避免部分成功导致状态不一致。
3. 列表筛选后要同步修正多选状态，避免“不可见项仍被批量操作”。
4. 批量移动采用“移动到当前选中文件夹”，无目标文件夹时提示用户先选择。

**涉及文件**:
- apps/desktop/src/components/SnippetPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src-tauri/src/tools/snippets.rs

**使用次数**: 0

## 2026-02-21: 代码片段专属工作区 V2 重构（右键入口 + 新模型 + FTS 检索）
**场景**:
需要将左上角 Lazycat 的交互改为左键回首页、右键进入专属代码片段工作区，并对代码片段页面做结构级重构。

**问题**:
1. 现有 snippets 页面挂在通用工具壳层中，无法形成专注工作区。
2. 旧 snippets 数据模型和查询逻辑偏旧，缺少“最近使用优先”和结构化初始化流程。
3. 首次进入需要执行“清空旧数据并重建”的强制流程。

**解决**:
1. App 壳层增加 `viewMode`，支持 `main` 与 `snippet-workspace` 双模式切换。
2. `SidebarNav` 品牌按钮增加右键事件，右键进入专属工作区，左键行为保持回首页。
3. `SnippetPanel.vue` 重写为标签优先三栏布局，接入 `tool:snippets:v2:*` 通道。
4. Rust `snippets.rs` 重写 V2 逻辑，新增：
   - `v2_init`（首次确认后清空并重建）
   - `v2_list` / `v2_search` / `v2_get` / `v2_create` / `v2_update` / `v2_delete`
   - `v2_mark_used` / `v2_tag_stats` / `v2_folder_list` / `v2_folder_create` / `v2_folder_update` / `v2_folder_delete`
5. `helpers.rs` 增加 schema migration 8，创建 snippets v2 表结构与索引；FTS5 建表降级为可选，避免不支持 FTS 的环境直接失败。

**关键点**:
1. 首次初始化采用强确认输入 `DELETE`，降低误触导致的数据清空风险。
2. 排序默认切到 `last_used_at + use_count`，并在打开/复制时调用 `mark_used`。
3. FTS 不可用时自动退化到 LIKE 查询，不阻断可用性。

**涉及文件**:
- apps/desktop/src/App.vue
- apps/desktop/src/components/SidebarNav.vue
- apps/desktop/src/components/SnippetPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src/styles/layout.css
- apps/desktop/src/styles/responsive.css
- apps/desktop/src-tauri/src/tools/snippets.rs
- apps/desktop/src-tauri/src/tools/helpers.rs

**使用次数**: 0

## 2026-02-21: Cron 工具易用性 V2（Spring 6 字段标准 + 5 字段兼容 + 时区预览）
**场景**:
Cron 工具原先仅提供基础 6 字段输入与简单预览，缺少规范化、模板、规则描述与时区切换，易用性不足。

**问题**:
1. 用户输入 5 字段表达式时无兼容策略，容易报错。
2. 缺少“表达式含义”反馈，用户难以快速确认规则。
3. 预览结果固定本地时间，跨环境排查不便。
4. 前端与后端接口粒度较粗，不利于扩展。

**解决**:
1. Rust `cron` 工具新增 action：`normalize`、`preview_v2`、`describe`。
2. 标准化策略固定为 Spring 6 字段；兼容 5 字段时自动补秒 `0` 并返回 warnings。
3. 预览支持时区参数（local / UTC / IANA 时区），并返回结构化时间项（display/iso/epochMs）。
4. Cron 面板重构为四段式：表达式规范化、字段构建、模板应用、预览表格。
5. 新增前端 `types/cron.ts`，统一响应类型定义。
6. 增加 Rust 单元测试覆盖 normalize、时区回退、常见描述规则。

**关键点**:
1. 保留旧 `tool:cron:preview/parse`，新增 v2 能力，降低回归风险。
2. 7 字段（含 year）明确拒绝，避免隐式不兼容。
3. 时区解析失败回退 local 并给 warning，不中断主流程。

**涉及文件**:
- apps/desktop/src/components/CronPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src/types/cron.ts
- apps/desktop/src/types/index.ts
- apps/desktop/src-tauri/src/tools/cron.rs
- apps/desktop/src-tauri/Cargo.toml

**使用次数**: 0

## 2026-02-21: 文本处理工具重做（清洗 + 提取 + 双栏统计）
**场景**:
将“文本处理”从仅按行去重/排序升级为可配置的文本清洗与提取管线，并增强结果展示。

**问题**:
1. 旧能力过窄，仅 2 个后端 action，难以覆盖日志/配置清洗场景。
2. 前端缺少操作编排、统计反馈、差异预览，用户难以判断处理效果。
3. 文本面板存在文案乱码风险，影响可读性和可维护性。

**解决**:
1. Rust `text` 域替换为统一 `process` action，支持 trim/remove-empty/dedupe/sort/filter/replace/prefix/suffix/extract-column。
2. 新增 `presets` action，返回日志清洗、配置键提取、错误日志提取等预设。
3. 前端 `TextProcessPanel` 重写为双栏对照，新增操作区、统计卡片、变更样本表、自动执行与预设套用。
4. 通道映射改为 `tool:text:process` + `tool:text:presets`，移除旧 `unique-lines/sort-lines`。
5. 新增 `types/text.ts` 并统一导出，明确请求/响应与操作类型。

**关键点**:
1. 采用“前后端协同”：Rust 提供稳定算子，前端负责编排与展示。
2. 变更样本做数量上限控制（`previewLimit`），避免大文本导致前端卡顿。
3. 直接替换旧通道前先全仓检索调用点，确认仅单点使用后再切换。

**涉及文件**:
- apps/desktop/src/components/TextProcessPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src/types/text.ts
- apps/desktop/src/types/index.ts
- apps/desktop/src-tauri/src/tools/text.rs
- apps/desktop/src/App.vue

**使用次数**: 0

## 2026-02-21: Backend Unit Test Expansion for Critical Tool Domains
**场景**: 为 Rust 后端 tools 域补充单元测试，重点覆盖编码转换、加解密、模板渲染与高风险输入分支。
**问题**:
1. 现有测试主要集中在 cron/text，核心安全与转换能力覆盖不足。
2. 多个 action 缺少错误分支验证，回归时容易出现静默偏差。
3. 系统能力（network/dns/file/image/env/port 等）缺少稳定 smoke 测试。
**解决**:
1. 为 `encode/crypto/convert/jwt/schema/mybatis/nginx` 增加核心单测与错误分支。
2. 为 `network/dns/file/image/env/port/format/gen/time/regex/manuals/mod` 增加稳定测试。
3. 调整易波动断言（如 OpenSSL DES 可用性、resize 等比行为、url_decode 容错行为）以避免假阳性失败。
4. 统一执行 `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`，最终 76/76 通过。
**关键点**:
1. 避免新增测试依赖导致环境下载失败；优先使用标准库与现有依赖。
2. 对系统相关能力尽量使用本地回环与临时文件，避免依赖外网和真实系统状态。
3. 对第三方库行为差异（OpenSSL provider、urlencoding 容错）采用兼容断言。
**涉及文件**:
- apps/desktop/src-tauri/Cargo.toml
- apps/desktop/src-tauri/src/tools/encode.rs
- apps/desktop/src-tauri/src/tools/crypto.rs
- apps/desktop/src-tauri/src/tools/convert.rs
- apps/desktop/src-tauri/src/tools/jwt.rs
- apps/desktop/src-tauri/src/tools/schema.rs
- apps/desktop/src-tauri/src/tools/mybatis.rs
- apps/desktop/src-tauri/src/tools/nginx.rs
- apps/desktop/src-tauri/src/tools/network.rs
- apps/desktop/src-tauri/src/tools/dns.rs
- apps/desktop/src-tauri/src/tools/file.rs
- apps/desktop/src-tauri/src/tools/image.rs
- apps/desktop/src-tauri/src/tools/env.rs
- apps/desktop/src-tauri/src/tools/port.rs
- apps/desktop/src-tauri/src/tools/regex.rs
- apps/desktop/src-tauri/src/tools/manuals.rs
- apps/desktop/src-tauri/src/tools/settings.rs
- apps/desktop/src-tauri/src/tools/snippets.rs
- apps/desktop/src-tauri/src/tools/hotkey.rs
- apps/desktop/src-tauri/src/tools/format.rs
- apps/desktop/src-tauri/src/tools/gen.rs
- apps/desktop/src-tauri/src/tools/time.rs
- apps/desktop/src-tauri/src/tools/mod.rs

## 2026-02-27: release 脚本 Git link.exe 遮蔽 MSVC 链接器

**场景**: 执行 `release-all-win.ps1` 打包脚本，Rust 编译链接阶段失败
**问题**: `C:\Program Files\Git\usr\bin\link.exe`（GNU coreutils link）在 PATH 中优先于 MSVC 的 `link.exe`，导致 `linking with link.exe failed: exit code: 1`。即使 VsDevCmd.bat 已执行，Git 的 usr/bin 仍在 PATH 前面
**解决**: 在 `Invoke-InVsDevEnv` 函数中，调用 cmd /c 前在 PowerShell 层面过滤 PATH：`$env:Path = ($env:Path -split ';' | Where-Object { $_ -notmatch 'Git\\usr\\bin' }) -join ';'`，并在 finally 块中恢复原始 PATH
**关键点**:
1. cmd.exe 内的 `set "PATH=%PATH:old=new%"` 字符串替换对含空格路径不可靠，应在 PowerShell 层面处理
2. VsDevCmd.bat 虽然设置了 MSVC 工具路径，但不会移除已有的 Git 路径
**涉及文件**: scripts/release-all-win.ps1
**使用次数**: 0
**使用次数**: 0

## 2026-03-07: 本地待办统一为事项实例 + 周期系列

**场景**: 用户希望把原本分开的“任务”和“周期事件”整合成统一模型与统一维护入口，主列表以当前可执行事项为中心。

**问题**:
1. 旧实现虽然在同一个 `todo` 工具内，但前端仍按 `task/template` 两套对象分栏维护。
2. 后端缺少“单次事项也属于系列”的统一语义，周期规则与实例操作边界不清晰。
3. 前端编辑器没有统一承载“单次事项 / 周期事项 / 当前实例 / 后续系列”四种编辑上下文。

**解决**:
1. `helpers.rs` 新增 migration 14：为 `todo_templates` 增加 `series_kind`，并把历史孤立任务回填为 `one_off` 系列。
2. `todo.rs` 新增 unified actions：`item_*` 与 `series_*`，同时保留 `task_*` / `template_*` 兼容别名。
3. 主列表统一走 `item_list`，补充 `seriesId`、`seriesKind`、`isRecurring`、`canEditFuture`、`displayAt` 等字段。
4. `TodoPanel.vue` 重构为“事项 / 系列 / 提醒中心 / 基础数据”四视图，并用单一弹窗统一创建与编辑。
5. 周期实例编辑支持 `this_instance` 与 `future_instances` 两种作用域；后者由后端转为系列更新。

**关键点**:
1. 单次事项创建时也自动创建 `one_off` 系列，避免后续逻辑继续依赖空的 `source_template_id`。
2. 周期系列继续保留“生成实例”语义，调度器只处理 `recurring` 且启用中的系列。
3. 系列删除不会删除历史实例；已生成实例会退化为独立事项继续保留。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/types/index.ts`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check`

**使用次数**: 0

## 2026-03-07: 本地待办自动收藏与命名快捷键接入

**场景**: 用户希望“本地待办”默认出现在首页常用工具中，并像代码片段/密码管理/快捷启动一样支持单独的全局快捷键呼出。
**问题**:
1. 首页“常用工具”当前完全依赖 `favorites` 与近 30 天点击历史，没有“默认收藏一次性补种”机制。
2. 现有命名快捷键链路已支持任意目标工具，但前端只暴露了 `snippets`、`vault`、`launcher` 三个配置入口。
3. 如果直接每次启动都强行把 `todo` 加回收藏，会覆盖用户手动取消收藏的意图。
**解决**:
1. 在 `useFavorites.ts` 中抽出 `normalizeFavoriteToolIds` 与 `bootstrapFavoriteToolIds`，统一做收藏去重、过滤与待办一次性补种。
2. 新增 `favorites_todo_seeded` 标记：首次启动时若收藏中没有 `todo`，自动插入收藏首位；一旦补种完成或用户原本已收藏，即写入标记，后续不再重复干预。
3. 在 `SettingsPanel.vue` 增加“本地待办”快捷键录入项，并纳入现有冲突检测、保存与清空流程。
4. 在 `App.vue` 启动阶段读取 `hotkey_todo`，通过现有 `registerNamedHotkey("todo", ...)` 注册；继续复用 `hotkey-navigate` 的显隐/跳转逻辑。
5. 新增 `useFavorites.test.ts` 覆盖补种规则，并补充 `hotkeyNavigate.test.ts` 的 `todo` 场景回归。
**关键点**:
1. “固定到常用工具”在本需求里等价于“走现有收藏模型的一次性自动收藏”，不是新增永久固定入口。
2. 对已手动收藏 `todo` 的用户也要写入补种完成标记，避免日后取消收藏后又被系统重新加回。
3. 复用现有命名快捷键协议即可，前后端无需新增 Tauri command 或事件结构。
**涉及文件**:
- `apps/desktop/src/composables/useFavorites.ts`
- `apps/desktop/src/composables/useFavorites.test.ts`
- `apps/desktop/src/App.vue`
- `apps/desktop/src/components/SettingsPanel.vue`
- `apps/desktop/src/utils/hotkeyNavigate.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/hotkeyNavigate.test.ts src/composables/useFavorites.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-07: 本地待办事件时间与提醒预设重构

**场景**: 用户要求把本地待办里的“截止时间 + 提醒时间”重构为“事件时间 + 提醒预设”，事件时间最小刻度统一为 5 分钟，并把周期系列从独立页签合并到事项页下方折叠区。
**问题**:
1. 旧模型同时维护 `due_at` / `remind_at`，单次事项与周期实例的含义不一致，前端也需要维护两个绝对时间输入。
2. 周期系列没有单独的提醒偏移字段，生成实例时只能把提醒时间直接写成触发时刻。
3. 现有文件里有历史乱码文案，重构过程中很容易把 Rust 字符串语法一并带坏，必须依赖编译器逐轮清理。
**解决**:
1. 前端类型与表单统一切到 `eventAt + reminderPreset`，提醒预设固定为 `none/5m/10m/30m/1h/1d/2d`，并在表单提交前校验 5 分钟刻度。
2. Rust 端新增 `event_at` 与 `reminder_offset_minutes` 模型：任务对外只暴露事件时间与提醒预设，内部继续复用 `remind_at + snooze_until` 做提醒调度。
3. migration 15 为历史数据回填 `event_at`，只保留能精确映射到新预设的旧提醒，其余旧 `remind_at` 直接清空；周期模板提醒偏移统一置空。
4. 事项页合并“系列”页签，在列表下方折叠展示周期系列，周期事项的规则区块移动到弹窗下半部分。
5. Rust 单测补充 5 分钟刻度与提醒预设换算，并用 `cargo test todo:: -- --nocapture` 做定向回归；前端用 `pnpm typecheck` 与 `pnpm --filter @lazycat/desktop build:web` 验证联调。
**关键点**:
1. 对任务编辑来说，`eventAt` 或 `reminderPreset` 任一变更都要重新计算 `remind_at`，同时清空 `snooze_until` 与 `last_notified_at`，避免旧稍后提醒污染新计划。
2. 对周期模板来说，只存提醒偏移，不存绝对提醒时间；实例生成时再根据发生时间反推 `remind_at`。
3. 处理历史乱码文件时，不要盲目整文件替换；先跑编译，再按报错行定点修复，成本最低、风险也最小。
**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/types/index.ts`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`

**验证**:
- `cargo test todo:: -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-08: 本地待办合并待办列表并改为前端逾期判断

**场景**: 用户要求把“超期事项 + 待办事项”合并为一个待办列表，已办默认折叠，并用复选框替代“完成”按钮。
**问题**:
1. 前端 `TodoPanel.vue` 已经切到固定双区块展示，但底层 `groupTodoItemsByBucket` 仍返回 `overdueItems / pendingItems / doneItems`，与面板消费口径不一致。
2. 旧分桶逻辑依赖后端下发的 `isOverdue` 字段，无法满足“前端自行判断逾期”的新要求，也会让展示规则和排序规则分散在不同层。
3. `todoBuckets.test.ts` 里“周期根事项进入待办桶”的历史断言此前就是红的，如果不在这次一并修正，后续待办列表改造很难确认真实回归状态。
**解决**:
1. `todoBuckets.ts` 改为只返回 `activeItems + doneItems`，其中 `activeItems` 收口为“可执行项 + 周期根事项”，并按 `eventAt || displayAt` 升序排序、无时间排最后。
2. `TodoPanel.vue` 改为消费 `activeItems / doneItems`，事项列中的“逾期”标记统一由前端按 `pending/in_progress + (eventAt || displayAt) < now` 判断。
3. 待办与已办表格统一增加复选框列：待办勾选即切 `completed`，已办取消勾选回 `pending`；同时移除状态列、时间列副文本和操作列里的“完成”按钮。
4. `todoBuckets.test.ts` 同步改成新返回结构断言，并补充排序、`displayAt` 回退、无时间排最后与周期根事项归活跃桶场景，恢复该模块测试基线。
**关键点**:
1. 逾期展示应只面向待处理项；如果直接按“时间早于当前”判断，已办项也会被错误标成逾期。
2. 固定分段页面适合“先过滤，再统一分桶”，分桶 helper 只负责分组和排序，不承担 UI 折叠或标记文案逻辑。
3. 历史红测如果正好落在本次改造路径上，应顺手修复；否则即使构建通过，也很难判断新改动是否真的稳定。
**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`

**验证**:
- `pnpm test src/utils/todoBuckets.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0
