# Living Wallpaper · 桌面壁纸仪表盘设计

- **状态**：草案 v0.5（2026-05-05 用户体验对齐修订；v0.4 PoC 通过后修订）
- **日期**：2026-05-05
- **目标版本**：v0.5.x
- **关联**：App.vue、tool-registry.ts、bridge/tauri.ts、tools/wallpaper.rs（新增）、user_settings、PM/Todo 数据
- **关联 spike**：`docs/superpowers/spikes/2026-05-05-wallpaper-screenshot.md`
- **来源**：基于 Todo / PM 现有数据，让 LazyCat 通过桌面壁纸持续呈现工作状态

## 1. 背景

当前 LazyCat 主窗口关闭后，所有任务信息从用户视野中消失。Todo / PM 已有完整的数据模型（今日任务、P0 警戒、截止日期、完成进度等），但只有打开主窗才能看到。

本设计在 Windows 桌面壁纸上叠加一个"右侧仪表盘"，把核心工作状态直接画到壁纸上，让用户即使关掉 LazyCat 也能感知工作节奏。

## 2. 目标与非目标

### 目标

- 让 LazyCat 通过桌面壁纸持续呈现工作状态（任务进度、待办列表、警戒）
- 保留用户原壁纸（不替换，仅在角落叠加信息层）
- 兼容 Windows 10（≥1809）+ Windows 11（22H2/23H2/24H2）
- 自动适应壁纸明度（浅字 / 深字、深玻璃 / 浅玻璃）
- 完整老板键 + 隐私机制（一键切净 / 全屏自动切净）
- 退出 LazyCat 时可恢复原壁纸

### 非目标

- 不做动态壁纸（视频 / 动画）
- 不依赖云服务、不联网
- 不做 macOS / Linux 桌面（按 LazyCat 整体定位 Windows 优先）
- 不做"完全替换原壁纸"或"模糊衬托"（DR-1 明确选定 B 角落叠加）
- 不在 MVP 做模块定制 / 多屏 / 时段切换（移到阶段 2-3）

## 3. 核心交互场景

### 早晨 8:50

用户开机，IDE 还没启动。桌面右侧的仪表盘显示：

- 进度环 0%，"5/8 件"
- 待办列表：📌 设计稿（P0，今天） / 修复闪烁（P1，今天） / 回 #42 / 整理文档（明天） / 写测试（5/7） / ……
- 警戒：⚠ P0×1 · ⏰ 9h 截止
- 扩展位：留空（阶段 2 起接回声短语）

### 中午 11:30

用户刚关掉一个 Chrome 全屏视频，15 分钟刷新周期到。仪表盘已经把第 ① 项划掉，进度环爬到 33%。

### 老板模式

老板从身后走过 → 按 `Ctrl+Alt+W` → 桌面瞬间切回纯原壁纸（信息层消失），再按一次回来。

### 深夜 0:30

用户已关闭 LazyCat。仪表盘按用户配置——「保留最后一帧」或「立即恢复原图」（推荐默认后者）。

## 4. 视觉规范

### 4.1 区域

- 默认右侧贴边 360×800 px
- 纵向居中：顶部留 12% / 底部留 12%（避开任务栏 + 系统托盘）
- 圆角 16 px
- 整体不透明度 0.85，毛玻璃感（背景模糊 + 半透明蒙层）

### 4.2 主色调自适应

每次合成前取右侧 360×800 区域的平均明度：

- 明度 < 0.5 → 浅字（#FFFFFF / #E0E0E0）+ 深玻璃蒙层（rgba(0,0,0,0.6)）
- 明度 ≥ 0.5 → 深字（#1A1A1A / #444）+ 浅玻璃蒙层（rgba(255,255,255,0.7)）

边缘 1px 极淡描边提升可读性。

## 5. 模块组成

精简后 3 个模块自上而下：

| #   | 模块     | 占高    | 说明                           |
| --- | -------- | ------- | ------------------------------ |
| ①   | 概览块   | ~220 px | 进度环 + 警戒灯（合并）        |
| ②   | 待办列表 | ~480 px | 不局限"今日"和"3 件"           |
| ③   | 扩展位   | ~100 px | 阶段 1 留空，阶段 2 接回声短语 |

总计 800 px。

### 5.1 概览块

- 左：进度环（圆环显示今日完成率，中央百分比 + 下方 "X/Y 件"）
- 右：⚠ P0×N · ⏰ 距最近截止 Xh
- **告警态**：P0 ≥ 3 或截止 ≤ 1h → 进度环外圈变红 + 警戒数字加粗

### 5.2 待办列表

- **数据源**：PM items + Todo items 合并去重（去重逻辑：Todo 已通过 `pmItemId` 关联到 PM 的，只显示 PM 一条）
- **过滤**：`status ≠ done` ∧ `status ≠ archived`
- **排序**：`pinned → 已逾期 → P0 → P1 → P2 → P3 → 截止日期升序 → 创建时间降序`
  - "已逾期"维度提升至优先级之上：逾期 P3 排在未逾期 P0 之前，符合用户"逾期 = 立即可见"的常规预期
  - 逾期判定复用 PM `pm_today.rs::is_overdue`（`is_open_status(status) && end_at < today`）和 Todo `todo.rs` 第 1485 行的 `is_overdue` 字段（`is_open_status(status) && event_at < now`）
- **逻辑复用**：合并排序在新增的 `wallpaper/dashboard_logic.rs` 中实现，`priority_rank` / `is_open_status` 直接复用 `pm_today.rs::priority_rank` 与 `todo.rs::is_open_status`（提升为 `pub`），不重写一份。所有改动需保持 PM / Todo 视图原排序行为不变（PM 看板按 `pinned + sort_order`，Todo 列表按 `pinned + priority + displayAt + id desc`，壁纸视图按本节规则）
- **显示数量**：自适应（默认配置约 10 行），超出显示 "+N 件" 提示
  - 公式：`maxLines = floor((listHeight - paddingY * 2) / lineHeight)`，默认 `listHeight=480`、`paddingY=16`、`lineHeight=44` → 10 行；字号变化时按 `lineHeight = fontSize * 1.6` 重算
