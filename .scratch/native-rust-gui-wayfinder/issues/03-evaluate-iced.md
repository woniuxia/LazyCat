# 评估 Iced 对 LazyCat 的适配度

Type: research
Status: resolved

## Question

基于截至当前的官方资料和真实 Windows 桌面应用证据，Iced 是否满足 LazyCat 的硬性约束与优先级？重点核实许可证、稳定性与版本演进、Elm 式状态架构、组件与主题生态、中文输入法、键盘与焦点、高 DPI、多窗口、异步任务、托盘/全局快捷键集成、测试能力、二进制与发布打包；明确已证实事实、未知项、否决风险及必须通过原型验证的假设。

## Answer

研究基线：2026-08-27。Iced 当前稳定版为 0.14.0（2025-12-07）；上游 `master` 已是 0.15.0-dev。

### 结论

**Iced 满足 MIT、纯 Rust/无 WebView、Elm 式状态管理、多窗口、异步和离线资源等基础硬约束，可保留为一级候选；但不应在原型前定为胜出框架。** 0.14.0 刚补齐 IME、无头测试和端到端测试等关键能力，官方仍明确称其为 experimental software；Windows 10/11、微软拼音、混合 DPI 和系统 shell 没有官方验收矩阵。它对 LazyCat 最大的结构性风险有两个：

1. Iced 每次从应用状态重建 `view`，控件内部状态由运行时 widget tree 续接；页面从树中移除或 widget tag 改变时子树会重建。LazyCat 的工具标签页若只渲染当前页，文本值可由应用状态保留，但光标/选区、焦点、滚动位置及复杂控件内部状态不会因此自动满足“关闭前持续保留”的契约。
2. Iced 0.14.0 没有托盘、系统全局快捷键或单实例公共 API。`daemon` 能在无窗口时维持运行，`window::run` 能取得 raw window handle，但托盘/热键仍要与 Win32/winit 事件循环集成；这条路径必须用 Windows shell 原型证明生命周期可靠。

若动态标签页、中文 IME、Windows shell、混合 DPI 和发布原型全部通过，并接受锁定 0.14.x/封装升级边界的维护方式，Iced 可进入终选。若必须跟随开发分支或维护 Iced fork 才能满足这些路径，按“长期可维护性优先”应淘汰。

### 硬约束判定

| 约束 | 判定 | 证据与边界 |
| --- | --- | --- |
| 免费闭源商业发行，不购买许可证、不承担 GPL 义务 | **通过（仍需锁版依赖审计）** | Iced 0.14.0 本体为 MIT，允许使用、修改、分发和销售，仅要求保留版权与许可声明；winit、wgpu 及候选 `tray-icon`/`global-hotkey` 均公开为 Apache-2.0/MIT。正式采用仍需对 LazyCat 锁文件和实际启用 feature 做 `cargo deny`/`cargo audit`，不能用顶层许可证替代传递依赖审计。 |
| Windows 10/11，纯 Rust，无 WebView/JS | **有条件通过** | 官方 README 声明 Windows 跨平台支持；默认渲染路径是 wgpu（含 DX12）并可带 tiny-skia 软件渲染器，无 WebView/JavaScript。官方没有像 Slint 那样给出 Windows 10/11 测试矩阵，因此 OS 版本、GPU/驱动和软件回退仍需实机验证。 |
| 主窗口、Spotlight、多窗口 | **原则上通过** | `window::open`、窗口 id、按窗口 `view/theme/scale_factor` 和官方 `multi_window` 示例均是稳定版源码能力；`daemon` 可不创建初始窗口，并在所有窗口关闭后继续运行。Spotlight 的无边框、前置和焦点恢复仍受 Win32 前台激活规则约束。 |
| 托盘、全局快捷键、单实例 | **框架本体不满足，允许外接后有条件通过** | Iced 无这些公共 API。可用 `tray-icon`/`global-hotkey` 或现有 `windows` 适配层；两种第三方 crate 均要求 Windows 线程上有 Win32 event loop，托盘还建议 winit 用户把事件转发到 event loop。Iced 没有直接提供该集成的正式示例。 |

