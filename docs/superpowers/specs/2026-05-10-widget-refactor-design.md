# 桌面挂件架构重构设计

**日期**：2026-05-10
**状态**：待实现

## 一、背景与动机

当前桌面挂件模块（`src-tauri/src/tools/widget/`）经过多次快速迭代后积累了三类技术债务：

1. **可靠性问题**：挂件偶尔不显示/卡在加载中。根因包括 widget://ready 握手存在 TOCTOU 竞态、show() 失败被静默吞掉、无自愈机制。
2. **状态管理分散**：widget 可见性被 scheduler.rs、events.rs、mod.rs 三处各自操控，CURRENT_STATE(AtomicU8)、PENDING_Y(AtomicI32)、LAST_INPUT_HASH(AtomicU64)、STATE(RwLock) 四种原子变量散落各处，无单一真相源。
3. **模块碎片化**：12 个 .rs 文件，scheduler 和 events 各自独立循环且各自实现 should_skip 判定，fullscreen/lock/idle 各仅 50-70 行却独立成文件。
4. **诊断困难**：eprintln! 散落各处，无结构化日志，出问题时无法快速定位根因。

## 二、设计目标

1. **根治竞态**：统一状态持有者，所有可见性变更走单点入口
2. **消除重复**：合并 scheduler + events 为单一调度循环，should_skip 逻辑只写一次
3. **自愈能力**：看门狗检测窗口异常并自动重建
4. **可诊断性**：结构化事件记录 + 诊断面板，替代散落日志
5. **保持兼容**：外部接口（Tauri events、通道、前端 props）不变

## 三、架构变更

### 3.1 模块重组

| 旧模块 | 新模块 | 动作 |
|--------|--------|------|
| （无） | `diagnostics.rs` | **新增** — WidgetEvent 枚举 + 环形缓冲 |
| （无） | `session.rs` | **新增** — WidgetSession 状态持有者 |
| （无） | `guards.rs` | **新增** — 合并 fullscreen + lock + idle |
| （无） | `pulse.rs` | **新增** — 合并 scheduler + events + midnight |
| `state.rs` | — | **删除** — 字段移入 WidgetSession |
| `scheduler.rs` | — | **删除** — 合并入 pulse.rs |
| `events.rs` | — | **删除** — 合并入 pulse.rs |
| `fullscreen.rs` | — | **删除** — 合并入 guards.rs |
| `lock.rs` | — | **删除** — 合并入 guards.rs |
| `idle.rs` | — | **删除** — 合并入 guards.rs |
| `widget.rs` | `widget.rs` | **修改** — 状态管理移出，保留窗口创建/销毁/定位/光标轮询(cursor_loop)；保留 flush_loop（200ms 将 PENDING_Y 持久化到 DB，现通过 session.pending_y 读写） |

模块数：12 → 10。新增 4，删除 6，修改 4（widget.rs, mod.rs, apply.rs, config.rs），不变 2（data.rs, dashboard_logic.rs），小幅联动 1（conflicts.rs — 写入目标从 state.rs 改为 session.rs）。

### 3.2 WidgetSession（session.rs）

单一状态持有者，替代所有散落的 atomic / RwLock / 每次读 DB 的 config。

**基础类型定义：**

```rust
/// 窗口可见状态（当前三态 + 新增 Windowless）.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisualState {
    Peek,     // 贴右边缘 8px 提示条
    Full,     // 完全展开 360px
    Hidden,   // 完全不可见（老板键/全屏/锁屏触发，窗口仍存在）
    Windowless, // 窗口未创建/已销毁（替代旧 is_open() 检查）
}

/// 暂停原因，与前端 WidgetPauseReason 对齐.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PauseReason {
    Fullscreen,
    Lock,
    Manual,
}

/// 配置缓存（字段对齐 config.rs 的 WidgetConfig，见 §3.1 表）.
/// enabled / style / refresh_interval_min / fullscreen_blacklist / privacy_mask /
/// privacy_mask_until / widget_y — 7 字段，定义同 config.rs 现有结构。
```

**WidgetSession 结构：**

