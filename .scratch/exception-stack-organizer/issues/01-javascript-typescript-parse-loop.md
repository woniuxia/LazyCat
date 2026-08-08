# 01 — JavaScript/TypeScript 异常堆栈解析闭环

**What to build:** 用户可以从统一工具入口打开独立的异常堆栈整理器，粘贴常见浏览器或 Node/V8 的 JavaScript/TypeScript 堆栈，并通过一次明确的解析操作得到结构化异常信息、调用帧和规范化摘要。

**Blocked by:** None — can start immediately.

**Status:** ready-for-agent

- [ ] 异常堆栈整理器出现在统一工具入口并能打开独立面板，不改变既有工具 ID 或默认导航行为。
- [ ] 只有点击解析或按下 `Ctrl+Enter` 才产生解析结果；编辑或粘贴原文不会自动刷新派生结果，原始输入始终保持可见且不被改写。
- [ ] 常见浏览器和 Node/V8 JavaScript/TypeScript 堆栈可提取异常类型、消息、调用名称、路径、行号和可选列号，并展示规范化排查摘要。
- [ ] 完全没有可识别结构的输入显示明确失败，不生成空的或猜测性的摘要。
- [ ] 解析和工具入口行为有定向测试，处理过程保持本地离线且不保存堆栈内容。
