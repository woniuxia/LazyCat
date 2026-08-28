# 评估 Slint 对 LazyCat 的适配度

Type: research
Status: resolved

## Question

基于截至当前的官方资料和真实 Windows 桌面应用证据，Slint 是否满足 LazyCat 的硬性约束与优先级？重点核实免费闭源商业发行授权、Windows 10/11、Rust 集成与声明式架构、组件与主题能力、中文输入法、键盘与焦点、高 DPI、多窗口、异步任务、托盘/全局快捷键集成、测试能力、二进制与发布打包；明确已证实事实、未知项、否决风险及必须通过原型验证的假设。

## Answer

研究基线：2026-08-27；当前稳定版为 Slint 1.17.1（2026-07-07）。

### 结论

**Slint 通过 LazyCat 的许可证和 Windows 平台硬门槛，保留一级候选，但不能在原型前直接定为胜出框架。** 它在声明式、编译期检查、Rust 类型化边界、Fluent 桌面风格和官方维护成熟度上很契合“长期可维护性优先”；真正的结构性风险不是富文本或表格，而是 LazyCat 的动态工具注册、多个已打开标签页及页面状态保留：Slint 1.17.1 用于运行时嵌入不同组件的 `ComponentContainer` 仍明确标为实验性且仅 Rust 可用。

若免费闭源发行可接受 Slint 署名义务，并且动态标签页、中文 IME、全局快捷键/单实例/Spotlight 窗口原型通过，Slint 可进入最终二选一；任一项失败，尤其是只能依赖不稳定动态组件 API 才能维持现有标签页语义时，应淘汰。

### 硬约束判定

| 约束 | 判定 | 证据与边界 |
| --- | --- | --- |
| 免费闭源商业发行，不购买许可证、不承担 GPL 义务 | **有条件通过** | Slint 1.17.1 框架三选一授权。Royalty-free 2.0 明确允许免费把 Slint 用于 proprietary desktop application，不要求 LazyCat 采用 GPL；但发行时必须二选一署名：在顶层菜单可达的 About 对话框展示 `AboutSlint`（无 About 时放 Splash），或在公开网页（最好是下载页）显著展示 Slint badge。该许可不是 MIT/Apache 类宽松许可证；若产品不能接受署名，则必须购买商业许可证，届时违反本地图硬约束。还禁止单独分发 Slint、嵌入式系统用途和让应用暴露 Slint API。正式采用前仍应由项目所有者确认非标准许可证条款。 |
| Windows 10/11 | **通过** | 1.17.1 官方测试矩阵明确列出 Windows 10 x86-64、Windows 11 x86-64/aarch64；符合 LazyCat 首版只承诺 Windows 10/11。 |
| 无 WebView / 无 JavaScript 运行时 | **通过** | Rust 桌面默认使用 winit 后端及 FemtoVG/软件渲染器，也可选 Skia；UI 由 Slint 编译器生成 Rust 代码并由原生渲染器绘制，不依赖 WebView。 |
| Rust 集成、长期可维护性 | **原则上通过** | `.slint` 可由 `build.rs`/`slint-build` 编译，生成类型化 Rust component handle、属性 getter/setter 与 callback；属性绑定自动重算。UI 和业务边界清晰，但 UI handle 类似 `Rc`、不能跨线程，需坚持“Rust 应用服务持有事实，Slint 只持有 ViewModel/视图状态”。 |

### 已证实能力

**桌面 UI 与主题**

- 标准控件覆盖 Button、CheckBox、Radio、Switch、Slider、SpinBox、ComboBox、输入框、文本编辑、List/Scroll/Table/Tab、GroupBox、日期/时间选择等基础桌面需求；LazyCat 已允许放弃复杂表格，基础集合足够启动迁移。
- 1.17.1 默认 `native` 风格在 Windows 映射到 `fluent`；Fluent、Material、Cupertino、Cosmic 均有亮/暗变体，默认会跟随系统亮暗色。`Palette` 与 `StyleMetrics` 可用于自定义组件。风格在编译期选择，不是运行时任意换整套组件实现。
- 没有 Monaco/TipTap 等成熟编辑器等价物。`TextEdit` 只适合作为基础纯文本编辑；这与本地图“代码编辑、富文本弱支持”相容，但语法高亮、富文本可编辑、复杂 tree/dock/command palette 等要自行实现或缩减。