```rust
struct WidgetSession {
    // 窗口
    window: Option<WebviewWindow>,

    // 热路径字段（独立原子变量，不在 RwLock 内）
    visual_state: AtomicU8,          // VisualState，cursor_loop 80ms 轮询无锁读取
    paused: AtomicBool,              // cursor_loop / pulse 高频读取，无锁

    // 运行时状态（原 state.rs，RwLock 保护）
    pause_reason: Option<PauseReason>,
    last_rendered_at: Option<String>,
    last_error: Option<String>,
    auto_skip_reason: Option<String>,
    spotlight_detected: bool,        // 原 state.rs，由 conflicts::refresh() 写入
    third_party_engine: Option<String>, // 原 state.rs

    // 配置缓存（原 config.rs 每次读 DB）
    config_cache: WidgetConfig,
    config_dirty: bool,

    // 内容去重（原 apply.rs atomic）
    input_hash: u64,

    // 拖拽位置（原 widget.rs atomic）
    pending_y: i32,

    // 握手（新增）
    ready_deadline: Option<Instant>, // ensure() 后设置的 3s 超时

    // 诊断（新增）
    events: VecDeque<WidgetEvent>,  // 环形缓冲 50 条，带 sequence_id
    next_sequence_id: u64,          // 单调递增，消费者可检测溢出间隙
    last_ping_at: Instant,

    // 看门狗（新增）
    window_generation: u64,          // 窗口重建次数，cursor_loop 用此检测窗口有效性
    watchdog_rebuilds: u32,         // 连续重建计数（成功时清零）
    rebuild_in_progress: bool,      // 防止并行重建
}
```

**核心方法：**

- `transition(to: VisualState)` — 所有可见性变更的唯一入口。保证幂等（to == 当前直接返回）、原子（先改状态再操作窗口，失败回写）、可追溯（每次自动 record 事件）。Windowless 状态仅用于表示窗口不存在，transition(Windowless) 等价于 destroy()。
- `should_skip() -> bool` — 统一判定：disabled / paused / locked / fullscreen，只写一次
- `sync_visibility(skip: bool)` — 根据 skip 结果驱动 transition(Hidden) 或 transition(Peek)
- `record(event: WidgetEvent)` — 追加事件到环形缓冲，next_sequence_id 自增
- `rebuild_window() -> Result<()>` — 设置 rebuild_in_progress=true → 销毁旧窗口 → 重建 → 等 ready（3s 超时）→ apply；失败返回 Err + watchdog_rebuilds += 1；成功清零 rebuilds
- `is_window_open() -> bool` — `window.is_some()`，看门狗/可见性同步前检查
- `visual_state() -> VisualState` — 从 AtomicU8 无锁读取当前可见状态
- `generation() -> u64` — 从 window_generation 无锁读取，cursor_loop 用于校验窗口有效性
- `refresh_config_if_dirty()` — config_dirty 为 true 时从 DB 重读 config 到 config_cache
- `mark_config_dirty()` — 供 config.rs 的 set_* 函数调用，标记下次 tick 需刷新配置缓存
- `invalidate_input_hash()` — 设置 input_hash = 0，下一轮 apply 强制推送
- `status_snapshot() -> Value` — 从 session 字段构造 status 通道返回值（替代原 state::status_snapshot()），privacy_mask 过期判断同现逻辑

**SINGLETON**: WidgetSession 通过 `std::sync::LazyLock<RwLock<WidgetSession>>` 全局访问。本项目 Rust 版本 ≥1.80，LazyLock 已 stabilised；否则可回退 `once_cell::sync::Lazy`。

**模块依赖规则**：diagnostics.rs 是叶模块 — 只被 session.rs 导入，不反向导入任何 widget 模块，避免循环依赖。

**并发安全设计：**

- **热路径字段拆分**：`visual_state` 和 `paused` 两个字段被 cursor_loop（80ms 轮询）和 pulse tick（30s~3600s）高频读取。将它们从 RwLock 中拆出为独立 `AtomicU8`（visual_state）和 `AtomicBool`（paused），避免 cursor_loop 的读锁被 pulse 的写操作（record 事件等）阻塞。其余低频字段仍走 RwLock。
- **窗口 generation counter**：session 中增加 `window_generation: u64`，每次 rebuild_window 成功后自增。cursor_loop 在调用 `transition()` 前对比当前 generation 与捕获时的值，不匹配则放弃本次 transition，防止操作已销毁的旧窗口。
- **transition() 锁持有策略**：transition() 仅在修改 `visual_state`（AtomicU8 store）和 record 事件（短暂写锁）时持锁，窗口操作（show/hide/set_position）在锁外执行，避免慢速窗口 API 阻塞其他读写。
- **配置变更通知路径**：前端设置面板修改配置（如 refresh_interval_min、privacy_mask 等）→ 走现有通道（`tool:widget:set_config` 等）→ 后端写入 DB → 调用 `session.mark_config_dirty()` 设置 `config_dirty=true`。pulse 下一轮 tick 时 `refresh_config_if_dirty()` 重读并清 dirty 标记。config.rs 中原有的 `set_*` 函数统一增加 `session.mark_config_dirty()` 调用。