- **每行格式**：`优先级圆点 + 📌(若 pinned) + 标题(超长截断) + 截止标签`
- **截止标签语义**：`今天 / 明天 / 5月7日 / 已逾期 N 天`
- **逾期项**：标签变红、标题不变（不动主信息）
- **完成即消失**（v0.5 修订）：刚完成的项不再保留迟滞周期，下一次刷新（无论被动心跳还是事件驱动）即从列表移除。原 v0.4 的"灰色删除线 + 1 周期保留"机制移除——理由：用户点完成后的常规预期是"立即消失给我成就感"，跨 15 分钟刷新周期才消失会让用户怀疑没保存。事件驱动立刷流程（§8）已能在 5-10 秒内反映完成状态变化

### 5.3 扩展位

阶段 1 MVP 留空。阶段 2 接：回声短语（"X 天前你完成了 Y"）/ 风暴预报 / 心情天气。

## 6. 数据接入

### 6.1 后端模块

新增 `apps/desktop/src-tauri/src/tools/wallpaper.rs`，在 `mod.rs` 注册。

### 6.2 通道

| 通道                            | 入参                               | 出参                                                            | 说明                               |
| ------------------------------- | ---------------------------------- | --------------------------------------------------------------- | ---------------------------------- |
| `tool:wallpaper:dashboard_data` | `{ tz?: string }`                  | `{ overview, todoList, echo? }`                                 | 一次性返回所有模块数据             |
| `tool:wallpaper:apply`          | `{ informationLayerPath: string }` | `{ wallpaperPath: string }`                                     | 接收信息层 PNG → 合成新图 → 设壁纸 |
| `tool:wallpaper:restore`        | `{}`                               | `{ ok: boolean }`                                               | 恢复原壁纸                         |
| `tool:wallpaper:pause`          | `{}`                               | `{ ok: boolean }`                                               | 暂停（老板键、自动切净都走这）     |
| `tool:wallpaper:resume`         | `{}`                               | `{ ok: boolean }`                                               | 恢复                               |
| `tool:wallpaper:status`         | `{}`                               | `{ enabled, paused, originalPath, lastRenderedAt, lastError? }` | 工具面板状态卡片用                 |

### 6.3 数据查询逻辑

`dashboard_data` 内部聚合（避免多次 IPC）：

```rust
pub fn dashboard_data(tz: Option<String>) -> WallpaperDashboardData {
    let now = Local::now();
    let today = now.date_naive();

    // 概览块
    let overview = OverviewData {
        completed_today: count_completed_today(today),
        total_today: count_total_today(today),
        p0_pending: count_pending_by_priority("P0"),
        nearest_deadline: find_nearest_deadline(now),
    };

    // 待办列表（最多 20 条，前端按高度再裁剪）
    let todo_list = query_pending_items(20);

    WallpaperDashboardData { overview, todo_list }
}
```

复用现有查询：

- 完成统计：复用 PM `pm_today.rs` + Todo today 查询
- P0 计数：直接 SQL 聚合 `pm_items` + `todo_items`
- 最近截止：`min(eventAt, endAt)` 跨 PM/Todo
- 待办列表：基于 PM `item_today_list` + Todo list 查询，按 §5.2 规则合并排序

## 7. 渲染流程

### 7.1 6 步流程

```
1. 后端 dashboard_data() 拿全部数据
2. 前端在 hidden WebView 渲染信息层 HTML（360×800） → 截图为 PNG
3. 后端读原壁纸 → 计算贴角主色调 → 决定明/暗模式
4. 后端 image crate：原图 + 信息层 alpha 合成 → 写到 <数据目录>\wallpapers\rendered\<timestamp>.png
5. 后端 IDesktopWallpaper::SetWallpaper(monitorPath, newPath)
6. 仅保留最近 N 张已合成图（默认 20 张），其余清理
```

### 7.2 渲染方案选择：HTML + WebView2 CapturePreview

候选（v0.2 spike 后修订，详见 `docs/superpowers/spikes/2026-05-05-wallpaper-screenshot.md`）：

| 方案                                              | 状态           | 备注                                                                                           |
| ------------------------------------------------- | -------------- | ---------------------------------------------------------------------------------------------- |
| Tauri 内置 webview screenshot API                 | **否决**       | Tauri 2.10.3 无原生 API，社区 issue 仍在讨论                                                   |
| `tauri-plugin-screenshots` / `xcap`               | **否决**       | 显式跳过 hidden / minimized 窗口                                                               |
| `windows-capture`（WGC）                          | **否决**       | WGC 自身要求窗口被 DWM 合成，hidden 窗口返回黑帧                                               |
| `PrintWindow` + `PW_RENDERFULLCONTENT`            | **降级备用**   | WebView2 occlusion 检测会让 hidden 窗口不渲染；要禁 occlusion + 屏外定位才稳                   |
| **`ICoreWebView2::CapturePreview`**               | **选定**       | WebView2 原生 API，绕过 DWM 合成依赖；只捕 viewport，但本场景信息层尺寸 = viewport，限制不触发 |
| Rust 直接画（`image` + `imageproc` + `rusttype`） | **保留为兜底** | 若 PoC 失败立即切换；MVP 3 个固定模块下完全可行，~150 ms 内出图                                |

选定 **WebView2 CapturePreview** 的理由：

- 不依赖 DWM 合成 / 窗口可见性
- 绕过 Chromium occlusion 优化（WebView2 自身读渲染缓冲）
- 直接输出 PNG / JPEG `IStream`，省 GDI → image crate 的 BGRA 转换
- Tauri 2 通过 `WebviewWindow::with_webview()` 拿 `ICoreWebView2`，`webview2-com` crate 已成熟
- 阶段 2/3 加新模块（CSS 阴影、字体动效）零成本

