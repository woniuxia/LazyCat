# Living Wallpaper · 实施计划（阶段 1 MVP）

> 依据设计文档：`docs/superpowers/specs/2026-05-05-living-wallpaper-design.md`（v0.4）
> 关联 spike：`docs/superpowers/spikes/2026-05-05-wallpaper-screenshot.md`
> 目标：实现阶段 1 MVP（单屏 + 仪表盘 + 3 模块 + 老板键 + 主色调自适应），目标工期 2 周

---

## 总览

| Phase   | 目标                                           | 预估   | 关键依赖         |
| ------- | ---------------------------------------------- | ------ | ---------------- |
| Phase 0 | 设置存储 / 通道骨架 / PoC 收尾                 | 0.5 天 | 已完成 PoC       |
| Phase 1 | Rust 渲染管线（数据 → 信息层 → 合成 → 设壁纸） | 3-4 天 | Phase 0          |
| Phase 2 | 前端信息层 canvas + hidden WebView 接入        | 2-3 天 | Phase 1 截图通道 |
| Phase 3 | 调度 / 老板键 / 自动切净 / 退出恢复            | 2-3 天 | Phase 1+2        |
| Phase 4 | 配置面板 + 工具入口                            | 1.5 天 | Phase 3          |
| Phase 5 | 联调 / 兼容性测试 / 文档                       | 1.5 天 | 全部             |

**Phase 1 / Phase 2 一旦截图通道（Phase 1.4）就绪即可并行。**
PoC 代码（`wallpaper_poc.rs` + `WallpaperPocCanvas.vue` + `WallpaperPocPanel.vue`）在 Phase 1.4 完成后切到正式实现，PoC 入口保留为开发态调试通道（`wallpaper-poc` 仅 dev）。

---

## Phase 0：准备

### 0.1 设置 key 与默认值

**文件**：`apps/desktop/src-tauri/src/tools/settings.rs`（沿用现有 `user_settings` 表，无需建表）

按设计 §12 写入默认值（首次启用时按需设置，未启用时不预置）：

| key                              | 类型          | 默认                                                            |
| -------------------------------- | ------------- | --------------------------------------------------------------- |
| `wallpaper.enabled`              | boolean       | false                                                           |
| `wallpaper.style`                | string        | `dashboard`                                                     |
| `wallpaper.position`             | string        | `right`                                                         |
| `wallpaper.refresh_interval_min` | number        | 15                                                              |
| `wallpaper.original_path`        | string        | -（首次启用写入）                                               |
| `wallpaper.original_set_method`  | string        | `com`                                                           |
| `wallpaper.fullscreen_blacklist` | string (JSON) | `["obs64.exe","obs32.exe","powerpnt.exe","wpp.exe","zoom.exe"]` |
| `wallpaper.privacy_mask`         | boolean       | false                                                           |
| `wallpaper.privacy_mask_until`   | string        | -（null=永久，否则 ISO 时间）                                   |
| `wallpaper.exit_behavior`        | string        | `restore_original`                                              |
| `wallpaper.boss_key`             | string        | `Ctrl+Alt+W`                                                    |
| `wallpaper.image_format`         | string        | `jpeg`（jpeg/png）                                              |
| `wallpaper.keep_history_count`   | number        | 20                                                              |

不动 schema，不写迁移；前端读不到时用默认值。

### 0.2 类型定义

**新增文件**：`apps/desktop/src/types/wallpaper.ts`

```typescript
export type WallpaperPosition = "right" | "left" | "top" | "bottom" | "tl" | "tr" | "bl" | "br";
export type WallpaperStyle = "dashboard" | "sticky" | "banner";
export type WallpaperExitBehavior = "keep_last" | "restore_original";
export type WallpaperImageFormat = "jpeg" | "png";

export interface WallpaperOverview {
  completedToday: number;
  totalToday: number;
  p0Pending: number;
  nearestDeadlineHours: number | null; // null = 无截止
}

export interface WallpaperTodoItem {
  id: string; // `pm:<id>` | `todo:<id>`
  title: string;
  priority: "P0" | "P1" | "P2" | "P3";
  pinned: boolean;
  endAt: string | null; // ISO 日期
  status: string;
  source: "pm" | "todo";
  recentlyCompleted?: boolean;
}

export interface WallpaperDashboardData {
  overview: WallpaperOverview;
  todoList: WallpaperTodoItem[];
  echo?: string | null; // 阶段 1 始终 null
  generatedAt: string;
}

export interface WallpaperStatus {
  enabled: boolean;
  paused: boolean;
  pauseReason?: "boss_key" | "fullscreen" | "lock" | "manual";
  originalPath: string | null;
  lastRenderedAt: string | null;
  lastRenderedPath: string | null;
  lastError?: string | null;
  spotlightDetected: boolean;
  thirdPartyEngine?: string | null;
}
```

### 0.3 通道注册骨架

**文件**：`apps/desktop/src/bridge/tauri.ts` 的 `CHANNEL_MAP`