### 3.3 状态机

**VisualState 四变体**（在旧 Peek/Full/Hidden 基础上增加 Windowless）：

```
Windowless ──[enable() → widget::ensure()]──> Peek
Peek ──[cursor_loop 检测 hover 右边缘]──> Full
Full ──[cursor_loop 检测 leave + 800ms]──> Peek
Peek/Full ──[disable() / lock / fullscreen]──> Hidden
Hidden ──[enable() / resume() / unlock / exit fullscreen]──> Peek
Peek/Full/Hidden ──[app_exit() / destroy()]──> Windowless
```

**hover 检测机制不变**：Peek↔Full 切换仍由 `widget.rs` 的后台线程 `cursor_loop()` 驱动（80ms GetCursorPos 轮询，12.5Hz），不改为前端驱动。`cursor_loop()` 检测到条件变化后调用 `session.transition(to)`。此机制经过验证稳定，本次重构不改动其逻辑，仅将其中的 `set_state()` 调用替换为 `session.transition()`。

**Windowless 状态约束**：`ensure()` 成功后必须执行 `session.transition(Peek)` 更新状态；`destroy()` 成功后必须执行 `session.transition(Windowless)`。禁止绕过 transition 直接操作窗口句柄或修改 visual_state。cursor_loop 在每次迭代中捕获 `window_generation` 并在调用 transition 前校验，不匹配则跳过本轮。

所有迁移通过 `session.transition(to)` 执行，调用方不直接操作窗口句柄。

**WidgetConfig 类型（7 字段，从现有 config.rs 提取，本次无变更）：**

```rust
struct WidgetConfig {
    enabled: bool,
    style: String,                 // "dashboard"
    refresh_interval_min: i64,     // 5/15/30/60
    fullscreen_blacklist: Vec<String>,
    privacy_mask: bool,
    privacy_mask_until: Option<String>,  // ISO RFC3339, None=永久
    widget_y: Option<i64>,         // 物理像素, None=居中
}
```

### 3.4 统一调度（pulse.rs）

合并 scheduler.rs + events.rs + midnight_loop 为一个单线程循环。

**单一线程**，使用 std::thread（不引入 tokio）：

```
loop {
    // 1. 非阻塞检查事件（try_recv 排空 channel）
    while event_rx.try_recv().is_ok() { has_event = true; }
    // 收到事件 → 立即进入 5s trailing-edge debounce，不等满 30s chunk
    // debounce 期间新事件重置计时；最多等 5s 静默期
    if has_event { debounce_5s(event_rx); tick(false); continue; }

    // 2. 心跳到期检查
    if since_heartbeat >= current_interval { tick(false); }

    // 3. 跨日检查（每小时一次，通过 last_midnight_check 记录）
    if midnight_just_passed() { tick(true); }

    // 4. 看门狗（每次 tick 后都检查）
    check_watchdog();

    // 5. 分块 sleep 30s，同时监听事件唤醒
    //    最大 tick 间隔 = 30s，收到事件立即 break 进入 debounce
    event_rx.recv_timeout(Duration::from_secs(30));
}
```

**tick() 统一逻辑：**
1. session.refresh_config_if_dirty()
2. skip = session.should_skip()
3. session.sync_visibility(skip)
4. if !skip → apply::apply_with_force(force)
5. session.record(ApplyAttempt 或 ApplySkipped)

**事件延迟最坏情况**：CRUD 事件到达后立即被 `try_recv` 捕获并进入 `debounce_5s()`，最坏延迟 = 5s 静默期。不再受 sleep chunk 长度影响。

**看门狗最坏间隔**：心跳间隔最大 3600s，但 `recv_timeout(30s)` 保证每 30s 至少醒来一次检查看门狗。实测最坏 = 30s sleep + 5s debounce = 35s（仅当事件在看门狗检查前到达时）。