**窗口、输入与 DPI**

- 官方 Rust API 明确支持同时 `show()` 多个 component instance 后统一进入事件循环；托盘示例也同时创建主窗口和托盘 component。
- 焦点有 `focus()`、`clear-focus()`、`forward-focus`、Tab 导航和焦点原因等显式机制；键盘事件、菜单快捷键、复制粘贴、撤销重做均有实现。
- winit 后端处理 `ScaleFactorChanged`，将物理坐标按当前 scale factor 转换；官方历史修复也覆盖窗口跨不同缩放显示器。说明框架具备 per-monitor DPI 基础，不等于 LazyCat 的混合 DPI 路径已验收。
- 1.17.1 winit 后端直接处理 `winit::event::Ime::Preedit` 和 `Ime::Commit`；`TextInput` 保存并绘制 pre-edit 文本与 selection，并向平台更新候选框位置。这是中文输入法组合输入的实现级证据，但官方没有给出 Microsoft Pinyin、搜狗等 Windows 中文 IME 的逐项兼容保证，仍必须手测。
- accessibility Cargo feature 默认启用，通过 OS accessibility API 暴露 UI 树，并有角色、动作和 TextInput 相关实现；实际 Narrator 覆盖质量未知。

**异步与后台任务**

- Slint 事件循环通常必须在主线程，所有 component 也须在同一线程创建。官方建议重任务放后台线程，再用 `invoke_from_event_loop` 回到 UI；`spawn_local` 仅驱动 UI 线程上的本地 future。
- 官方明确警告 `spawn_local` 的 executor 不是 Tokio runtime，Tokio future 不能假设会被它驱动，也不推荐直接以 `#[tokio::main]` 包住 Slint 主事件循环。LazyCat 应保留独立 Tokio 多线程 runtime/任务所有者，通过 channel + `invoke_from_event_loop` 传递进度与结果。
- 真实 Windows 项目 WSL Dashboard 0.11.0 使用 Slint 1.17.1 + winit/Skia + Tokio 多线程，证明该组合可发布；这只证明集成可行，不替代 LazyCat 长任务取消、页面关闭后不回写等生命周期原型。

**托盘与 Windows shell**

- 1.17.0 新增 `SystemTrayIcon`，1.17.1 默认 feature 已包含 `system-tray`。官方示例包含图标、tooltip、可见性、点击、菜单、隐藏到托盘，并注明最后窗口关闭后可由可见托盘继续维持事件循环。
- Slint 未发现内建的全局快捷键或单实例公共 API。`MenuItem.shortcut` 是应用菜单快捷键，不能当作系统级 hotkey。LazyCat 需要继续用 `windows`/`windows-sys` 实现 `RegisterHotKey`、命名 mutex 与实例激活 IPC，或引入专用 crate。
- 可通过稳定的 `raw-window-handle-06` feature 获取 HWND；也可通过 `unstable-winit-030` 访问 winit window/事件钩子，但官方明确说明该 API 不受通常稳定性保证，并建议依赖锁为 `~1.17`。Windows shell 集成不应把核心架构押在该不稳定接口上。
- WSL Dashboard 的真实发布仍另用 `tray-icon` 和 `windows` crate，而非只依赖 Slint 托盘；它说明 Windows 原生扩展可并存，也提示 Slint 新托盘能力尚缺少足够长期生产证据。

**测试与发布**