```typescript
"tool:wallpaper:dashboard-data": { domain: "wallpaper", action: "dashboard_data" },
"tool:wallpaper:render-once":    { domain: "wallpaper", action: "render_once" },
"tool:wallpaper:apply":          { domain: "wallpaper", action: "apply" },
"tool:wallpaper:restore":        { domain: "wallpaper", action: "restore" },
"tool:wallpaper:pause":          { domain: "wallpaper", action: "pause" },
"tool:wallpaper:resume":         { domain: "wallpaper", action: "resume" },
"tool:wallpaper:status":         { domain: "wallpaper", action: "status" },
"tool:wallpaper:enable":         { domain: "wallpaper", action: "enable" },
"tool:wallpaper:disable":        { domain: "wallpaper", action: "disable" },
"tool:wallpaper:get-config":     { domain: "wallpaper", action: "get_config" },
"tool:wallpaper:set-config":     { domain: "wallpaper", action: "set_config" },
"tool:wallpaper:list-history":   { domain: "wallpaper", action: "list_history" },
```

注：设计 §6.2 把"渲染合成 + 设壁纸"合并叫 `apply`；本 plan 拆为 `render_once`（前端把信息层 PNG 推回后端 → 合成 + 设壁纸）和 `apply`（保留为高层封装：取数据 → 通知前端 canvas → 等截图 → 合成 → 设壁纸的全链路）。`apply` 是后端主动发起的整链路，`render_once` 是前端 canvas 通知"我画好了，给你 PNG"的回写。

### 0.4 Rust 模块骨架

**新增文件**：

- `apps/desktop/src-tauri/src/tools/wallpaper/mod.rs`：`execute(action, payload)` 分发
- `apps/desktop/src-tauri/src/tools/wallpaper/data.rs`：`dashboard_data` 聚合
- `apps/desktop/src-tauri/src/tools/wallpaper/compose.rs`：base 图加载 + 主色调采样 + 合成
- `apps/desktop/src-tauri/src/tools/wallpaper/desktop.rs`：`IDesktopWallpaper` + `SystemParametersInfoW` 双层封装
- `apps/desktop/src-tauri/src/tools/wallpaper/state.rs`：进程内 `WallpaperState`（`LazyLock<RwLock<...>>`），保存 base 图缓存、recently completed、上次渲染时间、暂停状态
- `apps/desktop/src-tauri/src/tools/wallpaper/scheduler.rs`：心跳调度 + 事件触发（Phase 3 实现，Phase 0 仅占位）
- `apps/desktop/src-tauri/src/tools/wallpaper/capture.rs`：从 `wallpaper_poc.rs` 抽取的 `CapturePreview` 调用（Phase 2 完成后再迁）

`tools/mod.rs` 增加 `pub mod wallpaper;` + `"wallpaper" => wallpaper::execute(...)`。

### 验证 Phase 0

- [ ] `pnpm typecheck` 通过
- [ ] `cargo check`（不带 feature）通过
- [ ] `tool:wallpaper:status` 调用返回默认值（enabled=false，原图为空）

---

## Phase 1：Rust 渲染管线

### 1.1 dashboard_data 聚合

**文件**：`tools/wallpaper/data.rs`

```rust
pub fn dashboard_data(payload: &Value) -> Result<Value, String> {
    let today = Local::now().date_naive();
    let conn = db_conn()?;

    let overview = build_overview(&conn, today)?;
    let todo_list = build_todo_list(&conn, today, 20)?;
    let recently = drain_recently_completed(&conn, today)?;
    let merged = merge_recently_completed(todo_list, recently);

    Ok(json!({
        "overview": overview,
        "todoList": merged,
        "echo": Value::Null,
        "generatedAt": Utc::now().to_rfc3339(),
    }))
}
```

**子函数**：

- `build_overview(conn, today)`：复用 `pm_today` 的今日 SQL + Todo 今日 SQL，分别累加 `completed_today`、`total_today`、`p0_pending`、`nearest_deadline_hours`
- `build_todo_list(conn, today, limit)`：跨 `pm_items` + `todo_items` 各拉一批，按设计 §5.2 排序合并去重（`pm_id` 已被 Todo 关联的，跳过 Todo 那条）
- `merge_recently_completed`：v0.5 移除（不再有完成迟滞）

**纯函数抽到 `wallpaper/dashboard_logic.rs`** 并加 `#[cfg(test)]` 单测：

- `is_open_status`：复用 `pm_today.rs` 与 `todo.rs` 的判定（提升为 `pub` 共用），`pm_today.rs::priority_rank` 同步提升
- `is_overdue(item, now)`：`is_open_status(status) && deadline < now`（PM 用 `end_at`，Todo 用 `event_at`，归一化字段后判定）
- `merge_and_dedup_items`（pm 优先于 todo）
- `sort_dashboard_items`（**v0.5 修订排序**）：`pinned desc → is_overdue desc → priority_rank asc → deadline asc → created_at desc`，逾期维度提升至优先级之上
- `format_deadline_label`（今天/明天/N 月 N 日/已逾期 N 天/null）
- `compute_nearest_deadline_hours`（min(deadline) - now，跨 PM/Todo）
- `compute_dashboard_hash`（v0.5 新增）：对 overview + 排序后 todoList 计算稳定 hash（blake3），用于 §14.1 优化点 2 的内容短路

**口径对齐约束**（v0.5 强调）：

- 必须复用 PM / Todo 现有判定函数，禁止在 `wallpaper/` 下重写一套优先级 / 逾期判定
- PM `pm_today.rs` 与 Todo `todo.rs` 的视图自身排序保持原状不变（PM `pinned + sort_order`，Todo `pinned + priority + displayAt`），仅壁纸视图按本节排序