**interval 计算**：每秒检查 `guards::seconds_idle()`，≥300s 空闲时 interval=3600s，否则使用 config_cache.refresh_interval_min。钟控间隔可能因事件到达而提前唤醒，不影响正确性。

### 3.5 防护检测（guards.rs）

合并 fullscreen.rs、lock.rs、idle.rs 三个文件为一个 guards 模块。

- `guards::is_fullscreen_busy() -> bool` — 三层判定不变
- `guards::is_locked() -> bool` — OpenInputDesktop 不变
- `guards::seconds_idle() -> u32` — GetLastInputInfo 不变

仅在 pulse tick() 和 transitions 中组合使用，不再由 scheduler 独立调用。

### 3.6 诊断系统（diagnostics.rs）

**WidgetEvent 枚举**（9 变体）替代所有 eprintln!：

| 变体 | 携带字段 | 替代的日志 |
|------|---------|-----------|
| `StateTransition` | from, to, trigger | state 切换日志 |
| `ApplyAttempt` | force, result (ok/skipped/error + elapsed_ms) | apply enter/done 日志 |
| `ApplySkipped` | reason (disabled/no-change/locked/fs) | apply skipped 日志 |
| `WindowCreated` | elapsed_ms | building widget window |
| `WindowDestroyed` | reason | closing widget window |
| `Error` | source, message | 各种 failed 日志 |
| `PingReceived` | — | **新增** |
| `WatchdogTriggered` | seconds_since_ping | **新增** |
| `Lifecycle` | action (enable/disable/pause/resume) | enable/disable 日志 |

**环形缓冲**：`VecDeque<WidgetEvent>` 最大 50 条，新事件挤出最旧。

**查询通道**：`tool:widget:diagnostics` 返回最近事件列表 + 健康概览（状态、最近 ping、最近 apply、今日跳过/看门狗/重建计数）。

### 3.7 看门狗

**前端（WidgetCanvas.vue）：**
- `onMounted` 后启动 `setInterval` 每 5s 发 `emit("widget://ping")`
- 连接中断时仅通过 `session.last_error` 在面板中显示，不在挂件上展示可见遮罩（避免短暂 WebView2 渲染延迟导致用户可见的闪烁）。前端只记录 `lastDataReceivedAt`，当超过 60s 无数据时在挂件底部显示低调的"刷新中…"文字（非遮罩）
- 恢复收到数据时自动消除

**后端（pulse tick）：**
- 每次 tick 检查 `now - session.last_ping_at > 15s`
- 超时 → session.record(WatchdogTriggered) → session.rebuild_window()
- 重建成功 → watchdog_rebuilds = 0 → apply 推送数据
- 重建失败 → watchdog_rebuilds += 1

**rebuild_window 流程与防重入**：
1. 检查 `rebuild_in_progress`：true → 跳过（上一轮重建仍在进行）
2. 设 `rebuild_in_progress = true` → 从 `PENDING_Y` atomic 直读最新值（避免 200ms flush 窗口期内 DB 值滞后）→ destroy 旧窗口 → widget::ensure() 创建新窗口 → `window_generation += 1` → 恢复 `pending_y` → 等待 widget://ready（3s 超时）→ `rebuild_in_progress = false`
3. ensure() 失败或 ready 超时 → 返回 Err → rebuilds += 1
4. rebuilds >= 3 → session.last_error = "窗口连续 3 次重建失败，已暂停" + session.paused = true
5. 无论成功或失败，最后 `rebuild_in_progress = false`

**pending_y 保留**：重建前直接从 `PENDING_Y` atomic 读取（不经 DB 中转），重建后 restore_y_phys() 将 Y 值写回新窗口，保证用户拖拽位置不丢失。

### 3.8 widget://ready 握手加固

**术语定义：**
- `widget::ensure(app)` — 创建/获取挂件 WebView 窗口（widget.rs 现有函数），成功返回 WebviewWindow 句柄。本次重构改为通过 session 调用，窗口创建成功后立即 show() 并置为 Peek 态。
- `apply::apply_with_force(app, force)` — 聚合数据 + 推送 Tauri events（apply.rs 现有函数）。本次重构 hash 去重值从 session.input_hash 读写。

