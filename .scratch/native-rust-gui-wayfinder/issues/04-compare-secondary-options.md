# 校验次级候选与 Windows 原生参照

Type: research
Status: resolved

## Question

egui、Floem 与 `windows-rs/WinUI` 是否存在足以推翻一级候选范围的优势？使用与一级候选相同的硬约束检查许可证、Windows 成熟度、长期可维护性、生产力工具 UI 质量、输入与可访问性、多窗口和系统集成，给出继续晋级、保留为参照或淘汰的结论，避免为边缘候选投入同等原型成本。

## Answer

### 研究基线

- 结论截至 2026-08-27，针对 LazyCat 已确定的 Windows 10/11、无 WebView/JavaScript、免费闭源商业发行、Windows shell 与动态工具标签页约束。
- 评价顺序沿用地图：长期可维护性 > 桌面 UI 质量 > 迁移速度 > 极限性能。
- 源码存在 IME、DPI 或窗口事件路径不等于通过 Windows 实机验收；本票只判断是否值得占用一级候选的共同原型名额。
- Floem 必须区分 crates.io 的 `0.2.0` 与持续演进的 `main`；main 中的能力不能自动视为稳定版承诺。

### 总结论

| 候选 | 关键事实 | 结论 |
| --- | --- | --- |
| egui/eframe 0.36.1 | 许可证通过；Windows、IME、多窗口、AccessKit 和 UI 测试基础成熟；shell 能力需外接 | 不晋级。保留为成熟即时模式 fallback、性能与测试参照 |
| Floem 0.2.0 / main | MIT；Lapce 证明 Windows 生产可行；retained/reactive 模型契合，但稳定发布滞后、未到 v1，main 依赖自有 winit fork 与 git revision | 暂不晋级，保留观察 |
| `windows-rs` + WinUI 3 | bindings 许可证通过；WinUI 是原生能力上限，但官方 WinUI 工具链面向 C#/C++/XAML | 淘汰为 GUI 框架候选，仅作 Windows 行为与平台适配参照 |

三者均没有足以抵消新增原型成本与长期维护风险的独占优势。共同原型仍只保留 GPUI、Slint、Iced，不增加第四项。

### egui：成熟可靠，但不改变候选集合

#### 已核实事实