不再有 `recently_completed` 缓存（v0.5 移除完成迟滞机制）。

### 1.2 主色调采样

**文件**：`tools/wallpaper/compose.rs`

```rust
fn sample_color_mode(base: &DynamicImage, region: Region) -> ColorMode {
    let cropped = base.crop_imm(region.x, region.y, region.w, region.h);
    let resized = cropped.resize_exact(60, 80, FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let mut sum = 0.0_f64;
    for px in rgba.pixels() {
        let [r, g, b, _] = px.0;
        sum += relative_luminance(r, g, b);
    }
    let avg = sum / (60.0 * 80.0);
    if avg < 0.5 { ColorMode::Light } else { ColorMode::Dark }
}

fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    let to_linear = |c: u8| {
        let v = c as f64 / 255.0;
        if v <= 0.03928 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * to_linear(r) + 0.7152 * to_linear(g) + 0.0722 * to_linear(b)
}
```

`Region` 由 monitor 物理尺寸 + 贴边位置计算（MVP 仅 `right`）。

### 1.3 base 图加载与缓存

`WallpaperState` 内含：

```rust
pub struct WallpaperState {
    base_cache: HashMap<String /* monitor_id */, BaseCacheEntry>,
    recently_completed: HashSet<ItemKey>,
    last_rendered_at: Option<Instant>,
    paused: bool,
    pause_reason: Option<PauseReason>,
    last_rendered_path: Option<PathBuf>,
    last_error: Option<String>,
    burnout: u8, // 连续失败次数，3 触发熔断
}

pub struct BaseCacheEntry {
    path: PathBuf,
    mtime: SystemTime,
    image: DynamicImage,
}
```

- 读 base 图前检查 `mtime`，与缓存不一致则重新加载（处理 §18 E1：用户手改壁纸）
- 缓存按 monitor 分别保存（阶段 1 仅主屏 1 个 entry）

### 1.4 合成函数

`compose.rs::compose(base, info_layer, region, mode) -> DynamicImage`

- alpha 合成：信息层叠加在 base 的指定 region 上
- 边缘 1 px 极淡描边：合成前对 info_layer 加 `imageproc::drawing::draw_hollow_rect_mut`，颜色按 `mode` 选浅/深
- 输出 `DynamicImage`，由调用方按配置编码 PNG / JPEG

### 1.5 写盘 + 历史清理

`compose.rs::persist(image, format, keep) -> PathBuf`

- 路径：`<data_dir>/wallpapers/rendered/<timestamp>.<jpg|png>`
- JPEG 质量 90（`image::codecs::jpeg::JpegEncoder::new_with_quality(buf, 90)`）
- 写完后扫描目录，按 mtime 倒序保留前 `keep` 张，其余删除

### 1.6 IDesktopWallpaper 封装

`desktop.rs`：

```rust
pub fn set_wallpaper(monitor_index: usize, image_path: &Path) -> Result<(), String>;
pub fn get_wallpaper(monitor_index: usize) -> Result<PathBuf, String>;
pub fn monitor_count() -> Result<usize, String>;
pub fn monitor_device_path(index: usize) -> Result<String, String>;
pub fn monitor_rect(index: usize) -> Result<Rect, String>;  // 物理像素 + DPI
```

实现要点：

- COM 初始化：`CoInitializeEx(None, COINIT_APARTMENTTHREADED)`，每次调用线程 init 一次（用 `OnceLock` 包装）
- `CoCreateInstance(&CLSID_DesktopWallpaper, None, CLSCTX_LOCAL_SERVER)` 拿 `IDesktopWallpaper`
- `set_wallpaper` 内调 `SetPosition(DWPOS_FILL)` 然后 `SetWallpaper(monitorPath, image_path)`
- `monitor_rect` 通过 `GetMonitorRECT(monitorPath)` 拿到边界
- 异常分支：`HRESULT` 失败 → 回退 `SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, path, SPIF_UPDATEINIFILE | SPIF_SENDCHANGE)`，记录 `original_set_method = sysparam`

**新增依赖** `Cargo.toml`（已有 windows 0.61，仅补 features）：

```toml
windows = { version = "0.61", features = [
  # 已有：Win32_System_Com, Win32_System_Com_StructuredStorage, Win32_System_Memory
  "Win32_UI_Shell",
  "Win32_UI_Shell_PropertiesSystem",
  "Win32_UI_WindowsAndMessaging",
  "Win32_Graphics_Gdi",
  "Win32_System_RemoteDesktop",   # WTSRegisterSessionNotification（Phase 3）
  "Win32_System_Power",            # PBT_APMRESUMEAUTOMATIC
] }
```

### 1.7 高层 API：apply / render_once / restore

```rust
pub async fn apply(app: AppHandle) -> Result<Value, String>;
// 1. dashboard_data
// 2. emit("wallpaper://dashboard-data", data) → 前端 hidden WebView
// 3. 等待 render_once 回写（带 timeout 5s）
// 4. compose + persist + set_wallpaper
// 5. 更新 WallpaperState

pub fn render_once(payload: &Value) -> Result<Value, String>;
// payload: { pngBase64: string }
// 写入 WallpaperState.pending_info_layer，notify apply 等待方

pub fn restore(app: AppHandle) -> Result<Value, String>;
// 取 wallpaper.original_path → set_wallpaper
```