> **DR-4 修订**：HTML 渲染保留，截图实现从含糊的"WebView 截图"明确为 `ICoreWebView2::CapturePreview`。
> **DR-7（新增）**：截图优先级 `CapturePreview > PrintWindow + PW_RENDERFULLCONTENT > Rust 直接画`；删除 `windows-capture` / `xcap` 兜底。
> **PoC 结果**：2026-05-05 主仓 dev command 代码 PoC 已通过 P1-P5，hidden 窗口下 `CapturePreview` 可输出非黑 PNG，可进入实现 plan。
> **回退触发**：若实现阶段 P1 回归失败（hidden 窗口下 CapturePreview 输出黑图），先尝试 `屏外可见 + CapturePreview`，仍失败则回退 Rust 直接画，DR-4 同步降级。

### 7.3 截图实现细节（v0.2 spike 后修订）

**API 路径**：

```rust
// 伪代码
window.with_webview(|webview| {
    let core = webview.controller().CoreWebView2()?;
    let stream = SHCreateMemStream(None);
    let handler = CapturePreviewCompletedHandler::create(...);
    core.CapturePreview(
        COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
        &stream,
        &handler,
    )?;
})?;
```

**关键约束**：

1. CapturePreview 必须在 WebView2 完成首次 `ContentLoading` 之后调用 → 流程上等前端 `/wallpaper-canvas` 触发 `tool:wallpaper:canvas_ready` IPC 后再抓
2. hidden window size 必须等于 logical size × DPI scale，否则被 viewport 裁剪
3. WebView2 `IsVisible=false` 时能否仍输出非黑帧 = PoC 必测项 P1
4. 输出格式选 PNG（CapturePreview 支持 PNG / JPEG），合成阶段再按用户配置（§11.2）转 JPEG 质量 90 写盘

**PoC 必测项**（2026-05-05 已通过）：

2026-05-05 主仓 dev command 代码 PoC 已验证 `ICoreWebView2::CapturePreview` 可用于 hidden Tauri WebView 截图。PoC 控制台入口为 `wallpaper-poc`，画布入口为 `?view=wallpaper-poc-canvas`。

| 编号 | 验证项                                    | 验收标准                          | 结果 |
| ---- | ----------------------------------------- | --------------------------------- | ---- |
| P1   | hidden 窗口 + CapturePreview 输出非黑 PNG | 与 visible 窗口截图像素 hash 接近 | 通过 |
| P2   | 首次截图耗时                              | < 500 ms（含 WebView2 冷启动）    | 通过 |
| P3   | 后续截图耗时                              | < 200 ms                          | 通过 |
| P4   | DPI 200% 下尺寸                           | 720×1600 PNG 像素正确             | 通过 |
| P5   | Win10 22H2 + Win11 23H2/24H2 行为一致     | 三套环境都能输出                  | 通过 |

**P1 失败的备用顺序**：

1. 先试"屏外可见"：`set_position(LogicalPosition::new(-9999, -9999))` + `set_visible(true)`，再调 CapturePreview
2. 仍失败 → 触发 §7.2 回退到 Rust 直接画

**新增依赖**：

- `webview2-com = "0.38"`（与 wry 0.54 间接依赖版本对齐，避免 `ICoreWebView2` 类型重复）
- `windows = "0.61"`，features 至少包含 `Win32_System_Com`、`Win32_System_Com_StructuredStorage`、`Win32_System_Memory`

### 7.4 信息层渲染目标

- 一个独立的隐藏 Tauri WebView 窗口（off-screen），不参与主窗口生命周期
- 渲染路由：`/wallpaper-canvas`（前端独立路由，仅 hidden WebView 加载）
- 渲染完成后通过 IPC 通知后端"已就绪 → 抓图"

### 7.5 Hidden WebView 生命周期（v0.2 补充）

| 时机                                    | 行为                                           | 理由                              |
| --------------------------------------- | ---------------------------------------------- | --------------------------------- |
| LazyCat 启动且 `wallpaper.enabled=true` | 创建 hidden WebView + 加载 `/wallpaper-canvas` | 避免每次刷新冷启 ~300 ms          |
| 主窗口关闭（系统托盘运行）              | **保活**，继续刷新                             | 满足"关掉 LazyCat 仍刷新壁纸"目标 |
| 系统托盘退出 LazyCat                    | 销毁 + 按 `wallpaper.exit_behavior` 处理壁纸   | 进程结束                          |
| `wallpaper.enabled=false` 切换          | 销毁 hidden WebView                            | 释放内存（约 50-80 MB）           |
| 暂停（老板键 / 自动切净）               | 保活，仅停止刷新调度                           | 恢复时秒开                        |
| 渲染失败 3 次                           | 销毁并重建                                     | 防止 WebView 内存泄漏             |

资源占用估算：常驻 hidden WebView2 进程 ~60 MB，CPU 空闲时 < 1%。

## 8. 刷新策略

- **被动心跳**：默认 15 min（用户可配 5 / 15 / 30 / 60）
- **事件驱动立刷**：完成 / 创建 / 状态切换 / P0 警戒触发 / 进入新一天 0 点
- **节流**（v0.5 修订）：trailing-edge debounce，事件触发后等 5 s 静默期再合并触发一次刷新；连按 3 个完成 → 5 s 后只刷一次。**不再使用 leading + 30 s 锁定**（原方案让用户在 30 s 内的连续操作完全没有反馈）
  - 实现：`tokio::sync::Notify` + 每次事件 reset deadline；deadline 到期且无新事件时触发 `apply`
- **空闲降频**：用户 5 min 无操作时降频到 60 min（省 CPU）
  - 实现：`GetLastInputInfo` 轮询 60 s 一次，比较 `dwTime` 与当前 tick
  - **空闲恢复立刷**（v0.5 新增）：上一周期 idle 时间 ≥ 5 min、本周期 < 30 s → 用户刚回来，立即触发一次刷新（避免用户回桌看到陈旧数据）