- `egui` 0.36.1 于 2026-08-07 发布，workspace 声明 Rust 1.95、`MIT OR Apache-2.0`；此前数月持续发布 0.34、0.35、0.36，维护节奏强。[0.36.1 release](https://github.com/emilk/egui/releases/tag/0.36.1) [Cargo.toml](https://github.com/emilk/egui/blob/0.36.1/Cargo.toml)
- eframe 基于 winit 原生运行，不需要 WebView。默认 feature 包含 AccessKit、默认字体和 wgpu，也可选择 glow。[eframe Cargo.toml](https://github.com/emilk/egui/blob/0.36.1/crates/eframe/Cargo.toml)
- `egui-winit` 处理 IME Enabled/Disabled/Preedit/Commit，并设置候选区域；这是完整源码路径，不是 Windows 中文输入实机验收。[egui-winit](https://github.com/emilk/egui/blob/0.36.1/crates/egui-winit/src/lib.rs)
- eframe 明确支持额外原生 viewport/window；窗口和 DPI 事件由 winit 路径承接，主窗口加 Spotlight 不是架构空白。[eframe App API](https://github.com/emilk/egui/blob/0.36.1/crates/eframe/src/epi.rs) [viewport](https://github.com/emilk/egui/blob/0.36.1/crates/egui/src/viewport.rs)
- AccessKit 默认开启；`egui_kittest` 通过 AccessKit tree 定位、点击和输入控件并支持 snapshot，是次级候选中最明确的可访问性与无头 UI 测试基础。[egui_kittest](https://github.com/emilk/egui/blob/0.36.1/crates/egui_kittest/src/lib.rs)
- egui 官方定位是即时模式 GUI。动态标签页的稳定 id、输入和滚动状态、后台任务所有权仍需 LazyCat 自行建模。[README](https://github.com/emilk/egui/blob/0.36.1/README.md)
- 托盘、全局快捷键和单实例不是 eframe 内建能力，仍需外部 crate 或 Windows API，并验证多窗口生命周期与前台激活。

#### 判断

- 长期维护性中上：活跃、许可证清楚、测试成熟，但 MSRV/图形栈升级快，即时模式不会替代复杂状态所有权设计。
- 桌面 UI 质量中等：适合调试器、可视化和高频重绘工具；要达到 LazyCat 的表单、菜单、弹窗和密集生产力布局，需要长期维护主题与组合控件。
- 输入与可访问性证据强于 Floem，但微软拼音、候选框、焦点、混合 DPI 和屏幕阅读器仍须实机验证。

**结论：不进入共同原型。** 成熟度、AccessKit 和测试能力不足以抵消即时模式状态成本及非 Windows 桌面观感。若三个一级候选均失败，egui 是第一 fallback。

### Floem：方向契合，稳定性门槛未过

#### 已核实事实

- Floem 为 MIT。正式 release 仅 2024-01 的 `v0.1.1` 与 2024-11 的 `v0.2.0`；截至研究日稳定版仍为 0.2.0。[releases](https://github.com/lapce/floem/releases) [v0.2.0](https://github.com/lapce/floem/releases/tag/v0.2.0)
- README 明示项目仍在成熟、走向 v1 期间会有 breaking changes；当前 main workspace 仍标 0.2.0、Rust 1.91。[README](https://github.com/lapce/floem/blob/main/README.md) [Cargo.toml](https://github.com/lapce/floem/blob/main/Cargo.toml)
- Floem 构建一次 retained view tree，以细粒度 reactive signals 更新，使用 Taffy Flex/Grid。main 已有 tab、dropdown、text input/editor、virtual list、rich text、主题、动画和多渲染器，形态契合 LazyCat。[README](https://github.com/lapce/floem/blob/main/README.md) [views](https://github.com/lapce/floem/tree/main/src/views)
- main 的文本输入与编辑器处理 IME enable/disable、preedit、commit、delete-surrounding 和候选区域；也有多窗口示例。[text_input](https://github.com/lapce/floem/blob/main/src/views/text_input.rs) [examples](https://github.com/lapce/floem/tree/main/examples)
- main 已有键盘导航、布局、滚动、overlay、窗口关闭等测试，但不能据此推断 2024 年发布版有同等覆盖。[tests](https://github.com/lapce/floem/tree/main/test)
- Lapce 是真实复杂产品案例，并持续提供 Windows 包；这证明 Floem 能承载生产力应用，但最强证据来自同组织、强耦合的旗舰应用。[Floem v0.2.0](https://github.com/lapce/floem/releases/tag/v0.2.0) [Lapce releases](https://github.com/lapce/lapce/releases)
- main 固定使用 Lapce 的 `floem-winit` git revision，并直接依赖多个 `understory` git revision，增加版本、供应链和升级协调成本。[Cargo.toml](https://github.com/lapce/floem/blob/main/Cargo.toml)
- 检索当前 main 未发现 AccessKit、Windows UI Automation provider 或公开 accessibility tree。这里只能表述“未找到公开实现”，不能断言绝对不存在。
- 托盘、全局快捷键、单实例仍需外部 crate 或 Windows 平台层。

#### 判断

- retained/reactive 模型和生产力组件方向很适合动态工具标签页，Lapce 也证明其 UI 上限。
- 但“未到 v1 + 稳定版长期未更新 + 自有 winit fork + 多个 git 依赖”直接冲突于长期可维护性第一的优先级。
- IME 证据比候选印象成熟；可访问性树证据缺失则是进入共同原型前的硬缺口。

**结论：暂不晋级，保留观察。** 复评条件是新的稳定 release、官方 accessibility 路径、关键 git/fork 依赖收敛，以及与发布版对应的 Windows/IME 测试证据。

### `windows-rs` / WinUI 3：原生参照，不是 Rust GUI 框架

#### 已核实事实

- Microsoft 将 `windows` crate 定位为 Rust 的 Windows API bindings，覆盖 Win32、COM 和 WinRT；它不提供 retained tree、状态模型、组件封装或应用架构。[Rust for Windows](https://learn.microsoft.com/windows/dev-environment/rust/rust-for-windows) [`windows-rs`](https://github.com/microsoft/windows-rs)
- `windows-rs` 同时提供 MIT 和 Apache-2.0 许可证，商业闭源使用本身不阻塞。[MIT](https://github.com/microsoft/windows-rs/blob/master/license-mit) [Apache-2.0](https://github.com/microsoft/windows-rs/blob/master/license-apache-2.0)
- Microsoft 的平台对比表把 WinUI 语言列为 C#、C++；Windows 应用入口把 Windows App SDK + WinUI 3 描述为使用 C#、C++、XAML 的推荐原生平台，未把 Rust 列为对等项目语言。[platform overview](https://learn.microsoft.com/windows/apps/get-started/) [Windows apps](https://learn.microsoft.com/windows/apps/)
- `windows-rs` 官方 samples 有 API、COM、composition、canvas 等用法，但截至研究日未找到与官方 C#/C++ 模板对等的完整 Rust WinUI 3/XAML 应用样例。这是公开仓库检索结果，不证明 Rust 绝对无法调用 WinRT/WinUI。[samples](https://github.com/microsoft/windows-rs/tree/master/crates/samples)
- Windows App SDK 的 unpackaged/framework-dependent 部署要求初始化 SDK runtime；Rust 组合还要自行承担 XAML 编译产物、COM apartment、消息循环、生命周期和打包接缝。[deployment](https://learn.microsoft.com/windows/apps/windows-app-sdk/deploy-unpackaged-apps)
- WinUI 原生控件仍是视觉、键盘、IME、高 DPI 和 UI Automation 的合理上限参照；托盘和经典全局热键依然属于 Win32/shell 范畴。

#### 判断

- `windows-rs` 作为绑定库维护性高，不等于 Rust WinUI 应用栈维护性高。LazyCat 会自行拥有最难的 XAML/WinRT 与构建部署接缝。
- 受支持的 C#/C++ WinUI 路径有最高原生 UI 上限，但 Rust 主导约束下需要大量非官方集成才能兑现。
- 这会把“选择 GUI 框架”变成“自行建设 Rust WinUI 框架与工具链”，超过单人离线工具箱的维护边界。

**结论：淘汰为 GUI 框架候选。** `windows-rs` 仅作为候选框架缺失 shell 能力时的平台实现手段，WinUI 仅定义 Windows 原生行为和视觉参照线。

### 对后续决策的输入

1. 共同原型候选上限保持 GPUI、Slint、Iced 三个。
2. egui 不做第四套完整原型，但可把 AccessKit tree、自动交互测试和明确 IME 事件链作为横向参照。
3. Floem 进入观察清单，不进入当前评分。
4. WinUI 和 `windows-rs` 不与 GUI 框架并列打分。
5. 下一票至少应确定这些一票否决项：许可证/署名可接受性、无需长期维护框架 fork、Windows 中文 IME 与混合 DPI、最低可访问语义、托盘/单实例/全局快捷键/Spotlight 生命周期、动态标签页状态连续性。

### 未知项与证据边界

- 未运行 egui 或 Floem 的 Windows 实机 demo；IME、候选框、混合 DPI、屏幕阅读器和前台激活均未完成行为验收。
- “未找到 Floem accessibility tree”和“未找到官方 Rust WinUI 3 完整样例”仅限定在公开官方仓库与文档。
- 未做完整依赖许可证树审计；未来若重新晋级，仍需锁定版本执行审计。
- shell 能力可由外部 crate 或 Win32 API 补齐只是可行性，不代表生命周期、失败恢复和多窗口激活已验证。