### 1.8 备份原图

首次 `enable` 时：

```rust
fn enable() -> Result<Value, String> {
    let monitor_path = desktop::monitor_device_path(0)?;
    let original = desktop::get_wallpaper_by_path(&monitor_path)?;
    if !original.as_os_str().is_empty() && original.exists() {
        set_setting("wallpaper.original_path", original.to_string_lossy())?;
    } else {
        // 提示前端"无法读取原壁纸"，原图为空时降级走纯色
    }
    set_setting("wallpaper.enabled", "true")?;
    Ok(json!({ "ok": true, "original": ... }))
}
```

`disable` / `restore` 按 `exit_behavior` 处理。

### 验证 Phase 1

- [ ] 单测：`merge_and_dedup_items`、`sort_dashboard_items`、`format_deadline_label`、`compute_nearest_deadline_hours`、`sample_color_mode`（mock 1×1 像素图）
- [ ] `cargo test -p lazycat-desktop` 通过
- [ ] 手测：从 PoC canvas 抓出的 PNG → 喂给 `compose + persist + set_wallpaper`，桌面壁纸右侧出现信息层
- [ ] 手测：`restore` 能恢复原图（hash 对比）

---

## Phase 2：信息层 canvas + hidden WebView

### 2.1 信息层路由

**文件**：`apps/desktop/src/main.ts`

```typescript
} else if (currentView === "wallpaper-canvas") {
  import("./WallpaperCanvasApp").then(({ default: mount }) => mount());
}
```

**新增文件**：`apps/desktop/src/WallpaperCanvasApp.ts`

挂载 `WallpaperCanvas.vue`。

### 2.2 WallpaperCanvas.vue

**新增文件**：`apps/desktop/src/components/WallpaperCanvas.vue`

```vue
<script setup lang="ts">
import { ref, onMounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import { invokeToolByChannel } from "@/bridge/tauri";
import type { WallpaperDashboardData } from "@/types/wallpaper";

const data = ref<WallpaperDashboardData | null>(null);
const colorMode = ref<"light" | "dark">("light"); // 后端通过 IPC 推
const ready = ref(false);

onMounted(async () => {
  await listen<WallpaperDashboardData>("wallpaper://dashboard-data", (e) => {
    data.value = e.payload;
  });
  await listen<"light" | "dark">("wallpaper://color-mode", (e) => {
    colorMode.value = e.payload;
  });
  await listen<void>("wallpaper://capture-request", async () => {
    await waitForFrame();
    await invokeToolByChannel("tool:wallpaper:render-once", {});
    // 实际 PNG 字节通过 Rust 的 CapturePreview 直接抓 webview，
    // 前端只负责通知"已就绪"
  });
  ready.value = true;
});

async function waitForFrame(): Promise<void> {
  return new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r())));
}
</script>

<template>
  <div class="wallpaper-canvas" :class="['mode-' + colorMode]">
    <OverviewBlock v-if="data" :overview="data.overview" />
    <TodoList v-if="data" :items="data.todoList" />
    <ExtensionSlot />
  </div>
</template>
```

### 2.3 子组件

- `WallpaperOverviewBlock.vue`：进度环（SVG `<circle>` + `stroke-dasharray`，告警态变红），右侧警戒栏
- `WallpaperTodoList.vue`：按 `data.todoList` 渲染；自适应行数公式按设计 §5.2 计算 `maxLines`，超出显示 `+N 件`；`recentlyCompleted` 项灰色 + 删除线
- `WallpaperExtensionSlot.vue`：阶段 1 留空占位

样式：CSS 变量 + `:root[data-color-mode]` 切换浅/深字、毛玻璃蒙层。

**接入 Element Plus**（与主窗口视觉一致）：

- `WallpaperCanvasApp.ts` 在 `createApp` 后 `app.use(ElementPlus)`，复用主仓 `src/styles/index.css` 的 EP 主题
- 浅/深模式通过 `:root[data-color-mode="dark"]` 覆盖 EP CSS 变量（`--el-text-color-primary` / `--el-bg-color` 等），避免直接改 EP 样式表
- 注意 §05.1 双文件覆盖规则：浅色 `data-color-mode="light"` 下若被 `theme-light.css` 的 `html[data-theme="light"]` 抢占特异度，需在 canvas root 上**不**设 `data-theme` 属性，让 `theme-light.css` 的选择器整体不命中
- hidden WebView 加载 EP 整套样式表（~200 KB）+ JS（~600 KB）一次性成本可接受（仅冷启 +50-100 ms，不影响热路径）
- 进度环用 `el-progress :type="circle"`，告警态通过 `:color` 覆盖；任务行用 `el-tag` 标记优先级与截止；列表容器用纯 CSS Grid，不用 `el-table`（避免表格头/边框样式冲击仪表盘观感）

### 2.4 hidden WebView 创建

**Rust**（迁出 `wallpaper_poc.rs` 的复用部分到 `wallpaper/capture.rs`）：