- **锁屏暂停**：检测到屏幕锁屏 / 用户切换 → 暂停心跳
  - 实现：注册 `WTSRegisterSessionNotification(NOTIFY_FOR_THIS_SESSION)`，监听 `WM_WTSSESSION_CHANGE` 的 `WTS_SESSION_LOCK` / `WTS_SESSION_UNLOCK` / `WTS_SESSION_LOGOFF`
- **0 点触发**：本地时区 00:00:00，使用 Tauri 主进程 `tokio::time::sleep_until` 调度（仓库当前未引入 cron 库，无需新增依赖）

事件订阅来源：

- PM 通道完成 / 创建 / 状态切换的副作用
- Todo 通道完成 / 创建 / 状态切换的副作用
- 0 点 cron 触发（用户本地时区）

## 9. 隐私与老板键

- 全局快捷键 `Ctrl+Alt+W` toggle 信息层显示（可在工具面板自定义）
  - 实现：Tauri 2.x `tauri-plugin-global-shortcut`
  - 注册失败降级：状态卡片显示"⚠ 老板键 Ctrl+Alt+W 已被其他程序占用，请在面板中改键"，不阻塞功能其他部分
- **自动切净**：检测全屏应用 / 屏幕共享 / 演示软件 → 自动 pause
  - 实现：`SHQueryUserNotificationState()` 返回 `QUNS_BUSY` / `QUNS_RUNNING_D3D_FULL_SCREEN` / `QUNS_PRESENTATION_MODE` 任一即触发；轮询 30 s 一次
  - 兜底：前台窗口 `GetForegroundWindow` + `GetWindowRect` 与 monitor rect 完全相同时也判为全屏
  - **全屏切净触发列表**（v0.5 统一术语）：仅包含明确表示演示 / 录屏 / 会议的软件进程，默认 `obs64.exe` / `obs32.exe` / `powerpnt.exe` / `wpp.exe` / `zoom.exe`。不把 `chrome.exe` / `vlc.exe` 这类日常应用写进默认列表，避免窗口化使用时长期误切净；Chrome / VLC 全屏播放由系统 API 与前台窗口全屏检测兜底
- **敏感模式**（配置项）：标题打码（"▓▓▓▓"），只显示数字 / 图形
  - 状态卡片必须显眼显示"敏感模式已开启" + 一键关闭按钮
  - 开启敏感模式时提供过期时间（默认 2 小时自动关闭，可选：30 分钟 / 2 小时 / 直到手动关闭），避免用户忘记关闭后长期看到打码壁纸
- **退出策略**：用户配置——「保留最后一帧」或「立即恢复原图」（推荐默认后者）
- **卸载兜底**：装载 / 启动时把当前壁纸路径备份到 `user_settings.wallpaper.original_path`
  - 仅首次启用时备份；用户运行期间手动改壁纸不会覆盖备份（见 §18 边界场景 E1）

## 10. 多屏

| 阶段       | 行为                                                              |
| ---------- | ----------------------------------------------------------------- |
| 阶段 1 MVP | 仅主屏。`IDesktopWallpaper::GetMonitorDevicePathAt(0)` 取第一块屏 |
| 阶段 2     | 所有屏同步同一张                                                  |
| 阶段 3     | 每屏独立频道（主屏总览 / 副屏单任务详情）                         |

## 11. 配置入口

### 11.1 接入方式

按 CLAUDE.md §04.6"新增工具标准流程"：

1. `apps/desktop/src/App.vue` 的 `sidebarItems` 注册"桌面壁纸"入口（与 Todo / PM / Inbox 同级）
2. `apps/desktop/src/tool-registry.ts` 注册异步组件
3. 新增 `apps/desktop/src/components/WallpaperPanel.vue`
4. `apps/desktop/src/bridge/tauri.ts` 的 `CHANNEL_MAP` 增加 `tool:wallpaper:*`
5. 后端 `tools/wallpaper.rs` 在 `mod.rs` 注册

### 11.2 面板结构

四块分组：

**状态卡片**

- 当前是否启用、是否暂停
- [立即刷新] / [暂停] 按钮
- 上次刷新时间
- 当前合成图缩略图（点击放大）
- 原壁纸路径 + [恢复原图] 按钮
- 异常提示（COM 失败 / Spotlight 检测 / 第三方引擎检测）

**基础设置**

- 启用桌面壁纸（开关）
- 风格（仪表盘 / 便利贴 / 横幅，MVP 仅仪表盘可用）
- 贴边位置（右侧 / 左侧 / 顶部 / 底部，MVP 仅右侧；阶段 2 开放四角 tl/tr/bl/br）
- 刷新间隔（5 / 15 / 30 / 60 min）
- 合成格式（JPEG 质量 90 / PNG 无损，默认 JPEG）

**隐私与老板键**

- 老板键快捷键（默认 `Ctrl+Alt+W`，可改）
- 检测全屏应用自动切净（开关，默认开）
- 全屏切净触发列表（默认 `obs64.exe` / `obs32.exe` / `powerpnt.exe` / `wpp.exe` / `zoom.exe`，可增删；不预置 chrome / vlc）
- 敏感模式（开关，默认关；开启时选择过期：30 分钟 / 2 小时 / 直到手动关闭，默认 2 小时）
- 退出 LazyCat 时（保留最后一帧 / 立即恢复原图）

**高级**

- 全屏切净触发列表（默认仅含演示 / 录屏 / 会议软件，可自定义增删）
- 合成历史浏览（最近 N 张缩略图）
- 重置所有设置（点击需 ElMessageBox 二次确认，文案"重置后所有壁纸偏好将丢失，原壁纸会立即恢复，是否继续？"）

### 11.3 待办过滤规则

