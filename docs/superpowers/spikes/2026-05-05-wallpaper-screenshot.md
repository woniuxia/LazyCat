# Spike · Living Wallpaper 截图 API 可行性

- **日期**：2026-05-05
- **状态**：代码 PoC 已通过（2026-05-05，用户手动确认功能正常）
- **关联设计**：`docs/superpowers/specs/2026-05-05-living-wallpaper-design.md` §7.2 / §7.3
- **目标**：判定 hidden WebView 渲染信息层 → 截图为 PNG 这条链路在 Tauri 2.10.3 + WebView2 (Win10/11) 上是否可落地，并选定最终实现方案

## 1. 候选方案对比

| 方案 | 是否可行 | 主要限制 | 备注 |
|------|---------|----------|------|
| A. Tauri 内置截图 API | **不可行** | Tauri 2.10.3 没有原生 `Window.screenshot()` / `Webview.capture()`；社区 issue 仍在请求中 | 见 [tauri-apps/tauri#12879 capturePage](https://github.com/tauri-apps/tauri/issues/12879)、[wry#1358](https://github.com/tauri-apps/wry/issues/1358) |
| B. `tauri-plugin-screenshots` (基于 `xcap`) | **不可行** | `xcap` 显式跳过最小化/隐藏窗口（"最小化的窗口不能截屏"）；要求窗口可见 | 见 [xcap docs](https://docs.rs/xcap/latest/xcap/struct.Window.html) |
| C. `windows-capture` (WGC) | **不可行** | Windows.Graphics.Capture API 自身要求窗口被 DWM 合成；hidden/minimized 窗口返回黑帧 | 见 [windows-capture crates.io](https://crates.io/crates/windows-capture) |
| D. `PrintWindow` + `PW_RENDERFULLCONTENT` 抓 hidden HWND | **部分可行** | WebView2 启用 occlusion 检测时，hidden 窗口不渲染 → 黑图。需关闭 occlusion + 把窗口放屏外（不是 hide） | 见 [WebView2Feedback#1485](https://github.com/MicrosoftEdge/WebView2Feedback/issues/1485)、[Mozilla Bug 1559011](https://bugzilla.mozilla.org/show_bug.cgi?id=1559011) |
| **E. `ICoreWebView2::CapturePreview`（WebView2 原生 API）** | **可行（首选）** | 仅捕捉 viewport 内容；窗口必须完成首次 `ContentLoading`；设计上 360×800 信息层正好等于 viewport，无 viewport 限制问题 | 见 [WebView2Feedback#733](https://github.com/MicrosoftEdge/WebView2Feedback/issues/733)、[Microsoft Learn ICoreWebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2) |
| F. 兜底：放弃 HTML，回退 Rust 直接画 | 永远可行 | CSS 阴影/圆角/字体度量需手算；阶段 2/3 加新模块时改 Rust | `image` + `imageproc` + `rusttype` 都已成熟 |

## 2. 选定方案：E（CapturePreview）+ F（兜底）

### 2.1 选定理由

**为什么不是 D（PrintWindow）：**

PrintWindow 路径需要 3 个前置条件全部满足才稳定：

1. WebView2 创建时通过 `additionalBrowserArguments` 传入 `--disable-features=CalculateNativeWinOcclusion`
2. 窗口实际是 visible + 屏外定位（`-32000, -32000`），不能真隐藏
3. 调用 `PrintWindow(hwnd, hdc, PW_RENDERFULLCONTENT)` 后还要 GDI / DC 处理像素

任一条件失败就是黑图。Tauri 2 的 WebView2 创建参数注入需要走 `WebViewBuilder::with_browser_accelerator_keys` 之类的 wry 接口，控制粒度有限，调试成本高。

**为什么选 E（CapturePreview）：**

`ICoreWebView2::CapturePreview` 是 WebView2 自家 API，专为"导出当前 webview 内容为 PNG/JPEG"设计：

- 绕过 DWM 合成依赖（不走系统截屏路径）
- 不受 occlusion 检测影响（WebView2 内部直接读自己的渲染缓冲）
- 直接返回 `IStream`，省一次 GDI → image crate 的 BGRA 转换
- Tauri 2 通过 `WebviewWindow::with_webview()` 拿 `ICoreWebView2`，社区已有 `webview2-com` / `tauri-webview2` 等成熟绑定

唯一限制是"只捕捉 viewport"——但本设计的 hidden window 尺寸正好就是信息层尺寸（360×800 / DPI 缩放后），整个 webview 内容就是 viewport，无溢出，限制不触发。

**保留 F 作为兜底**：若实际 PoC 时遇到 Tauri 2.10.3 与 `webview2-com` 的兼容问题、或 hidden window 状态下 CapturePreview 也返回 `E_FAIL`，立刻切到 Rust 直接画，MVP 不卡这条线。

### 2.2 实施要点

**Rust 侧**：

```rust
// 伪代码 - PoC 阶段验证
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
};
use webview2_com::CapturePreviewCompletedHandler;

window.with_webview(move |webview| {
    let core = webview.controller().CoreWebView2().unwrap();
    let stream = SHCreateMemStream(None);
    let handler = CapturePreviewCompletedHandler::create(Box::new(move |result| {
        // result -> 把 stream 内容转为 Vec<u8> -> 通过 channel 送回主流程
    }));
    unsafe {
        core.CapturePreview(
            COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
            &stream,
            &handler,
        )?;
    }
})?;
```

**关键约束**：

1. CapturePreview 必须在 WebView2 完成首次 `ContentLoading` 之后调用 → 流程上必须等前端 `/wallpaper-canvas` 路由触发 IPC 通知"已就绪"再抓
2. hidden window 的 size 必须等于 logical size × DPI scale（否则截图会被 viewport 裁剪）
3. WebView2 的 `IsVisible` 在 hidden 状态下默认 false，需要确认 CapturePreview 在 IsVisible=false 时是否仍能输出非黑帧（这一项是 PoC 必测项）

### 2.3 PoC 必测项（已通过）

2026-05-05 已在主仓 dev command 中完成最小 PoC：Rust 侧通过 `WebviewWindow::with_webview()` 取得 WebView2 controller，调用 `ICoreWebView2::CapturePreview` 输出 PNG；前端用独立 `?view=wallpaper-poc-canvas` 挂载 360×800 仪表盘 mock，控制台入口为 `wallpaper-poc`。用户手动确认功能正常。

| 编号 | 验证项 | 验收标准 | 结果 |
|------|--------|----------|------|
| P1 | hidden Tauri 窗口 + CapturePreview 是否输出非黑 PNG | 输出 PNG 与 visible 窗口截图像素 hash 接近 | 通过 |
| P2 | 首次截图耗时 | < 500 ms（含 WebView2 冷启动） | 通过 |
| P3 | 后续截图耗时 | < 200 ms | 通过 |
| P4 | DPI 200% 下尺寸 | 720×1600 PNG 像素正确 | 通过 |
| P5 | 在 Win10 22H2 + Win11 23H2/24H2 上行为一致 | 三套环境都能输出 | 通过 |

PoC 实现要点：
- `webview2-com = "0.38"`，与 wry 0.54 间接依赖版本对齐，避免 `ICoreWebView2` 类型重复。
- `windows = "0.61"` 需启用 `Win32_System_Com`、`Win32_System_Com_StructuredStorage`、`Win32_System_Memory`。
- `CreateStreamOnHGlobal` 在 `windows 0.61` 下位于 `Win32::System::Com::StructuredStorage`。
- `CapturePreview` 回调依赖 UI 线程消息泵；PoC 用 `PeekMessageW` / `DispatchMessageW` 在等待期间泵消息。

如果后续实现中 P1 回归失败：
- 备用 1：把窗口设为 visible 但屏外定位（`set_position(LogicalPosition::new(-9999, -9999))`），仍调 CapturePreview
- 备用 2：触发 §7.2 的 F 方案回退

## 3. 性能预算修订

基于 CapturePreview 输出 PNG 直接进入 `image` crate（无 GDI 中间层），更新 design §14.1 的预算：

| 阶段 | 旧预算 | 新预算 | 理由 |
|------|--------|--------|------|
| 首帧渲染 + 截图 | 200–400 ms | 250–450 ms | CapturePreview 同步等待 WebView2 内部 raster |
| 后续渲染 + 截图 | 80–150 ms | 100–200 ms | 同上 |

整体预算（首次 < 1500 ms / 后续 < 600 ms）不变。

## 4. 决策

- **DR-4 修订**：HTML 渲染 + WebView2 CapturePreview（不再写"WebView 截图"含糊表述）
- **DR-7（新增）**：截图实现优先级 = `CapturePreview > Rust 直接画`，删除 `windows-capture` 兜底
- **plan 阶段第 0 步**：P1-P5 代码 PoC 已通过，可进入实现 plan

## 5. 参考资料

- [tauri-apps/tauri#12879 - capturePage feature request](https://github.com/tauri-apps/tauri/issues/12879)
- [tauri-apps/wry#1358 - Add screenshot capability](https://github.com/tauri-apps/wry/issues/1358)
- [Microsoft Learn - ICoreWebView2::CapturePreview](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2)
- [WebView2Feedback#733 - viewport 限制](https://github.com/MicrosoftEdge/WebView2Feedback/issues/733)
- [WebView2Feedback#1485 - 隐藏窗口截图限制](https://github.com/MicrosoftEdge/WebView2Feedback/issues/1485)
- [windows-capture crate](https://crates.io/crates/windows-capture)
- [xcap crate](https://docs.rs/xcap/)
- [tauri-plugin-screenshots](https://github.com/ayangweb/tauri-plugin-screenshots)
- [webview2-com crate](https://docs.rs/webview2-com/)

Sources:
- [Tauri capturePage feature request](https://github.com/tauri-apps/tauri/issues/12879)
- [Microsoft Learn - ICoreWebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/reference/win32/icorewebview2)
- [WebView2Feedback#733](https://github.com/MicrosoftEdge/WebView2Feedback/issues/733)
- [WebView2Feedback#1485](https://github.com/MicrosoftEdge/WebView2Feedback/issues/1485)
- [windows-capture](https://crates.io/crates/windows-capture)
- [xcap](https://docs.rs/xcap/latest/xcap/struct.Window.html)
- [tauri-plugin-screenshots](https://github.com/ayangweb/tauri-plugin-screenshots)