```rust
pub fn ensure_canvas_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(w) = app.get_webview_window(CANVAS_LABEL) {
        return Ok(w);
    }
    let dpi = current_main_monitor_dpi(app)?;
    let scale = dpi as f64 / 96.0;
    WebviewWindowBuilder::new(
        app, CANVAS_LABEL,
        WebviewUrl::App("index.html?view=wallpaper-canvas".into()),
    )
    .inner_size(360.0 * scale, 800.0 * scale)
    .visible(false)
    .decorations(false)
    .resizable(false)
    .skip_taskbar(true)
    .build()
    .map_err(|e| e.to_string())
}
```

`CANVAS_LABEL = "wallpaper-canvas"`（不与 PoC 的 `wallpaper-poc-canvas` 冲突）。

### 2.5 截图 → Rust 流程

`apply` 内：

```rust
pub async fn apply(app: AppHandle) -> Result<Value, String> {
    let win = ensure_canvas_window(&app)?;
    let data = data::dashboard_data(&Value::Null)?;

    let base = compose::load_base(0)?;       // 主屏
    let region = compose::region_for(&base, Position::Right);
    let mode = compose::sample_color_mode(&base, region);

    app.emit("wallpaper://color-mode", mode.as_str())?;
    app.emit("wallpaper://dashboard-data", &data)?;

    // 等前端渲染完成（首次冷启 max 1.5s，热路径 max 600ms）
    await_canvas_ready(&app, Duration::from_secs(2)).await?;

    // 直接调 CapturePreview 抓 PNG bytes
    let png = capture::capture_window_png(&win).await?;
    let info_layer = image::load_from_memory(&png)?;

    let composed = compose::compose(&base.image, &info_layer, region, mode);
    let format = read_image_format()?;
    let path = compose::persist(&composed, format, history_keep())?;
    desktop::set_wallpaper(0, &path)?;

    state::write(|s| {
        s.last_rendered_path = Some(path.clone());
        s.last_rendered_at = Some(Instant::now());
        s.last_error = None;
        s.burnout = 0;
    });

    Ok(json!({ "ok": true, "path": path.to_string_lossy() }))
}
```

`await_canvas_ready` 通过监听一次 `tool:wallpaper:render-once` 调用或 emit 一个 `wallpaper://canvas-ready` 事件实现。

### 2.6 PoC 退役

PoC 代码继续保留为 dev-only 入口（`wallpaper-poc` / `wallpaper-poc-canvas`）。Phase 2 完成后：

- `wallpaper_poc.rs` 内的 `capture_inner` / `pump_messages` / `read_stream_to_vec` 抽取到 `tools/wallpaper/capture.rs`，PoC 引用之
- PoC 控制台仍能跑（用作单步调试）

### 验证 Phase 2

- [ ] 调 `tool:wallpaper:apply` 一次能跑通完整链路：取数据 → 推 canvas → 抓图 → 合成 → 设壁纸
- [ ] 手测：模拟 1000 条 todo，自适应行数计算正确（`floor((480 - 32) / 44) = 10` 行 + 提示）
- [ ] 浅/深模式切换：在浅色壁纸 / 深色壁纸下分别启用，文字可读性 OK
- [ ] DPI 200% 下信息层不被裁剪（720×1600）
- [ ] `pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web` 通过

---

## Phase 3：调度 / 老板键 / 自动切净 / 退出恢复

### 3.1 心跳调度

**文件**：`tools/wallpaper/scheduler.rs`

```rust
pub fn start(app: AppHandle) {
    tokio::spawn(async move {
        loop {
            let interval = current_interval_min();
            sleep_with_idle_check(interval).await;
            if should_skip().await { continue; }
            let _ = wallpaper::apply(app.clone()).await;
        }
    });
}
```

`should_skip`：

- 暂停态（boss key / fullscreen / lock / manual）
- `wallpaper.enabled = false`
- 内容 hash 与上次相同（v0.5 新增，§14.1 优化点 2）：跳过整条 compose + set 链路
  - 强制刷新（手动"立即刷新" / 老板键恢复 / 唤醒）跳过 hash 检查

`sleep_with_idle_check`：每 60 s 检查 `GetLastInputInfo`，5 min 无操作降频到 60 min。

**空闲恢复立刷**（v0.5 新增）：上一周期 `idle_seconds ≥ 300` 且本周期 `idle_seconds < 30`，立刻打破 sleep，触发一次 `apply`。

### 3.2 事件驱动立刷（v0.5 修订节流策略）

`tools/wallpaper/events.rs` 暴露 `notify_data_changed(reason)`：

- PM `item_create` / `item_update` / `item_change_status` 副作用末尾调
- Todo `item_create` / `item_update` / `item_change_status` 副作用末尾调
- 0 点跨日：`tokio::time::sleep_until` 调度本地 00:00:00

**节流实现**（trailing-edge debounce 5 s）：

```rust
static DEBOUNCE_NOTIFY: LazyLock<Notify> = LazyLock::new(Notify::new);
static LAST_EVENT: LazyLock<Mutex<Instant>> = LazyLock::new(|| Mutex::new(Instant::now()));

pub fn notify_data_changed(_reason: &str) {
    *LAST_EVENT.lock().unwrap() = Instant::now();
    DEBOUNCE_NOTIFY.notify_one();
}

// 后台任务：被通知后等 5 s 静默期才触发 apply
async fn debounce_loop(app: AppHandle) {
    loop {
        DEBOUNCE_NOTIFY.notified().await;
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            let last = *LAST_EVENT.lock().unwrap();
            if last.elapsed() >= Duration::from_secs(5) { break; }
        }
        let _ = wallpaper::apply(app.clone()).await;
    }
}
```