MVP 阶段使用固定过滤 + 排序规则（见 §5.2），不做用户可配置编辑器；阶段 2 才开放。

## 12. 数据库变更

不新建表，仅在 `user_settings` 增加 key：

| key                              | 类型          | 默认值                                                          | 说明                                             |
| -------------------------------- | ------------- | --------------------------------------------------------------- | ------------------------------------------------ |
| `wallpaper.enabled`              | boolean       | false                                                           | 总开关                                           |
| `wallpaper.style`                | string        | `dashboard`                                                     | dashboard / sticky / banner                      |
| `wallpaper.position`             | string        | `right`                                                         | right / left / top / bottom / tl / tr / bl / br  |
| `wallpaper.refresh_interval_min` | number        | 15                                                              | 刷新间隔（分钟）                                 |
| `wallpaper.original_path`        | string        | -                                                               | 原壁纸备份路径                                   |
| `wallpaper.original_set_method`  | string        | `com`                                                           | com / sysparam（恢复时选 API）                   |
| `wallpaper.fullscreen_blacklist` | string (JSON) | `["obs64.exe","obs32.exe","powerpnt.exe","wpp.exe","zoom.exe"]` | 触发自动切净的进程名列表（仅演示/录屏/会议软件） |
| `wallpaper.privacy_mask`         | boolean       | false                                                           | 敏感模式                                         |
| `wallpaper.privacy_mask_until`   | string        | -                                                               | 敏感模式自动关闭时间（ISO，null=直到手动关）     |
| `wallpaper.exit_behavior`        | string        | `restore_original`                                              | keep_last / restore_original                     |
| `wallpaper.modules`              | string (JSON) | `[...]`                                                         | 启用的模块（阶段 2 用）                          |
| `wallpaper.boss_key`             | string        | `Ctrl+Alt+W`                                                    | 老板键                                           |

## 13. Windows 10 / 11 兼容性

### 13.1 目标兼容范围

| 系统            | 版本                   | 支持                            |
| --------------- | ---------------------- | ------------------------------- |
| Windows 10      | ≥ 1809（LTSC 2019 起） | 是                              |
| Windows 11      | 22H2 / 23H2 / 24H2     | 是                              |
| Windows 10      | 1607–1803              | 不主动支持，但 API 兼容（未测） |
| Windows 10      | < 1607                 | 不支持                          |
| Windows 7 / 8.x | 已 EOL                 | 不支持                          |

### 13.2 双层 API 策略

| 路径   | 接口                                          | 用途                       | 触发时机         |
| ------ | --------------------------------------------- | -------------------------- | ---------------- |
| 主路径 | `IDesktopWallpaper` COM 接口                  | 多屏独立壁纸、读写当前壁纸 | 默认             |
| 兜底   | `SystemParametersInfoW(SPI_SETDESKWALLPAPER)` | 单屏壁纸（所有屏同一张）   | COM 初始化失败时 |

`IDesktopWallpaper` 自 Windows 8 起原生支持，Win 10 / Win 11 全部 OK。Win 10 1607+ 实际不会触发兜底，但保留作为安全网。

### 13.3 关键方法

通过 Rust `windows` crate 调用：

- `CoCreateInstance(CLSID_DesktopWallpaper)` → 拿到 `IDesktopWallpaper` 实例
- `GetMonitorDevicePathCount()` → 屏幕数
- `GetMonitorDevicePathAt(index)` → 第 N 块屏的 device path
- `GetWallpaper(monitorPath)` → 当前壁纸路径（用于备份原图）
- `SetWallpaper(monitorPath, imagePath)` → 设壁纸（per-monitor）
- `SetPosition(DWPOS_FILL)` → 填充模式

### 13.4 Win 11 特殊处理

#### Windows Spotlight 冲突