**旧流程（有竞态）：**
```
widget::ensure() → show() + set_state(Peek) → apply::apply(force=true)
  → emit("widget://dashboard-data")
       ↑ Vue 未挂载，listener 未注册 → 数据丢失 → 用户看到永久"加载中…"
```

**新流程：**
```
widget::ensure() → show() + transition(Peek)
  // 不立即 apply，等待前端 ready 信号
Vue mount → listen("widget://dashboard-data") 注册 → emit("widget://ready")
  → backend ready handler:
      apply::invalidate_input_hash()  // 新 session
      apply::apply_with_force(app, force=true)
      → emit("widget://dashboard-data") → ✅ 保证送达
```

**3s 超时兜底：**
- ensure() 调用后启动 3s 定时器（存储在 session 中作为 `ready_deadline: Option<Instant>`）
- pulse tick() 中检查：ready_deadline 已过且未收到 ready → session.record(Error { source: "ready_timeout" }) → session.rebuild_window()
- 防止 Vue 挂载因脚本错误或 WebView2 渲染器崩溃而永久挂起

## 四、前端变更

### 4.1 WidgetCanvas.vue

- `onMounted` 新增 `setInterval(emitPing, 5000)` → `emit("widget://ping")`
- 新增 `lastDataReceivedAt` ref：收到 dashboard-data 时更新为 `Date.now()`
- 超过 60s 无数据时在挂件底部显示低调的"刷新中…"文字（非遮罩，与 §3.7 一致）；数据恢复后自动消除。不使用半透明遮罩，避免短暂 WebView2 渲染延迟导致用户可见的闪烁

### 4.2 WidgetPanel.vue

- 新增"诊断" Tab（el-tab-pane label="诊断" name="diagnostics"）
- 健康概览卡片：状态、窗口态、最近 ping、最近 apply、今日统计
- 事件时间线：渲染最近 20 条 WidgetEvent（时间戳 + 类型 + 详情）
- 5s 轮询 `tool:widget:diagnostics` 刷新

## 五、类型变更

### 5.1 类型导出

所有诊断相关类型需 derive `Serialize`/`Deserialize` 以支持 Tauri IPC 序列化（Step 1 单测覆盖序列化往返）。

```rust
// diagnostics.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
enum WidgetEvent { ... }  // 9 变体，见 §3.6
#[derive(Debug, Clone, Serialize, Deserialize)]
enum ApplyResult { ... }
#[derive(Debug, Clone, Serialize, Deserialize)]
enum SkipReason { ... }
```

```rust
// 诊断通道返回值
struct WidgetHealth {
    status: String,               // "running" | "paused" | "disabled" | "error"
    visual_state: String,         // "peek" | "full" | "hidden" | "windowless"
    last_ping_secs_ago: u64,
    last_apply_secs_ago: u64,
    last_apply_result: String,    // "ok" | "skipped" | "error"
    today_skip_count: u32,
    today_watchdog_count: u32,
    today_rebuild_count: u32,
}
```

### 5.2 通道新增

- `tool:widget:diagnostics` → 返回 `{ health: WidgetHealth, events: WidgetEvent[] }`

## 六、实施顺序

按依赖关系分 5 步，每步可独立编译验证：

| Step | 内容 | 新增行 | 验证 |
|------|------|--------|------|
| 1 | diagnostics.rs | ~120 | `cargo test widget::diagnostics` — WidgetEvent 全变体 serde 往返、环形缓冲溢出、sequence_id 单调性 |
| 2 | guards.rs + 删除 fullscreen/lock/idle | ~200 | `cargo check -p lazycat-desktop` 编译通过；手动验证锁屏/全屏检测行为不变 |
| 3a | session.rs + 删除 state.rs | ~300 | 编译通过；`cargo test widget::session` — transition 幂等性、状态全组合往返、should_skip 四条件真值表（启用/禁用 × 暂停/恢复 × 锁屏 × 全屏）、config_dirty 标记→刷新流转、generation counter 单调性 |
| 3b | widget.rs 改造（cursor_loop 接入 session.transition + generation 校验） | ~150 | 编译通过；手动验证拖拽 + peek↔full 交互正常、拖拽 Y 持久化、generation 不匹配时 cursor_loop 安全跳过 |
| 4 | pulse.rs + 删除 scheduler/events | ~250 | 编译通过；`cargo test widget::pulse` — tick 跳过逻辑、debounce 策略（含事件提前唤醒不等待 30s chunk）、看门狗触发/恢复、空闲降频切换；手动验证 CRUD 事件 5s 内触发刷新 |
| 5 | 前端 ping + 诊断 Tab + mod.rs/config.rs 清理 | ~200 | `pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web`；手动验证看门狗（模拟：启动后立即 kill WebView2 进程，15s 内应自动重建）；config.rs 移除 Legacy key 读取兼容后，`migrate_legacy_keys()` 对空 DB 无害通过 |