效果：用户连按 3 个完成 → 5 s 后只触发 1 次刷新；区别于 v0.4 的 leading + 30 s 锁定（30 s 内连续操作完全无视觉反馈）。

> **改动点**：`tools/pm.rs` / `tools/todo.rs` 各自的状态变更函数末尾插入 `wallpaper::events::notify_data_changed("pm_item_changed")`。这是少量函数（~6 个），每个 1-2 行。

### 3.3 老板键

**文件**：`main.rs` 的 `register_named_hotkey` 复用

- 启用壁纸时调 `register_named_hotkey(app, "wallpaper-boss", "Ctrl+Alt+W")`
- 在 `handle_main_window_shortcut` 旁新增 `handle_wallpaper_boss_shortcut(app)`：toggle pause 状态，pause 时调 `restore`，resume 时调 `apply`
- 注册失败 → 状态卡片提示（不阻塞功能）

### 3.4 全屏自动切净

`tools/wallpaper/fullscreen.rs`：

```rust
fn check_fullscreen() -> bool {
    if shquns_busy() { return true; }
    if foreground_is_fullscreen() { return true; }
    if foreground_in_blacklist() { return true; }
    false
}
```

- `SHQueryUserNotificationState` → `QUNS_BUSY` / `QUNS_RUNNING_D3D_FULL_SCREEN` / `QUNS_PRESENTATION_MODE`
- `GetForegroundWindow` + `GetWindowRect` 与 monitor rect 完全相同
- 黑名单：读 `wallpaper.fullscreen_blacklist`，比对 `GetWindowThreadProcessId` 拿到的进程名

调度器每 30 s 扫描；进入 → `pause(reason=fullscreen)`，离开 → `resume`。

### 3.5 锁屏 / 休眠

`main.rs` 已有 windows 子类化机制；在主窗口 `WndProc` 里追加：

- `WM_WTSSESSION_CHANGE` + `WTS_SESSION_LOCK` → `wallpaper::pause("lock")`
- `WTS_SESSION_UNLOCK` → `wallpaper::resume()` + 立即触发一次 `apply`
- `WM_POWERBROADCAST` + `PBT_APMRESUMEAUTOMATIC` → 唤醒后 `apply`
- `WM_DISPLAYCHANGE` → 清空 base 图缓存

注册：`WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)`。

### 3.6 退出恢复

`tauri::RunEvent::ExitRequested` 钩子末尾：

```rust
if get_setting("wallpaper.enabled") == "true" {
    let exit_behavior = get_setting("wallpaper.exit_behavior");
    if exit_behavior == "restore_original" {
        let _ = wallpaper::restore_sync();
    }
}
```

`restore_sync` 同步实现，避免 tokio runtime 已 drop。

### 3.7 Spotlight / 第三方引擎检测

`tools/wallpaper/conflicts.rs`：

- 读注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Wallpapers\BackgroundType`，==2 → Spotlight 启用
- 读 `HKCU\Control Panel\Desktop\WallPaper`，路径含 `MicrosoftWindows.Client.CBS_*\\LocalState\\Assets\\` → Spotlight
- 启动后 10 min 内若 `GetWallpaper` 返回路径变化 → 标记疑似 Spotlight / 第三方
- 进程扫描：`wallpaper32.exe` / `wallpaper64.exe` / `Lively.exe` / `DeskScapes11.exe` → `state.third_party_engine`

返回给 status 通道，前端面板显示警告。**不主动改注册表，不强停第三方进程**。

### 3.8 熔断

`apply` 出错时：

```rust
state::write(|s| {
    s.burnout += 1;
    s.last_error = Some(err.clone());
});
if state::read(|s| s.burnout) >= 3 {
    wallpaper::pause_internal("manual");  // 自动暂停
}
```

恢复条件：用户手动点状态卡片"重试"。

### 验证 Phase 3

- [ ] 心跳：5 min 间隔下能稳定刷新；空闲 5 min 后降频到 60 min
- [ ] 老板键 `Ctrl+Alt+W`：toggle 切净 / 恢复，状态正确
- [ ] OBS 全屏 / Chrome 视频全屏 / PPT 演示 → 自动切净
- [ ] 锁屏 → 暂停；解锁 → 立即刷新
- [ ] 系统休眠 → 唤醒后刷新
- [ ] 拔插外接显示器 → 不崩溃，下次刷新基于新 monitor
- [ ] 退出 LazyCat（关托盘）→ 壁纸恢复原图
- [ ] Spotlight 启用 → 状态卡片提示
- [ ] 模拟连续 3 次合成失败 → 自动暂停，状态显示错误

---

## Phase 4：配置面板 + 工具入口

### 4.1 工具入口注册

按 CLAUDE.md §04.6 三处改动：

1. `apps/desktop/src/composables/toolCatalog.ts`：在 `more` 分组（或新建独立分组）加 `{ id: "wallpaper", name: "桌面壁纸", desc: "把工作状态画到桌面壁纸上" }`
2. `apps/desktop/src/tool-registry.ts`：注册 `wallpaper: defineAsyncComponent(() => import('./components/WallpaperPanel.vue'))`
3. 新增 `apps/desktop/src/components/WallpaperPanel.vue`

注：把 PoC 入口（`wallpaper-poc`）保留在 `toolCatalog.ts` 的网络与系统分组（仅 dev 可见，不动）。

### 4.2 WallpaperPanel.vue

按设计 §11.2 四块分组：状态 / 基础 / 隐私与老板键 / 高级。

**核心结构**：

```vue
<el-tabs>
  <el-tab-pane label="状态" name="status">
    <WallpaperStatusCard />     <!-- 启停、立即刷新、上次时间、缩略图、原图、异常 -->
  </el-tab-pane>
  <el-tab-pane label="基础设置" name="basic">
    <WallpaperBasicSettings />  <!-- 启用开关、风格、贴边、刷新间隔、合成格式 -->
  </el-tab-pane>
  <el-tab-pane label="隐私与老板键" name="privacy">
    <WallpaperPrivacySettings /> <!-- 老板键、自动切净、敏感模式、退出策略 -->
  </el-tab-pane>
  <el-tab-pane label="高级" name="advanced">
    <WallpaperAdvancedSettings /> <!-- 黑名单、历史、重置 -->
  </el-tab-pane>
