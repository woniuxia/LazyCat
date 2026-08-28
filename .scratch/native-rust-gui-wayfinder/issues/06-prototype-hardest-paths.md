# 用 Slint 最小原型验证原生 GUI 高风险路径

Type: prototype
Status: resolved

Blocked by: 05

## Question

Slint 最小原型应实现哪些共同场景，才能验证 LazyCat 的真实风险并形成 E3 证据？原型至少需要覆盖动态工具标签页及状态连续性、密集表单、可编辑长文本或基础代码编辑、长列表、Windows 中文输入法、完整键盘操作、异步任务与取消、主窗口/托盘/单实例/全局快捷键/Spotlight、混合 DPI 和窄窗口；明确稳定 API 封装边界、测量方式、人工验收点和命中硬门槛后的停止条件。

## Comments

- 2026-08-28：已认领并建立隔离原型
  [`demos/slint-native-risk-prototype`](../../../demos/slint-native-risk-prototype/README.md)。
  [动态工具标签页 E3 阶段 1 证据与 HITL 步骤](../../../demos/slint-native-risk-prototype/evidence/e3-stage-1-dynamic-tabs.md)
  已记录自动运行时部分；票据保持 claimed，等待用户完成可见状态连续性验收。
- 2026-08-28：用户首次实机启动命中 Slint `Recursion detected`，旧的“可运行”结论立即失效。已用
  `--startup-smoke` 稳定复现并定位为 Slint 1.17.1 `ListView` 嵌入实验性
  `ComponentContainer` 时的循环布局求值；改用保留 2,000 行场景的 `ScrollView`
  后，真实窗口依次切换 12 个 factory 并正常退出。该组合风险和失去虚拟列表的成本保留在
  [E3 证据](../../../demos/slint-native-risk-prototype/evidence/e3-stage-1-dynamic-tabs.md)中，仍需用户重新执行 HITL。

## Answer

Slint 1.17.1 在 Windows 实机上命中已确认的一票否决项，停止该候选。用户可见运行多次报告
`Focused ID ... is not in the node list`；保留动态组件 handle 的方案在获得焦点后切页时
20 次有 9 次触发 `properties.rs:628: Recursion detected`，把焦点先移回稳定主树仍有 8 次失败。

改为在 `ComponentFactory` 内按官方契约重建 handle 后，焦点压力 20/20 不再递归崩溃，证实
复用预创建 handle 与实验性 `ComponentContainer` 的父树/焦点生命周期不兼容；但重建方案又
无法从离场实例可靠读取草稿、选择和双滚动状态，并在跨真实帧连续重建时触发 FemtoVG
`Unable to create Texture object` panic。`ListView` 嵌入该容器还会独立触发循环布局，只能退化为
非虚拟化 `ScrollView`。

因此，Slint 无法同时满足“切换保留、关闭销毁”、完整焦点路径和可维护稳定接口三项硬门槛。
[完整 E3 失败证据](../../../demos/slint-native-risk-prototype/evidence/e3-stage-1-dynamic-tabs.md)
保留为决策资产；不再继续 Slint 的 IME、混合 DPI、Shell、异步、视觉或发布验证。
