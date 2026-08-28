# 原生 Rust GUI 重写选型与迁移路线

Label: wayfinder:map

## Destination

形成一项有证据支撑、可交付实施的决策：为 LazyCat 选定替代 Tauri 2 + Vue 3 的原生 Rust GUI 框架，并明确首版能力边界、架构接缝、验证原型、分阶段迁移与回退方案。

## Notes

- 领域：Windows 优先的离线桌面开发者工具箱；每次会话读取 `CONTEXT.md`、`docs/experience/architecture.md` 和 `docs/experience/ui-and-styling.md`。
- 使用 `wayfinder`、`grilling`、`domain-modeling`；涉及模块边界时使用 `codebase-design`。
- “原生 Rust GUI”指无 Tauri、无 WebView、无 JavaScript 运行时，允许框架自绘控件。
- Windows 10/11 是首版唯一承诺平台；避免无必要的 Win32 强绑定，不承诺 macOS/Linux。
- 候选框架和核心依赖必须允许闭源/商业发行，不购买商业许可证，不承担 GPL 开源义务。
- 选择优先级：长期可维护性 > 桌面 UI 质量 > 迁移速度 > 极限性能。
- 一级候选为 GPUI、Slint、Iced；egui 是即时模式对照，Floem 是探索候选，`windows-rs/WinUI` 是平台原生参照。
- 保留现有 Rust 业务模块、SQLite 数据格式和用户数据目录；Tauri IPC 改为进程内应用服务接口。
- 富文本与代码编辑只要求弱支持；甘特图和复杂表格不要求兼容。
- 首版保留主窗口、托盘、单实例、全局快捷键和 Spotlight；其他辅助窗口后续迁移。
- 新原生 GUI 应用在仓库内并行开发，达到首版验收线后切换默认入口；旧版保留一个发布周期作为回退。开发期间不得让新旧应用同时写同一用户数据库。
- 本地图默认只产出决策、证据和迁移路线，不实施正式重写。

## Decisions so far

- [评估 GPUI 对 LazyCat 的适配度](issues/01-evaluate-gpui.md) — 保留一级候选，但进入终选前必须通过 Windows shell 原型、IME/混合 DPI 验证和锁定版本许可证审计。
- [评估 Slint 对 LazyCat 的适配度](issues/02-evaluate-slint.md) — 保留一级候选，但动态工具标签页必须能稳定封装，且免费闭源发行需接受 Slint 署名义务。
- [评估 Iced 对 LazyCat 的适配度](issues/03-evaluate-iced.md) — 保留一级候选，但动态标签页状态连续性及无需维护 fork 的 Windows shell 与中文 IME 集成必须通过原型。
- [校验次级候选与 Windows 原生参照](issues/04-compare-secondary-options.md) — 不扩大候选集：egui 作为成熟 fallback，Floem 保留观察，`windows-rs`/WinUI 仅作平台与原生行为参照。
- [锁定框架否决门槛与评分方法](issues/05-lock-evaluation-gates.md) — 四个评价候选统一过硬门槛并按 40/30/20/10 加权；首轮只为 E2 得分最高的 Slint 建立完整原型。
- [用 Slint 最小原型验证原生 GUI 高风险路径](issues/06-prototype-hardest-paths.md) — Slint 1.17.1 的动态容器在状态连续性、焦点/可访问性节点和稳定嵌入接口上命中硬门槛，停止该候选。

## Not yet specified

- 胜出框架确定后，业务模块与 GUI 运行时之间的应用服务、异步任务、事件和状态所有权边界。
- 递补原型候选确定后，按同一硬门槛建立其最小 E3 原型。
- 胜出框架确定后，旧 Tauri 应用与新原生应用的仓库布局、构建产物和发布切换机制。
- 需要由原型结果明确的键盘导航、中文输入法、可访问性、高 DPI、多显示器和窄窗口验收细节。
- 旧数据复用、单写者约束、失败回退和一个发布周期并存的具体运行规则。

## Out of scope

- 本地图内不实施正式 GUI 重写，也不改变现有产品 UI。
- 首版不恢复甘特图和现有复杂表格能力。
- 首版不迁移快速采集、番茄钟提示、全局通知和桌面挂件等辅助窗口。
- 不承诺 macOS 或 Linux 发布。
- 不追求与现有 Vue/Element Plus 界面像素级一致。