</el-tabs>
```

每个子组件 100-200 行，独立文件，统一通过 `tool:wallpaper:get-config` / `tool:wallpaper:set-config` 读写。

**关键交互**：

- 启用开关 toggle → `enable` / `disable`
- "立即刷新" → `apply`
- "恢复原图" → `restore`
- 重置：`ElMessageBox.confirm('重置后所有壁纸偏好将丢失，原壁纸会立即恢复，是否继续？', '重置壁纸设置', { customClass: 'wallpaper-reset-confirm' })`

样式按 CLAUDE.md §05.1 同步检查 `element-overrides.css` 与 `theme-light.css`。

### 4.3 状态轮询

`WallpaperStatusCard.vue` 用 `useToolInvoke` + `setInterval(5000)` 拉 `tool:wallpaper:status`，在面板未挂载时自动停止（`onUnmounted` 清 interval）。

### 验证 Phase 4

- [ ] 工具入口出现在侧边栏，icon / 描述正确
- [ ] 启用 / 禁用 → 壁纸状态变化，原图备份成功
- [ ] 修改刷新间隔 / 风格 / 位置 → 持久化生效
- [ ] 老板键改键 → 重启后仍生效
- [ ] 重置：弹出二次确认，确认后所有 `wallpaper.*` 设置回默认 + 恢复原图
- [ ] 浅色主题下面板样式正确（无被 `theme-light.css` 覆盖问题）

---

## Phase 5：联调 / 兼容性 / 文档

### 5.1 测试矩阵执行

按设计 §13.6：

| 环境                       | 单 / 多屏 | 缩放      | 验证点              |
| -------------------------- | --------- | --------- | ------------------- |
| Win 10 22H2                | 单屏      | 100%      | 主路径 + 备份/恢复  |
| Win 10 22H2                | 双屏      | 150%/100% | 多屏只设主屏 + DPI  |
| Win 11 23H2 (Spotlight 关) | 单屏      | 100%      | 主路径              |
| Win 11 23H2 (Spotlight 开) | 单屏      | 100%      | 检测 + 提示         |
| Win 11 24H2                | 双屏      | 200%/100% | 高 DPI + 任务栏居中 |

每个环境跑设计 §19.2 的手测清单，记录在 `docs/superpowers/specs/2026-05-05-living-wallpaper-test-log.md`（新增）。

### 5.2 性能验证

按设计 §14.1 预算：

- 首次 < 1500 ms：日志埋点 `apply()` 入口 / 出口 timestamp 差
- 后续 < 600 ms：连跑 10 次取 p95
- 内存：观察 hidden WebView2 进程稳定 ≤ 80 MB
- 磁盘：JPEG 历史 20 张 ≤ 300 MB（每张约 8-15 MB）

### 5.3 文档同步

- `CLAUDE.md` / `AGENTS.md` §04.6 后追加"非 channel 工具"列表里 `wallpaper` 的特殊点（hidden WebView + 后端调度）
- `CLAUDE.md` 04 章新增"04.x 桌面壁纸渲染管线"（数据 → canvas → 截图 → 合成 → set 流程图 + 关键文件路径）
- `process.md` 评估：本任务跨 12+ 文件，复用了多项现有模式（hidden WebView、`with_webview`、IPC emit/listen、`user_settings` 持久化）。需沉淀的经验是"WebView2 CapturePreview + 主进程 tokio 调度组合"——只有当后续再有"hidden WebView 抓图"类需求时才固化

### 5.4 release notes

`CHANGELOG` / GitHub Release 描述：

- 新增"桌面壁纸"工具：把今日任务、待办列表、警戒数据画到桌面壁纸右侧（仅 Windows）
- 配置入口：侧边栏 → 桌面壁纸
- 已知限制：阶段 1 仅支持单屏 + 仪表盘风格 + 右侧贴边，多屏与多风格在阶段 2 开放

### 验证 Phase 5

- [ ] 全测试矩阵通过
- [ ] 性能预算达标
- [ ] 文档同步
- [ ] `pnpm typecheck` + `pnpm test` + `pnpm --filter @lazycat/desktop build:web` 全绿
- [ ] `pnpm release:all:win -- -Tag v0.5.0` 干跑成功（不上传）

---

## 任务依赖图

```
Phase 0 (准备)
  ├─ 0.1 设置 key
  ├─ 0.2 类型定义
  ├─ 0.3 通道注册
  └─ 0.4 Rust 模块骨架