### 已证实能力

**版本、维护和稳定性**

- 0.14.0 是截至基线日的最新稳定 release；此前 0.13.1 发布于 2024-09-19，说明稳定版发布间隔较长。上游 `master` 在基线日仍有提交，项目并非停更，但其 Cargo 版本已是 0.15.0-dev，Rust 下限从 0.14.0 的 1.88 升至 1.93，wgpu 从 27 升至 29，winit 又改为上游 git revision。[0.14.0 release](https://github.com/iced-rs/iced/releases/tag/0.14.0)；[0.14.0 Cargo.toml](https://github.com/iced-rs/iced/blob/0.14.0/Cargo.toml)；[master Cargo.toml](https://github.com/iced-rs/iced/blob/master/Cargo.toml)；[master commits](https://github.com/iced-rs/iced/commits/master/)
- 官方 README 和 crate 顶层文档都明确写着 Iced 是 experimental software；0.14.0 虽新增 reactive rendering、time-travel、IME、headless testing、端到端测试、table/grid 等大量能力，也包含不少 API/渲染依赖变化。不能把 0.14 的功能广度等同于稳定兼容承诺。[README disclaimer](https://github.com/iced-rs/iced/blob/0.14.0/README.md)；[crate disclaimer](https://github.com/iced-rs/iced/blob/0.14.0/src/lib.rs)
- 真实项目 Halloy 在 2026.8 发布了 15.6 MB 的 Windows `halloy-installer.exe`，证明 Iced 血缘的复杂桌面应用能够在 Windows 持续交付；但该 release 声明依赖 0.15.0-dev，并用 Halloy 自有 Iced fork 的固定 commit 覆盖 crates.io，因此它不能证明未修改的 Iced 0.14.0 对 LazyCat 已具生产稳定性。[Halloy 2026.8 release](https://github.com/squidowl/halloy/releases/tag/2026.8)；[Halloy 2026.8 Cargo.toml](https://github.com/squidowl/halloy/blob/2026.8/Cargo.toml)

**Elm 架构与 LazyCat 状态模型**

- Iced 的正式模型是 `State + Message + view + update`；`Task` 表示主动异步动作，`Subscription` 声明当前仍应存活的外部流。子模块可拥有自己的状态、消息、`update/view` 并映射到根消息，适合把每个工具做成明确的页面状态，而把 SQLite、长任务和跨工具事件留在 Rust 应用服务。[Iced pocket guide and scaling applications](https://github.com/iced-rs/iced/blob/0.14.0/src/lib.rs)
- 该模型有利于保持单一事实源，但不会自动解决页面生命周期。Iced 的持久 widget tree 只在新旧 widget tag 相同时执行 diff，否则整棵子树重建；子项减少时状态会截断。LazyCat 必须明确区分业务状态、页面会话状态和控件内部状态，不能假设隐藏标签页的滚动、焦点、文本选区会自动保存。[widget state tree](https://github.com/iced-rs/iced/blob/0.14.0/core/src/widget/tree.rs)
- Iced 0.14 有 `keyed`、`lazy` 和具 id 的部分控件，可减少列表重排或重绘成本，但没有一个现成的“多工具标签页缓存”组件。是否保留所有打开页面的 subtree、还是把必要状态提取到页面模型并在激活时恢复，是必须由原型决定的架构接缝。

**组件、主题和桌面 UI**

- 官方控件包括 button、checkbox、combo box、pick list、radio、slider/toggler、text input/editor、scrollable、table、grid、pane grid、tooltip、progress、responsive、stack 等，并支持 canvas/image/svg/markdown/highlighter feature。0.14 的基础集合足以做 LazyCat 的侧栏、工具标签、表单、列表、基础表格和弱代码编辑。[widget inventory](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/lib.rs)
- `Theme` 提供一组内建亮/暗 palette 和自定义 palette；每个内建控件通过 `style(theme, status)` 定义 active/hover/pressed/disabled 等状态，应用可响应系统主题变化。[theme source](https://github.com/iced-rs/iced/blob/0.14.0/core/src/theme.rs)；[styling guide](https://github.com/iced-rs/iced/blob/0.14.0/src/lib.rs)
- 这些是自绘控件和 palette，不是 Windows Fluent/原生控件。官方控件清单没有完整 tabs、tree、date/time picker、rich text、成熟 dialog/menu/command palette 体系；复杂生产力控件和专业浅色桌面视觉需要 LazyCat 自建或采用第三方组件。Halloy 证明可做出成熟密集界面，同时其自有 Iced fork 也说明不能把 showcase 当成低维护成本证据。

**中文 IME、键盘与焦点**

- 0.14.0 首次在 release 中正式列出 input method support。稳定版源码把 winit 的 `Ime::Enabled/Preedit/Commit/Disabled` 转为 Iced 事件，`TextInput` 与 `TextEditor` 均处理 preedit、selection 和 commit；窗口层启用 IME、设置光标区域并绘制 preedit overlay。这是组合输入链路的实现级证据。[winit IME conversion](https://github.com/iced-rs/iced/blob/0.14.0/winit/src/conversion.rs)；[window IME integration](https://github.com/iced-rs/iced/blob/0.14.0/winit/src/window.rs)；[TextInput IME](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/text_input.rs)；[TextEditor IME](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/text_editor.rs)
- 未找到官方针对 Windows Microsoft Pinyin、搜狗输入法、中文候选框定位或多显示器缩放的测试矩阵。0.14 才引入该能力，不能把源码存在视作中文输入验收通过。
- 框架提供按 id 聚焦、取消聚焦、查询聚焦、`focus_next/focus_previous` 和键盘事件订阅；官方 modal 示例仍显式拦截 Tab/Shift+Tab 来循环焦点。这说明能力存在，但生产级 Tab 顺序、焦点陷阱、弹窗关闭后的焦点恢复需要应用编排。[focus operations](https://github.com/iced-rs/iced/blob/0.14.0/core/src/widget/operation/focusable.rs)；[modal example](https://github.com/iced-rs/iced/blob/0.14.0/examples/modal/src/main.rs)
- 0.14.0 源码和锁文件未发现 AccessKit 或等价的公开 accessibility tree 集成。若 Narrator/辅助功能进入首版验收线，这是高风险未知项，不能用键盘可操作性代替辅助功能支持。

**DPI、多窗口和窗口控制**

- winit shell 接收 `ScaleFactorChanged`，把它发布为 `window::Event::Rescaled`，并在窗口状态中使用 OS scale factor 与应用 per-window scale factor 更新 viewport；鼠标、触摸和窗口尺寸在逻辑/物理坐标之间转换。说明 per-monitor DPI 基础链路存在，不等于混合 DPI 已通过。[DPI event conversion](https://github.com/iced-rs/iced/blob/0.14.0/winit/src/conversion.rs)；[window state scaling](https://github.com/iced-rs/iced/blob/0.14.0/winit/src/window/state.rs)
- `multi_window` 官方示例以 `BTreeMap<window::Id, Window>` 持有每窗状态，动态打开/关闭窗口，并为每窗选择 title、theme 和 scale factor；这与主窗口 + Spotlight 的状态所有权模型相容。[multi-window example](https://github.com/iced-rs/iced/blob/0.14.0/examples/multi_window/src/main.rs)
- `window::run` 的回调拿到实现 `HasWindowHandle`/`HasDisplayHandle` 的窗口对象，可在小型 Windows adapter 中取 HWND；框架也有 focus、level、decorations、transparent、request attention 等窗口 task。可靠前置、二次启动激活和全局热键消息处理仍不是框架保证。[window runtime API](https://github.com/iced-rs/iced/blob/0.14.0/runtime/src/window.rs)

**异步、后台任务与生命周期**

- `Task::perform/run/batch` 支持 Future、Stream 和并行任务；`Task::abortable` 返回显式 `Handle`，可手动中止或设置 drop 时中止。Iced feature 可选择内建 thread-pool、Tokio 或 smol executor。[Task implementation](https://github.com/iced-rs/iced/blob/0.14.0/runtime/src/task.rs)；[executor features](https://github.com/iced-rs/iced/blob/0.14.0/Cargo.toml)
- `Subscription` 的身份由每次 `subscription(state)` 声明；不再返回时对应 stream 会停止。LazyCat 的应用级长任务不能归属活动标签页的 subscription，否则切页可能结束观察流；应由根 `AppState/TaskRegistry` 持有任务 id、取消句柄、进度和结果，页面只订阅/投影视图。
- 0.14 的 Tokio feature 适合保留现有异步 Rust 业务，但是否复用单一 runtime、阻塞任务如何隔离、页面关闭后如何丢弃迟到 UI 消息仍需按 LazyCat 的既有任务生命周期契约实现，框架不会自动保证。

**托盘、全局快捷键和单实例**

- `iced::daemon` 明确支持无初始窗口静默运行、最后窗口关闭后继续运行，并要求显式 `iced::exit()` 才退出；这是隐藏到托盘所需的基础生命周期，但它不创建托盘。[daemon API](https://github.com/iced-rs/iced/blob/0.14.0/src/daemon.rs)
- `tray-icon` 支持 Windows，要求创建托盘的线程同时运行 Win32 event loop；对 winit 建议用 event handler + `EventLoopProxy` 唤醒循环。`global-hotkey` 同样要求 Windows 线程有 Win32 event loop。Iced 封装了自己的 winit loop，公开 API 没有直接暴露自定义 `EventLoopProxy`，因此必须验证事件桥接、线程归属、隐藏/恢复和退出释放，不能只验证图标出现。[tray-icon README](https://github.com/tauri-apps/tray-icon/blob/dev/README.md)；[global-hotkey README](https://github.com/tauri-apps/global-hotkey/blob/dev/README.md)
- 另一条可控路径是沿用 `windows` crate，通过 Win32 `RegisterHotKey`、命名 mutex 与实例激活 IPC 建立平台适配层；微软文档确认 `RegisterHotKey` 定义系统级热键、`CreateMutexW` 可创建命名 mutex。具体消息泵如何与 Iced 共存仍是原型问题。[RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)；[CreateMutexW](https://learn.microsoft.com/en-us/windows/win32/api/synchapi/nf-synchapi-createmutexw)

**测试、离线资源和发布**

- 0.14 新增 `iced_test` 无头 `Simulator`，支持查找控件、点击、键入、按键和 snapshot；`iced_tester` 可录制/播放 `.ice` 端到端脚本，`iced_test::run` 执行真实 Program 和副作用，并支持 preset。这可覆盖状态更新、工具导航和基础视觉快照。[iced_test](https://github.com/iced-rs/iced/blob/0.14.0/test/src/lib.rs)；[iced_tester](https://github.com/iced-rs/iced/blob/0.14.0/tester/src/lib.rs)
- 无头测试不能验证 Windows 原生 IME 候选窗、托盘、全局热键、前台激活、混合 DPI 或真实 GPU；这些必须保留为 Windows 实机原型/冒烟层。
- `Application/Daemon` builder 可从 `Cow<'static, [u8]>` 加载字体，常见做法是 `include_bytes!`；image/svg 同样可从内存数据创建，默认渲染不要求 CDN。Iced 只构建原生 exe，不提供 Windows 安装器、签名、升级或 portable zip 工具，这些仍由 LazyCat 的 Tauri 之外发布脚本负责。[daemon font loading](https://github.com/iced-rs/iced/blob/0.14.0/src/daemon.rs)；[image handle](https://github.com/iced-rs/iced/blob/0.14.0/core/src/image.rs)
- Iced 默认同时启用 wgpu 和 tiny-skia，Halloy 也同时启用两者；这提供 GPU + 软件路径，却会影响编译时间和包体。Halloy 的 15.6 MB installer 只证明一种真实交付结果，不能外推 LazyCat 的 exe、portable zip、冷启动或内存，也不能替代 MSVC/签名验证。

### 否决风险

1. **版本与维护成本**：官方仍称 experimental，稳定版发布慢而开发分支继续大幅升级；若 LazyCat 需要追 master、依赖 fork 或频繁适配 Iced/winit/wgpu 才能修 Windows 问题，不符合长期可维护性优先。
2. **标签页状态连续性**：若工具页离开 widget tree 后无法以可维护方式保存/恢复滚动、焦点、选区和复合控件状态，或只能把所有页面永久渲染以保状态，Iced 不满足 LazyCat 核心工作台语义。
3. **Windows shell 集成**：托盘和热键依赖 Win32 事件循环；若无法把事件桥接封装成小型 adapter，出现重复注册、退出悬挂、托盘残留或 Spotlight 无法可靠激活，应淘汰。
4. **IME 与输入成熟度**：IME 是 0.14 新能力且无中文 Windows 测试矩阵；丢字、重复提交、候选窗漂移、切窗后 composition/focus 异常均是阻断问题。
5. **桌面 UI 与组件建设量**：基础控件齐全但非 Fluent，tabs/tree/dialog/menu/date picker 等工作台控件缺口会转化为长期自建成本。若代表性工作台无法在合理自建组件边界内达到干净、密集、键盘友好的浅色 UI，应降低优先级或淘汰。
6. **辅助功能未知**：未发现公开 accessibility tree；若 Windows Narrator 成为首版硬验收项，当前证据不足，可能直接否决。

### 必须通过的原型

1. **动态工具标签页**：实现至少 10～15 个异构工具页；验证单工具单实例、打开/切换/关闭/重开，以及文本、光标/选区、局部选择、弹层和双层滚动按 LazyCat 语义保留。记录是显式页面状态、`keyed/lazy` 还是常驻 subtree；若需要脆弱的通用状态快照，淘汰。
2. **Windows IME 与键盘**：在 Windows 10/11 用 Microsoft Pinyin，条件允许再测搜狗；覆盖 `TextInput`/`TextEditor` 的预编辑、候选窗、选词、提交、撤销、复制粘贴、Tab/Shift+Tab、弹窗焦点陷阱、切页/切窗后恢复和 Spotlight 搜索框。
3. **混合 DPI 与渲染**：100%/150%/200% 显示器间拖动主窗口和 Spotlight，验证尺寸保存/恢复、文字/图标、overlay、命中区域和 IME 候选位置；分别跑 DX12/wgpu 与 tiny-skia 软件路径，检查黑屏、设备丢失和驱动失败反馈。
4. **Windows shell**：`iced::daemon` + 主窗口 + `tray-icon`/Win32 托盘 + 全局快捷键 + 单实例二次启动激活 + Spotlight；验证隐藏后运行、事件唤醒、重复注册/释放、Explorer 重启后的托盘恢复、正常退出和异常路径无悬挂。集成必须限制在独立 Windows adapter。
5. **异步生命周期**：以根任务注册表运行一个可取消长任务和持续日志流；切换/关闭页面不取消应用级任务、不向销毁页面回写，重开后恢复观察；验证 UI 线程不阻塞、取消句柄释放和应用退出收口。
6. **代表性 UI 与控件成本**：实现侧栏、可关闭标签栏、密集设置表单、长列表/基础表格、modal/context menu、浅色主题和窄窗口；记录自建组件代码量、键盘路径和 disabled/hover/focus/error 状态。不要用纯控件画廊代替工作台。
7. **测试与发布**：给标签页和任务生命周期写 `iced_test`/`.ice` 用例并确认 CI 可稳定运行；构建 Windows x86-64 portable zip 与安装包，嵌入全部字体/图标，执行许可证扫描，记录 clean/incremental build、exe/包体、冷启动、空闲内存、签名与离线启动。

### 最终评价

- **长期可维护性：中等潜力，高版本风险。** Elm 模型、显式消息和纯 Rust 模块边界很适合 LazyCat，但 experimental 状态、升级幅度、Halloy 依赖 fork 的现实证据显著拉低确定性。
- **桌面 UI 质量：中等。** 自绘和样式自由度高，能做成熟产品；默认控件/主题不是 Windows 桌面设计系统，高级工作台组件需要自建。
- **迁移速度：中低。** Rust 业务可直接复用，基础页面清晰；动态标签页状态、系统 shell 和组件建设会成为主要工期。
- **性能与交付：路径可行，LazyCat 指标未知。** wgpu + tiny-skia、多窗口、真实 Windows installer 都有证据，但需以 LazyCat 原型验证包体、启动、内存和驱动兼容。

**建议：保留 Iced 为一级候选，但把“无需 fork 的 Windows shell + 中文 IME”和“动态标签页状态连续性”并列为第一批否决门槛；在两项通过前，不进入正式迁移设计。**

### 一手资料

- [Iced 0.14.0 release](https://github.com/iced-rs/iced/releases/tag/0.14.0)
- [Iced 0.14.0 MIT license](https://github.com/iced-rs/iced/blob/0.14.0/LICENSE)
- [Iced 0.14.0 README and experimental disclaimer](https://github.com/iced-rs/iced/blob/0.14.0/README.md)
- [Iced 0.14.0 crate features, versions and renderers](https://github.com/iced-rs/iced/blob/0.14.0/Cargo.toml)
- [Iced master development version](https://github.com/iced-rs/iced/blob/master/Cargo.toml)
- [Elm architecture, styling, tasks, subscriptions and scaling applications](https://github.com/iced-rs/iced/blob/0.14.0/src/lib.rs)
- [Built-in widget inventory](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/lib.rs)
- [Persistent widget state tree reconciliation](https://github.com/iced-rs/iced/blob/0.14.0/core/src/widget/tree.rs)
- [IME event conversion](https://github.com/iced-rs/iced/blob/0.14.0/winit/src/conversion.rs)
- [IME window/candidate integration](https://github.com/iced-rs/iced/blob/0.14.0/winit/src/window.rs)
- [TextInput IME and focus implementation](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/text_input.rs)
- [TextEditor IME implementation](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/text_editor.rs)
- [Focus operations](https://github.com/iced-rs/iced/blob/0.14.0/core/src/widget/operation/focusable.rs)
- [DPI/window state](https://github.com/iced-rs/iced/blob/0.14.0/winit/src/window/state.rs)
- [Multi-window example](https://github.com/iced-rs/iced/blob/0.14.0/examples/multi_window/src/main.rs)
- [Daemon lifecycle](https://github.com/iced-rs/iced/blob/0.14.0/src/daemon.rs)
- [Runtime window/raw handle API](https://github.com/iced-rs/iced/blob/0.14.0/runtime/src/window.rs)
- [Task and cancellation implementation](https://github.com/iced-rs/iced/blob/0.14.0/runtime/src/task.rs)
- [Headless simulator and `.ice` test runner](https://github.com/iced-rs/iced/blob/0.14.0/test/src/lib.rs)
- [End-to-end test recorder/player](https://github.com/iced-rs/iced/blob/0.14.0/tester/src/lib.rs)
- [tray-icon Windows/winit integration requirements](https://github.com/tauri-apps/tray-icon/blob/dev/README.md)
- [global-hotkey Windows event-loop requirements](https://github.com/tauri-apps/global-hotkey/blob/dev/README.md)
- [Halloy 2026.8 Windows release](https://github.com/squidowl/halloy/releases/tag/2026.8)
- [Halloy 2026.8 Iced fork dependency](https://github.com/squidowl/halloy/blob/2026.8/Cargo.toml)