- Win 11 默认可能启用 Spotlight 自动换桌面壁纸 → LazyCat 设的壁纸会被覆盖
- **检测**（v0.2 修订）：Spotlight 启用状态没有单一权威注册表 key，采用三重检测：
  1. 读 `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Wallpapers\BackgroundType`，Type=2 表示桌面 Spotlight（实测值，需 spike 阶段二次确认）
  2. 读 `HKCU\Control Panel\Desktop\WallPaper` 是否指向 `%APPDATA%\Local\Packages\MicrosoftWindows.Client.CBS_*\LocalState\Assets\` 路径（Spotlight 缓存目录）
  3. 启动后 10 min 内若 `IDesktopWallpaper::GetWallpaper` 返回的路径自变化（非 LazyCat 写入），判定为 Spotlight 或第三方覆盖
- **应对**：首次启用 LazyCat 壁纸时弹窗——"检测到 Windows Spotlight 启用，将与本功能冲突。请手动到「设置 → 个性化 → 背景」改为「图片」"（不替用户改注册表）

#### 第三方壁纸引擎冲突（Win 10 / 11 都可能有）

- Wallpaper Engine / DeskScapes / Lively Wallpaper / DesktopHut 等
- **检测进程**：`wallpaper32.exe` / `wallpaper64.exe` / `Lively.exe` / `DeskScapes11.exe`
- **应对**：状态卡片显示"⚠ 检测到 X 正在运行，可能覆盖本功能输出的壁纸"，不强制阻止

#### 任务栏位置

- Win 11 默认底部居中，Win 10 默认底部左对齐
- 仪表盘默认右侧贴边 → 不受任务栏位置影响

#### DWM 桌面合成器

Win 10 / 11 都强制启用，行为一致，无需特殊处理。

### 13.5 HiDPI 处理

- 用 `GetDpiForMonitor(hMonitor, MDT_EFFECTIVE_DPI)` 拿每屏的有效 DPI
- 渲染时按 DPI 缩放：100% → 360×800、125% → 450×1000、150% → 540×1200、200% → 720×1600
- Tauri 2.x 默认 `PerMonitorV2` DPI 感知，无需 tauri.conf.json 改动

### 13.6 测试矩阵（最小集）

| 环境                       | 单 / 多屏 | 缩放        | 验证点                   |
| -------------------------- | --------- | ----------- | ------------------------ |
| Win 10 22H2 (LTSC)         | 单屏      | 100%        | 主路径 + 原图备份/恢复   |
| Win 10 22H2                | 双屏      | 150% / 100% | 多屏 set 单屏 + DPI 缩放 |
| Win 11 23H2 (Spotlight 关) | 单屏      | 100%        | 主路径                   |
| Win 11 23H2 (Spotlight 开) | 单屏      | 100%        | Spotlight 检测 + 提示    |
| Win 11 24H2                | 双屏      | 200% / 100% | 高 DPI + 任务栏居中      |

### 13.7 失败兜底

- COM 初始化失败 → 回退 `SystemParametersInfoW`
- 原壁纸路径读不到（少数情况，比如用户用了第三方引擎）→ 提示"无法读取原壁纸路径，请手动选一张图作为原图备份"
- 连续 3 次合成 / 设置失败 → 自动暂停 + 状态卡片显示错误
- 渲染或 API 任何步骤失败 → 不动壁纸，记录错误日志

## 14. 性能与失败边界

### 14.1 性能预算（v0.2 拆分）

| 阶段     | 单步                              | 预算       | 说明                                           |
| -------- | --------------------------------- | ---------- | ---------------------------------------------- |
| 首次渲染 | 总计                              | < 1500 ms  | 含 hidden WebView 冷启动 + Vue 挂载 + 字体加载 |
|          | dashboard_data 查询               | < 50 ms    | 跨 PM/Todo SQL，复用现有索引                   |
|          | WebView 冷启 + Vue 挂载           | 300–500 ms | 一次性                                         |
|          | 首帧渲染 + 截图                   | 200–400 ms | 含字体加载                                     |
|          | 主色调采样 + 合成                 | 100–200 ms | image crate，4K 壁纸                           |
|          | PNG 编码 + 写盘                   | 200–400 ms | 4K 全屏 PNG，**改用 JPEG 可降至 80–150 ms**    |
|          | `IDesktopWallpaper::SetWallpaper` | 50–200 ms  | 多屏 / Spotlight 残留时偶发 500 ms+            |
| 后续渲染 | 总计                              | < 600 ms   | hidden WebView 已热，无冷启                    |
|          | dashboard_data                    | < 50 ms    |                                                |
|          | 渲染 + 截图                       | 80–150 ms  | hot path                                       |
|          | 合成 + 编码 + set                 | 350–500 ms | 编码占大头，建议 JPEG                          |

> **决策**：合成产物默认 JPEG 质量 90（壁纸用途无需无损），保留 PNG 作为高质量选项。
>
> **优化点 1**：base 图（原壁纸）首次加载后缓存到内存（按 monitor 分），用户未手动改壁纸时不重新读盘。
>
> **优化点 2（v0.5 新增，缓解 SetWallpaper 闪烁）**：每次刷新前对 `dashboard_data` 计算稳定 hash（基于 overview + todoList 的有序序列化），与上次 hash 一致则跳过整条合成 + set 链路，避免每 15 分钟无变化也闪一次桌面。`IDesktopWallpaper::SetWallpaper` 在部分 Win 环境会触发 100-300 ms 桌面整屏黑闪，按 hash 短路后绝大多数空闲周期不再触发。强制刷新（手动点"立即刷新" / 老板键恢复 / 唤醒）跳过 hash 检查。

### 14.2 资源占用

- **合成图磁盘**：默认保留最近 20 张，JPEG 约 8–15 MB（PNG 约 30 MB）
- **常驻内存**：hidden WebView2 进程 ~60 MB，Rust 后端 base 图缓存按 monitor 分别 10–25 MB

### 14.3 失败边界

- 渲染或 API 任何步骤失败 → 不动壁纸，记录错误日志
- 连续 3 次合成 / 设置失败 → 自动暂停 + 状态卡片显示错误（同 §13.7）
- WebView 截图返回空 / 透明图 → 视为失败计入 3 次熔断
- 主色调采样失败 → 默认浅字 + 深玻璃

## 15. MVP 范围划分

### 15.1 阶段 0（spike，已完成）

- B1：调研 hidden window 截图方案 → 完成。详见 `docs/superpowers/spikes/2026-05-05-wallpaper-screenshot.md`
- B1 代码 PoC：主仓 dev command 已通过 P1-P5（用户手动确认功能正常）
- 结论：选定 `ICoreWebView2::CapturePreview`，回退方案 Rust 直接画
- 阶段 1 可直接进入实现 plan；PoC 代码在实现阶段可选择保留为开发入口或迁移为正式 wallpaper 渲染服务

### 15.2 阶段 1（MVP，目标 2 周）

- 单屏 + 仪表盘风格固定
- 3 个模块固定不可定制（概览 / 待办 / 扩展位空白）
- 默认 15 min + 事件驱动
- 老板键 + 退出恢复 + 原图备份
- 主色调自适应
- Win 10 22H2 + Win 11 23H2 主路径手测

### 15.3 阶段 2（完善）

- 4 个位置可选 + 风格三选一（仪表盘 / 便利贴 / 横幅）
- 模块开关 + 排序 + 用户自定义待办过滤规则
- 时段切换 4 套布局（晨报 / 中午 / 晚冲刺 / 明日预告）
- 全屏白名单自动切净 + 敏感模式
- 扩展位填充（回声短语 / 心情天气）

### 15.4 阶段 3（扩展）

- 多屏支持（v2 同步、v3 独立频道）
- 接回连续打卡 / 心电图作为扩展位可选模块
- 月末壁纸合集 / 桌面回放
- 与脑洞模块联动（风暴预报 / 情绪记录 / 回声墙）

## 16. 关键设计决策记录

### DR-1：与原壁纸的关系——B 角落叠加

候选：A 完全接管 / B 角落叠加 / C 模糊衬托 / D 模板合成

选定 **B 角落叠加**。理由：保留用户原壁纸接受度最高，工程上等价于"原图合成信息层"。

### DR-2：风格——方案 2 仪表盘

候选：方案 1 便利贴（迷你低侵入）/ 方案 2 仪表盘（信息密集操盘感）/ 方案 3 报刊横幅（仪式感）

选定 **方案 2 仪表盘**。其它两种作为风格选项保留在阶段 2 启用。

### DR-3：模块精简

原 6 模块（进度环 / 今日 3 件 / 警戒 / 连续打卡 / 心电图 / 扩展位）精简为 3 模块：

- 进度环 + 警戒栏 → 合并为概览块
- 今日 3 件 → 待办列表（不局限"今日"和"3 件"）
- 移除连续打卡（移到阶段 3 可选模块）
- 移除心电图（移到阶段 3 可选模块）

### DR-4：渲染 HTML + WebView2 CapturePreview（v0.2 spike 后修订）

候选：HTML 渲染截图 / Rust 直接画

选定 **HTML 渲染**。截图实现明确为 `ICoreWebView2::CapturePreview`（非含糊的"WebView 截图"）。2026-05-05 代码 PoC 已通过 P1-P5，确认 hidden Tauri WebView 可输出非黑 PNG。理由：保真度高、复用前端栈、未来加模块零成本。详见 spike 报告 `docs/superpowers/spikes/2026-05-05-wallpaper-screenshot.md`。

### DR-5：配置入口为独立工具

候选：放 PM / Todo 设置抽屉 / 独立工具

选定 **侧边栏独立工具**。理由：跨 PM + Todo 数据、配置项较多、需要展示状态卡片和合成历史等非配置内容。

### DR-6：合成产物默认 JPEG

详见 §14.1。壁纸用途无需无损，JPEG 质量 90 把 PNG 编码 200-400 ms 降到 80-150 ms，磁盘占用减半。

### DR-7：截图实现优先级（v0.2 新增）

候选：Tauri 内置 / `tauri-plugin-screenshots` / `windows-capture` (WGC) / `PrintWindow` / `ICoreWebView2::CapturePreview` / Rust 直接画

选定优先级：**`CapturePreview` > `PrintWindow + PW_RENDERFULLCONTENT` > Rust 直接画**。2026-05-05 代码 PoC 已确认首选路径可行。

否决理由：

- Tauri 内置：2.10.3 无原生 API
- `xcap` / WGC：自身要求窗口被 DWM 合成，hidden 窗口黑帧
- `windows-capture` crate：同上 + 需关 occlusion + 屏外定位

详见 spike 报告。

## 17. 与其他脑洞模块的联动（备忘）

本设计为后续脑洞模块预留扩展位 / 联动接口（均属于阶段 2-3 范围）：

- **风暴预报** → 接入扩展位（壁纸顶部浮"乌云"显示未来 7 天截止密度）
- **情绪记录** → 概览块加"心情天气"icon
- **回声墙** → 扩展位的回声短语数据源（"X 天前你完成了 Y"）
- **屏保昨日剧场** → 复用 `wallpapers/rendered/` 历史图作为素材
- **月末壁纸合集** → 复用 `wallpapers/rendered/` 月度归档

## 18. 边界场景（v0.2 新增）

| 编号 | 场景                                        | 处理                                                                                                                                                                                   |
| ---- | ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| E1   | 用户在 LazyCat 运行时手动改了壁纸           | 后端轮询 60 s 检测 `IDesktopWallpaper::GetWallpaper` 路径变化；若非 LazyCat 写入路径，更新 base 图缓存但**不更新** `wallpaper.original_path`（保护首次备份），下次刷新基于新 base 合成 |
| E2   | 多次启用 / 禁用                             | `wallpaper.original_path` 仅在首次启用时写入；禁用时按 `exit_behavior` 处理；再次启用时若 `original_path` 仍存在且文件可读，直接复用                                                   |
| E3   | 外接显示器分辨率变化 / 拔插                 | 监听 `WM_DISPLAYCHANGE`，清空 base 图缓存 + 已合成图缓存，下次刷新时重新拉取 monitor device path 与 DPI                                                                                |
| E4   | `original_path` 文件被用户删除              | 渲染前 `Path::exists` 检查；不存在则状态卡片提示"原壁纸文件已丢失，请手动选一张"，恢复操作降级为设纯色壁纸                                                                             |
| E5   | 全局热键注册失败                            | 见 §9，状态卡片提示 + 不阻塞功能                                                                                                                                                       |
| E6   | hidden WebView 进程被外部强制结束           | 渲染失败计入 3 次熔断；熔断后状态卡片显示"⚠ 渲染进程异常，已暂停。点击重试"                                                                                                            |
| E7   | 数据目录磁盘满                              | 写盘失败 → 跳过本次刷新 + 记日志；连续 3 次失败触发 §13.7 熔断                                                                                                                         |
| E8   | 系统休眠 / 唤醒                             | `WM_POWERBROADCAST` 监听 `PBT_APMRESUMEAUTOMATIC`，唤醒后立即触发一次刷新（数据可能已过期）                                                                                            |
| E9   | 跨日（0:00 切换）时正在渲染                 | 0 点立刷被 30 s 节流吃掉时，下次心跳自然带入新日期；不做特殊处理                                                                                                                       |
| E10  | Spotlight / 第三方引擎在 LazyCat 运行中开启 | E1 检测到外部覆盖会触发提示，但不主动停止刷新（用户可手动暂停）                                                                                                                        |

## 19. 测试边界（v0.2 新增）

按 CLAUDE.md §07.5 要求按影响面验证。

### 19.1 单元测试（`*.test.ts` / Rust `#[cfg(test)]`）