- Slint 1.17.1 有默认关闭的 `system-testing` 和 `mcp` 开发/测试 feature，可远程检查和操控运行中 UI；内部也有 testing backend、事件驱动测试和截图测试。对应用方而言，这套 UI 测试链比普通 Rust 单测更专用，需先验证 CI 稳定性，业务和应用服务仍应保持为纯 Rust 定向测试。
- `slint-build` 支持将图片和字体以压缩原文件嵌入二进制，也支持面向软件渲染器的预处理嵌入，满足离线资源约束。Windows 安装器、签名、升级和 portable zip 不是 Slint 替应用完成的能力，仍由 LazyCat 发布脚本负责。
- WSL Dashboard 0.11.0 在 2026-08-25 实际发布 Windows portable zip（约 13.2 MB）和 setup exe（约 11.8 MB），README 声明可单文件 portable 运行。该数据只证明真实 Slint Windows 应用可按两种形式交付，不能外推 LazyCat 的最终体积、冷启动或内存。

### 关键结构风险

1. **动态标签页是最大风险。** LazyCat 要求同一工具单实例标签页、切换时保留输入/滚动/局部上下文、关闭才销毁。Slint 1.17.1 的 `ComponentContainer` 可以从 Rust 在运行时提供 `ComponentFactory` 并嵌入不同 component，但官方明确标记为 experimental、subject to change、currently only available from Rust。若不用它，只靠静态条件组件，可能导致所有工具预实例化、根组件持续膨胀，或每增一工具都修改中央分派；两者都会削弱长期可维护性。
2. **多个顶层 component 不共享各自的 Slint global。** 官方托盘示例明确说明主窗口与托盘各有自己的 `SharedGlobals`，需要宿主语言显式同步。LazyCat 必须以 Rust `AppState/ApplicationService` 为唯一事实源，不能把跨窗口状态放进 Slint global 后期待天然共享。
3. **窗口激活链仍需 Win32。** Spotlight 要从全局快捷键唤起、跨进程激活并可靠前置；Slint 的多窗口与 focus API不能绕过 Windows 前台激活限制。当前 LazyCat 已有 `windows` 依赖与强制前台逻辑，可以迁移，但必须作为明确的平台适配层而非 UI 框架隐式职责。
4. **组件生态够用但不宽。** 官方只列出少量第三方 component set；复杂生产力工具的树、分栏、虚拟化、command palette、密集表单细节大概率需自建。Slint 自带 StandardTableView 不应被误认为能覆盖现有复杂表格。
5. **许可证有产品展示义务。** 技术上通过免费闭源门槛，但如果未来不接受公开 Slint badge/AboutSlint，则候选立即不满足“不购买商业许可证”。

### 未知项与必须通过的原型

进入终选前需完成一个 Windows shell + 代表性工作台原型，并把以下项目作为通过/淘汰门槛：

1. **动态标签页原型**：至少 10～15 个不同工具 component，验证打开、切换、关闭、再次打开；输入、局部选择与双层滚动位置按 LazyCat 语义保留。分别评估实验性 `ComponentContainer` 和可接受的稳定替代方案；若只能依赖易变 API且无法封装在一个小边界内，淘汰 Slint。
2. **IME 与键盘原型**：Windows 10/11 上用 Microsoft Pinyin，条件允许再测搜狗；覆盖 LineEdit/TextEdit 的预编辑、候选窗位置、选词、提交、撤销、复制粘贴、Tab/Shift+Tab、组合键和 Spotlight 搜索框。出现丢字、重复提交、候选窗漂移或焦点无法恢复即阻断。
3. **混合 DPI 原型**：100%/150%/200% 显示器之间拖动主窗口和 Spotlight，检查文字、图标、弹层、命中区域、保存/恢复窗口尺寸及无渲染空白。
4. **Windows shell 原型**：主窗口 + Slint `SystemTrayIcon` + 全局快捷键 + 单实例二次启动激活 + 无边框 Spotlight；验证隐藏到托盘后事件循环存活、快捷键重复注册/释放、退出清理和前台激活。优先把 Win32 消息处理隔离在独立 adapter，避免依赖 `unstable-winit-030`；做不到则记录版本锁与升级成本。
5. **异步生命周期原型**：独立 Tokio runtime 跑一个可取消长任务和持续日志流；切换/关闭页面不取消应用级任务，不向销毁页面回写，重开后可恢复观察；UI 线程无阻塞。
6. **视觉与可访问性原型**：用 Fluent light 实现主侧栏、标签栏、密集表单、列表、弹窗和窄窗口；测 Windows Narrator 基本名称/角色/焦点顺序。仅能做出嵌入式大按钮风格或键盘路径断裂则淘汰。
7. **发布原型**：构建 x86-64 portable zip 和安装包，全部字体/图标离线嵌入；记录 clean build 时间、增量 build、exe/包体积、冷启动、空闲内存、GPU/软件回退，并验证 AboutSlint 或下载页 badge 的合规方案。