**测试策略摘要：**

- **单元测试**（Step 1/3a/4）：覆盖纯逻辑 — 状态机迁移、should_skip 真值表、环形缓冲、hash 去重、config_dirty 流转、generation counter 单调性、serde 全变体往返。不需要真实的 WebviewWindow。
- **集成测试**（Step 3b/4/5）：需要真实 Tauri 窗口的手动场景 — 拖拽位置持久化、peek↔full 过渡、generation 不匹配安全跳过、看门狗触发重建、跨日立刷。
- **回归测试**：`pnpm test`（现有单元测试套件）+ `pnpm test:e2e`（E2E 回归，确保 PM/Todo CRUD 不因 widget 重构而退化）。
- **冒烟测试**（Step 5）：构建 web + 运行应用 → 启用挂件 → 验证仪表盘数据 → 鼠标右边缘触发展开 → 拖拽改变 Y 位置 → 禁用再启用 → kill WebView2 验证看门狗 15s 内自动重建 → 检查诊断 Tab 事件时间线。
- **Legacy 迁移测试**（Step 5）：config.rs 移除 Legacy key 读取兼容后，对空 DB 调用 `migrate_legacy_keys()` 应无害通过；对含旧 key 的 fixture DB 应正确迁移。

## 七、风险与缓解

| 风险 | 缓解 |
|------|------|
| session 锁竞争 | visual_state/paused 拆为独立原子变量，热路径（cursor_loop 80ms、pulse tick）无锁读取；transition 锁持有时间 μs 级（仅 atomic store + event push），窗口操作在锁外执行 |
| cursor_loop 操作已销毁窗口 | window_generation counter：cursor_loop 捕获当前值 → 调用 transition 前校验 → 不匹配则跳过 |
| 看门狗误触发 | 阈值 15s（3 个 ping 周期），pulse tick 每 30s 检查一次（recv_timeout chunk）；短时 WebView2 渲染延迟不会触发 |
| rebuild_window 死锁 | 与 disable 同策略：destroy 异步 close()，不等待回调 |
| rebuild_window 重入 | `rebuild_in_progress` 标志位阻止并行重建；看门狗下次 tick 重试 |
| rebuild_window 丢 Y 位置 | 直接从 PENDING_Y atomic 读取最新值，不经 DB 中转，消除 200ms flush 窗口 |
| 前端 ping 未注册 | pulse tick 中 `session.is_window_open()` 检查；窗口不存在时跳过看门狗 |
| 配置缓存过时 | 所有配置写入方统一调用 `session.mark_config_dirty()`；pulse 每轮 tick 前刷新 |
| 旧配置 | 移除 wallpaper.* Legacy key 的**读取**兼容；`migrate_legacy_keys()` 已在 v2 迁移时调用完毕（所有已升级用户均完成迁移）；`perform_legacy_cleanup()` 已完成历史使命一并删除；新增空 DB/fixture DB 迁移测试防止回归 |
| cursor_loop panic | 改为 `std::panic::catch_unwind` 包裹；panic 后 record(Error) + 自动重启线程 |
| pulse panic | 同 catch_unwind；panic 后 record(Error) + 重启 pulse 线程 |

## 八、不变项

- 外部 API：Tauri events（`widget://dashboard-data`、`widget://color-mode`、`widget://navigate`、`widget://ready`、`widget://ping`、`widget://canvas-action`）不变
- 通道接口：`tool:widget:*` 动作名和 payload 结构不变；新增 `diagnostics` 通道
- 前端 props：WidgetOverviewBlock / WidgetTodoList / WidgetExtensionSlot 接口不变
- 挂件尺寸 360×800、Peek 8px 贴边、Full 展开、Hidden 隐藏的行为不变
- 80ms 光标轮询机制不变（后端正驱动 Peek↔Full）
- data.rs / dashboard_logic.rs / conflicts.rs 代码不变