纯函数：

- `mergeAndDedupItems`：PM + Todo 合并去重逻辑（覆盖 pmItemId 关联、纯 Todo、纯 PM、空集）
- `sortDashboardItems`：排序稳定性（pinned 优先、同优先级按截止、无截止落最后）
- `formatDeadlineLabel`：今天 / 明天 / N 天后 / 已逾期 N 天 / 跨年（覆盖时区边界）
- `pickColorMode`：主色调判定（明度阈值、空图、纯色）
- `computeMaxLines`：自适应行数公式
- Rust：`dashboard_data` SQL 聚合（in-memory sqlite fixture）
- Rust：完成迟滞缓存的写入 / 读取 / 清空

### 19.2 手动测试矩阵（沿用 §13.6 + 补充）

| 场景                              | 验证点                                        |
| --------------------------------- | --------------------------------------------- |
| 启用 → 设壁纸 → 恢复              | 原图能 100% 还原（hash 比对）                 |
| 启用 → 系统重启 → 启动 LazyCat    | 自动恢复刷新；`original_path` 仍指向用户原图  |
| 老板键连按                        | toggle 状态正确，不重复合成                   |
| Spotlight 开启时启用              | 提示弹窗出现                                  |
| 拔插外接显示器                    | base 图缓存清空 + 下次刷新基于新 monitor 数据 |
| 全屏 OBS / Chrome 视频 / PPT 演示 | 自动暂停                                      |
| 锁屏 → 解锁                       | 解锁后立即刷新一次                            |
| 系统休眠 → 唤醒                   | 唤醒后触发刷新                                |

