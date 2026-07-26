# 置顶参考卡 Esc 关闭设计

日期：2026-07-26
状态：设计已确认

## 背景

置顶参考卡当前只能通过标题栏关闭按钮关闭。卡片创建后会自动聚焦 Monaco 编辑器，因此用户在查看或编辑内容时，需要移动鼠标才能关闭当前卡片。

本次增加键盘关闭能力：当前参考卡窗口获得焦点时，按一次 `Esc` 立即关闭该窗口。

## 交互规则

- 只有当前获得系统焦点的参考卡 WebviewWindow 能接收到按键，因此 `Esc` 只关闭当前卡片，不影响其他参考卡或 LazyCat 主窗口。
- 即使 Monaco 查找框、自动补全列表或正文编辑器正在接收键盘输入，按一次 `Esc` 仍直接关闭当前参考卡。
- `Esc` 被参考卡处理后不再继续传给 Monaco。
- 关闭行为复用现有 `closeCard()`，与标题栏关闭按钮保持同一错误处理链路。
- 关闭失败时保留窗口，并继续在卡片内显示现有“关闭失败”错误信息。

## 实现方案

在 `ReferenceCard.vue` 挂载时，为当前浏览器 `window` 注册捕获阶段的 `keydown` 监听。捕获阶段可以在 Monaco 消费事件前识别 `Escape`：

1. 非 `Escape` 按键直接返回。
2. 对 `Escape` 调用 `preventDefault()` 和 `stopPropagation()`。
3. 调用现有异步 `closeCard()` 关闭当前 Tauri 窗口。
4. 组件卸载时移除同一个捕获监听，避免监听残留。

不新增 Rust 命令、IPC、设置项、全局快捷键或持久化逻辑。

## 测试与验证

先在 `ReferenceCard.contract.test.ts` 增加失败契约，守卫以下行为：

- 使用捕获阶段注册 `keydown` 监听。
- 仅在 `event.key === "Escape"` 时关闭。
- Escape 事件阻止默认行为和继续传播。
- 复用 `closeCard()`。
- 组件卸载时移除监听。

实现后运行：

1. `pnpm --filter @lazycat/desktop test -- src/components/ReferenceCard.contract.test.ts`
2. `pnpm typecheck`
3. `pnpm --filter @lazycat/desktop build:web`
4. `git diff --check`

## 验收标准

- 聚焦任意一张参考卡后，按一次 `Esc` 只关闭该卡片。
- Monaco 正文、查找框或自动补全列表获得焦点时行为一致。
- 非 Escape 按键不触发关闭。
- 标题栏关闭按钮和关闭失败提示行为不回归。