### 最终评价

- **长期可维护性：高潜力，但受动态组件风险约束。** 声明式 UI、编译期类型和小而清晰的 Rust callback 边界优于即时模式堆叠状态；动态工具工作台若无法稳定封装则反转为主要劣势。
- **桌面 UI 质量：中高。** Fluent light、系统亮暗跟随、布局/动画/主题 token 足够做专业桌面工具，但高级生产力控件需自建。
- **迁移速度：中。** 表单和基础列表快，动态标签页、Spotlight、系统集成和编辑器会拖慢。
- **性能与交付：有可行证据，LazyCat 数据未知。** 官方支持 GPU/软件多渲染器，已有真实 Windows app/安装包；最终指标必须以 LazyCat 原型实测。

**建议：继续保留 Slint 为一级候选，并把“动态标签页可稳定封装”设为第一否决门槛；其优先级高于再做普通控件画廊。**

### 一手资料

- [Slint 1.17.1 release](https://github.com/slint-ui/slint/releases/tag/v1.17.1)
- [Slint 1.17.1 framework license choices](https://github.com/slint-ui/slint/blob/v1.17.1/LICENSE.md)
- [Royalty-free Desktop, Mobile, and Web Applications License 2.0](https://github.com/slint-ui/slint/blob/v1.17.1/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md)
- [Windows tested platform matrix](https://github.com/slint-ui/slint/blob/v1.17.1/docs/astro/src/content/docs/guide/platforms/desktop.mdx)
- [Standard widgets overview and styles](https://github.com/slint-ui/slint/blob/v1.17.1/docs/astro/src/content/docs/reference/std-widgets/overview.mdx)
- [Widget style selection](https://github.com/slint-ui/slint/blob/v1.17.1/docs/astro/src/content/docs/reference/std-widgets/style.mdx)
- [Rust API, generated components, threading and event loop](https://github.com/slint-ui/slint/blob/v1.17.1/api/rs/slint/lib.rs)
- [Winit IME, keyboard and scale-factor event handling](https://github.com/slint-ui/slint/blob/v1.17.1/internal/backends/winit/event_loop.rs)
- [TextInput pre-edit, selection and input-method implementation](https://github.com/slint-ui/slint/blob/v1.17.1/internal/core/items/text.rs)
- [Experimental ComponentContainer](https://github.com/slint-ui/slint/blob/v1.17.1/docs/astro/src/content/docs/guide/experimental/component-container.mdx)
- [System tray Rust example](https://github.com/slint-ui/slint/blob/v1.17.1/examples/system-tray/main.rs)
- [SystemTrayIcon definition and example UI](https://github.com/slint-ui/slint/blob/v1.17.1/examples/system-tray/system-tray.slint)
- [Slint Cargo features: accessibility, system tray, system testing, backends/renderers](https://github.com/slint-ui/slint/blob/v1.17.1/api/rs/slint/Cargo.toml)
- [Resource embedding API](https://github.com/slint-ui/slint/blob/v1.17.1/api/rs/build/lib.rs)
- [WSL Dashboard owner README: Windows 10/11, Slint/Skia/Tokio, tray and packaging](https://github.com/owu/wsl-dashboard/blob/main/README.md)
- [WSL Dashboard dependency manifest](https://github.com/owu/wsl-dashboard/blob/main/Cargo.toml)
- [WSL Dashboard 0.11.0 Windows release artifacts](https://github.com/owu/wsl-dashboard/releases/tag/v0.11.0)