### 19.3 不做自动化的部分

- COM `IDesktopWallpaper` 调用：依赖真实 Windows 桌面，仅手测
- 截图 API：仅 spike 阶段一次性验证
- 主色调采样的视觉效果：人工评估

### 19.4 推送前最低要求

- `pnpm typecheck`
- `pnpm test`（含本设计新增的纯函数测试）
- 手测 §13.6 测试矩阵的最小集（Win 10 22H2 单屏 100% + Win 11 23H2 单屏 100%）
- e2e 仅在 §11 配置面板首次接入时跑一遍 `pnpm test:e2e` smoke

## 20. 修订总结

### v0.2（2026-05-05 spec review 后）

| 章节  | 变更                                                  |
| ----- | ----------------------------------------------------- |
| §5.2  | 自适应行数公式 + 完成迟滞实现位置                     |
| §7.2  | DR-4 反向论证 + 回退方案                              |
| §7.3  | 加入 B1 spike 任务定义                                |
| §7.5  | 新增：Hidden WebView 生命周期表                       |
| §8    | 空闲 / 锁屏 / 0 点的具体 Windows API                  |
| §9    | 老板键、自动切净的具体 API + 注册失败降级             |
| §11.2 | 重置确认 UX + 合成格式选项 + 位置 MVP/阶段 2 范围统一 |
| §13.4 | Spotlight 检测改三重判定                              |
| §14   | 性能预算拆首次 / 后续 + JPEG 决策 + base 图缓存       |
| §15.1 | 新增 阶段 0 spike                                     |
| §18   | 新增：边界场景表（10 项）                             |
| §19   | 新增：测试边界（单测 / 手测 / 推送前最低要求）        |
| §20   | 本表                                                  |

### v0.4（2026-05-05 B1 代码 PoC 后）

| 章节   | 变更                                                                              |
| ------ | --------------------------------------------------------------------------------- |
| §7.2   | 记录 P1-P5 代码 PoC 已通过，首选 `ICoreWebView2::CapturePreview` 可进入实现       |
| §7.3   | PoC 必测项从待执行改为已通过；依赖版本固定为 `webview2-com 0.38` + `windows 0.61` |
| §15.1  | 阶段 0 更新为 spike + 代码 PoC 均完成，阶段 1 可进入实现 plan                     |
| §16    | DR-4 / DR-7 补充 PoC 通过结论                                                     |
| 状态行 | 标记 v0.4                                                                         |

### v0.5（2026-05-05 用户体验对齐修订）

| 章节             | 变更                                                                                    | 动机                                                |
| ---------------- | --------------------------------------------------------------------------------------- | --------------------------------------------------- |
| §5.2             | 排序：`pinned → 已逾期 → P0..P3 → 截止 → 创建`                                          | 逾期 P3 不应排在未逾期 P0 之后                      |
| §5.2             | 排序逻辑复用 PM `priority_rank` + `is_open_status`，新增 `wallpaper/dashboard_logic.rs` | 与现有任务清单口径对齐，不重写一份                  |
| §5.2             | 移除完成迟滞机制；删除 `recentlyCompleted` 字段                                         | 用户点完成后预期立即消失，跨周期保留违反直觉        |
| §5.2             | "默认配置约 7 行" 改为 "约 10 行"                                                       | 与公式 `floor((480-32)/44)` 一致                    |
| §8               | 节流改为 trailing-edge debounce 5 s，删 leading + 30 s 锁定                             | 30 s 锁定让连续完成无视觉反馈                       |
| §8               | 新增空闲恢复立刷（idle ≥ 5 min 后回归立刷）                                             | 用户离开 30 min 回桌看到陈旧数据                    |
| §9               | 全屏切净触发列表去掉 chrome / vlc，仅留演示 / 录屏 / 会议软件                           | chrome.exe 整进程进黑名单导致日常窗口化使用长期切净 |
| §9 / §11.2       | 统一术语为"全屏切净触发列表"，删除"白名单"措辞                                          | §9 写白名单 §12 字段叫 blacklist 语义反向           |
| §9 / §11.2 / §12 | 敏感模式新增过期时间（默认 2h）+ 状态卡片显眼提示                                       | 防止用户忘记关闭后长期看到打码壁纸                  |
| §14.1            | 新增 dashboard hash 短路：内容无变化时跳过 SetWallpaper                                 | 缓解 SetWallpaper 100-300 ms 桌面黑闪               |
| 状态行           | 标记 v0.5                                                                               |