Phase 1 (Rust 渲染管线)
  ├─ 1.1 dashboard_data ────────────┐
  ├─ 1.2 主色调采样                 │
  ├─ 1.3 base 图缓存                │
  ├─ 1.4 合成函数 ──┐               │
  ├─ 1.5 写盘清理 ──┤               ├─> 1.7 apply / render_once
  ├─ 1.6 IDesktopWallpaper ─────────┤
  └─ 1.8 备份原图                   │
                                    │
Phase 2 (canvas + WebView)          │
  ├─ 2.1 路由                       │
  ├─ 2.2 WallpaperCanvas.vue ──┐   │
  ├─ 2.3 子组件 ────────────────┤   │
  ├─ 2.4 hidden WebView 创建    ├──> 2.5 截图链路（依赖 1.7）
  └─ 2.6 PoC 退役               │
                                │
Phase 3 (调度 / 老板键 / 切净)  │
  ├─ 3.1 心跳调度 ──────────────┤
  ├─ 3.2 事件驱动 ──────────────┤
  ├─ 3.3 老板键                 │
  ├─ 3.4 全屏自动切净           │
  ├─ 3.5 锁屏 / 休眠 / 显示变化 │
  ├─ 3.6 退出恢复               │
  ├─ 3.7 Spotlight 检测         │
  └─ 3.8 熔断                   │

Phase 4 (面板)
  ├─ 4.1 工具入口
  ├─ 4.2 4 块分组
  └─ 4.3 状态轮询

Phase 5 (联调)
  ├─ 5.1 兼容性矩阵
  ├─ 5.2 性能验证
  ├─ 5.3 文档
  └─ 5.4 release notes
```

---

## 风险与缓解（实施层面）

| 风险                                                                                    | 缓解                                                                                                             |
| --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `apply` 链路过长（数据 → emit → 等 canvas → 截图 → 合成 → set），任一环卡住整体 timeout | 每环都有独立 timeout（dashboard_data 1s / canvas ready 2s / capture 5s / compose 1s / set 2s）；任一超时计入熔断 |
| `IDesktopWallpaper` 在某些 Win10 环境下 COM 初始化失败                                  | §13.7 已设回退 `SystemParametersInfoW`；首次 `enable` 时跑一次自检并把结果存 `original_set_method`               |
| hidden WebView 内存泄漏                                                                 | §7.5 渲染失败 3 次销毁重建；面板"重置"按钮额外手动重建                                                           |
| 多屏壁纸 API 在双屏 + 不同 DPI 下首次 set 出现黑边                                      | MVP 仅设主屏；其它屏保持原状不动；阶段 2 再处理                                                                  |
| 事件驱动立刷与心跳同时触发导致重复合成                                                  | 30 s 节流 + `state.last_rendered_at` 检查                                                                        |
| Spotlight 反复覆盖 LazyCat 写入的壁纸                                                   | §13.4 仅提示用户手动关 Spotlight，不强斗                                                                         |
| 老板键 `Ctrl+Alt+W` 与其它软件冲突                                                      | §9 注册失败提示 + 用户改键                                                                                       |
| 退出 LazyCat 时 tokio runtime 已 drop 导致恢复失败                                      | 退出钩子用同步 `restore_sync`，直接 COM 调用                                                                     |
| 数据目录磁盘满                                                                          | §18 E7：写盘失败计入熔断                                                                                         |
| `pm.rs` / `todo.rs` 调用 `wallpaper::events::notify_data_changed` 引入隐性依赖          | `notify_data_changed` 内自身 try/catch，失败不影响调用方                                                         |

---

## 已确认决策（2026-05-05 用户确认）

1. **渲染链路**：Rust 主动调 `CapturePreview` 直抓 hidden WebView 渲染缓冲，前端 canvas 仅 emit "已就绪" 信号；不走 base64 IPC 回传。
2. **canvas 技术栈**：复用 Element Plus 组件库，与主窗口视觉一致；hidden WebView 接入主仓 `src/styles/index.css`，浅/深模式通过 `:root[data-color-mode]` 覆盖 EP CSS 变量。
3. **退出 LazyCat 默认行为**：立即恢复原图（`exit_behavior = restore_original`），首次启用时弹一次说明 dialog。
4. **贴边位置 MVP 范围**：仅右侧（按设计 §15.2），左/上/下/四角统一推到阶段 2。

---

## 开工前需用户再次确认的点

1. **PoC 工具入口（`wallpaper-poc`）保留还是删除**？保留可作开发态调试；删除则需移除 `toolCatalog.ts` / `tool-registry.ts` / `wallpaper_poc.rs` / `WallpaperPocCanvasApp.ts` / `WallpaperPocCanvas.vue` / `WallpaperPocPanel.vue`。倾向保留，dev 构建可见，release 构建已通过 `#[cfg(all(windows, debug_assertions))]` 自动剥离。
