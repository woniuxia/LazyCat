# Spotlight v0.6：默认动作优化

## 概述

不扩 provider，把 3 个日常高频默认动作做厚：

1. 剪贴板智能首项 -- 呼出 Spotlight 时，若剪贴板内容能被识别（JSON / XML / SQL / JWT / Base64 / 时间戳 / Path 等），结果列表首位插入"建议项"，Enter 直接打开对应工具并预填。
2. Todo 速建 -- 输入 `+ <文本>` 即建一条 Todo，落入默认/未分组列表，不解析时间。
3. Hosts 切换反馈 -- 切换 Profile 后明确反馈：主窗口可见走 ElMessage，否则降级 Tauri Notification。

## 目标 / 非目标

### 目标

- 把"打开工具 → 粘贴 → 点按钮"压成一次 Enter
- 让 Spotlight 在主窗口未开时也能快速记录一条 Todo
- 消除"Hosts 是否切成功"的确认成本

### 非目标 / YAGNI

- 不解析 Todo 时间 / 优先级 / 标签
- 不引入 `>` 命令模式、表达式直算、自然语言识别
- 不做"多工具候选挑选"，建议项只展示最匹配的一个
- 不增加 snippet / 正则 / 快捷键库等新 provider
- Vault 默认动作（复制密码）保持不变
- 工具预填的"显式参数语法"（`工具名 | 内容`）不实现

## 现状回顾

- Spotlight 在独立 webview 窗口，5 个 provider：tool / vault / hosts / todo / pm
- 作用域前缀 `t / v / h / p` + 空格
- `spotlight_pick(target)` -> 隐藏 Spotlight，唤起主窗口，emit `hotkey-navigate { target, ... }` 让主窗口切到对应工具
- 主窗口已有 `useClipboardSuggestion` 模块（`detectClipboardContent` + `setPendingToolInput` + `watchPendingInput`）；目标面板均已对接 `watchPendingInput`，可消费 `pendingInput.text`
- Hosts provider 默认动作执行后无任何视觉反馈

## 特性 1：剪贴板智能首项

### 输入与展示

- 触发时机：Spotlight `onMounted` 与 `spotlight-reset` 事件，后台读取 `navigator.clipboard.readText()` 并跑 `detectClipboardContent`
- 命中条件：`detectClipboardContent` 返回非 null，且 `actions[]` 至少含一个 `kind === "tool"` 的项；取第一个 tool action
- 展示规则：仅在解析后 query 为空时插入；用户开始输入即让位
  - 标题：`<工具名>（剪贴板：<preview>）`，`preview` 取 `truncatePreview(text, 32)`
  - badge：`建议`，tone=`warn`
  - 子标题：`Enter 打开并预填剪贴板内容`
  - 排在结果列表第 0 位（不与"高频工具列表"竞争权重，由 Spotlight 渲染层硬置顶）

### 执行

1. `suggestionProvider.defaultAction` 调用 `invoke('spotlight_pick', { target: toolId, text, source: 'clipboard-suggestion' })`
2. Rust 端 `spotlight_pick` 接受可选 `text` / `source`，透传到 `hotkey-navigate` payload
3. 主窗口 `App.vue` 的 `hotkey-navigate` 监听器：navigate 完成后，如 payload 含 `text`，调用 `useClipboardSuggestion().setPendingToolInput({ toolId: target, text, source })`
4. 目标面板已 `watchPendingInput`，自动消费

### 模块归属

- 新文件 `src/spotlight/providers/suggestion.ts`：导出 `suggestionProvider`，scope 不暴露前缀（`scopeKeys: []`）
- 在 `SpotlightPanel.vue` 引入：`import "../spotlight/providers/suggestion"`
- 渲染层在 `results` computed 中，**空查询路径**额外把"建议项"前置（不参与 `searchItems` 评分流，避免与工具高频列表互相影响）

### 边界

- 剪贴板为空 / 读取失败 / 解析为 unknown：不展示建议项，沿用现有"空查询展示工具高频"
- 工具 ID 在当前 sidebar 中不存在：跳过
- 建议项的 `itemId` 设为 `suggestion:<toolId>:<hash(text前64字符)>`，避免列表 key 冲突

## 特性 2：Todo 速建（独立前缀）

### 语法

- 前缀：`+ `（加号 + 空格）；空格必须存在，避免与 `+1` 之类内容冲突
- `+ <文本>`：识别为 Todo 速建指令
- `+ ` 单独：合法触发但内容为空，列表显示提示「输入要新建的任务标题」

### 解析

- 在 `src/utils/spotlight-query.ts` 新增 `parseQuickCommand(raw)`，返回 `{ kind: 'todo-create', text } | null`
- 优先级：在 `SpotlightPanel.vue` 的 `parsed` 计算中，先跑 `parseQuickCommand`；命中则忽略 `parseSpotlightQuery` 的 scope 解析

### 展示

- 命中速建模式时，`results` 只包含一条虚拟项：
  - `text` 非空：标题 `+ 新建任务：<text>`，badge=`新建` tone=`success`，子标题 `Enter 创建`
  - `text` 为空：标题 `+ 新建任务...`，子标题 `输入要新建的任务标题`，禁用 Enter（默认动作 no-op）
- 禁用作用域 chip 显示

### 执行

- 调用现有 todo 创建通道（`tool:todo:item_create` 或等价；以 `todo.ts` provider 已有调用为参照）
- 字段：`title=<text>`，`status='pending'`，`list_id=null`（默认/未分组），其余字段服务端默认
- 成功：`closeSpotlight: true`，toast 由主窗口或 Spotlight 内部显示「已创建：<title>」
- 失败：进 errorBar，可 Ctrl+R 重试

### 模块归属

- `src/spotlight/providers/todo.ts` 增加 `createDraft(text)` helper（不进 provider 接口）
- `SpotlightPanel.vue` 在 results computed 命中速建模式时调用 helper；不走标准 `provider.defaultAction` 流程，单独路径

## 特性 3：Hosts 切换反馈

### 反馈链路

1. `hostsProvider.defaultAction` 在 apply 成功路径调用 Tauri emit：`hosts-applied { name, ruleCount }`
2. 主窗口 `App.vue` 监听 `hosts-applied`：
   - 主窗口可见且已聚焦：`ElMessage.success('已应用 Hosts 配置：<name>')`
   - 否则：通过 Tauri Notification API 弹系统通知（首次会触发权限申请）
3. Notification 权限被拒后：记录到 `user_settings.hosts_notification_denied=true`，后续不再尝试，回落静默

### 内容文案

- ElMessage：`已应用 Hosts 配置：<name>`（简洁；规则数已经在主窗口可见，不冗余展示）
- Notification：标题 `LazyCat`，正文 `已应用 Hosts 配置：<name>`

### 实现位置

- `src/spotlight/providers/hosts.ts`：apply 成功后 `emit('hosts-applied', { name })`
- `src/App.vue`：新增 `listen('hosts-applied', ...)`，按上述策略选 ElMessage 或 Notification
- 不需要改 Rust 端（Tauri 事件可在 webview 间互发；如不行则在 Rust 端加桥接命令）

## 改动文件清单

| 文件                                                     | 改动类型 | 说明                                                                                   |
| -------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------- |
| `apps/desktop/src-tauri/src/main.rs`                     | 修改     | `spotlight_pick` payload 扩展可选 `text` / `source`；同步 `HotkeyNavigatePayload` 结构 |
| `apps/desktop/src/spotlight/providers/suggestion.ts`     | 新增     | 剪贴板建议 provider                                                                    |
| `apps/desktop/src/spotlight/providers/todo.ts`           | 修改     | 新增 `createDraft(text)` helper                                                        |
| `apps/desktop/src/spotlight/providers/hosts.ts`          | 修改     | apply 成功后 emit `hosts-applied`                                                      |
| `apps/desktop/src/components/SpotlightPanel.vue`         | 修改     | 引入 suggestion provider；results 计算识别 `+ ` 速建；建议项前置渲染                   |
| `apps/desktop/src/utils/spotlight-query.ts`              | 修改     | 新增 `parseQuickCommand`                                                               |
| `apps/desktop/src/utils/spotlight-query.test.ts`         | 修改     | 增加 `+ ` 前缀解析单测                                                                 |
| `apps/desktop/src/App.vue`                               | 修改     | `hotkey-navigate` 处理新 payload；新增 `hosts-applied` 监听及降级逻辑                  |
| `apps/desktop/src/composables/useClipboardSuggestion.ts` | 不改     | 复用现有 `setPendingToolInput` 通道                                                    |

## 关键风险与对策

| 风险                                                         | 对策                                                                                            |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| 剪贴板含敏感信息（密码 / token），用户不希望被预读           | 沿用现有 `clipboard_detection=false` 用户开关；建议项遵守该开关                                 |
| Spotlight 与主窗口跨 webview 传 `text`，payload 过大可能截断 | `text` 透传不做长度限制（Tauri event 可承载较大字符串）；前端上限以剪贴板可读量为界，不额外裁剪 |
| `+ ` 前缀与正常搜索冲突                                      | 必须有空格，且空格后允许任意字符；用户主动输入 `+ ` 概率极低                                    |
| Hosts Notification 在 Windows 首次弹权限影响体验             | 首次只在主窗口不可见时尝试；用户拒绝后写入设置不再尝试                                          |
| 建议项 `text` 长度过长导致 UI 溢出                           | 标题预览取前 32 字符 + 省略号；保持单行                                                         |

## 验证

### 自动化

- `apps/desktop/src/utils/spotlight-query.test.ts` 新增用例：
  - `+ 写周报` -> `{ kind: 'todo-create', text: '写周报' }`
  - `+ ` -> `{ kind: 'todo-create', text: '' }`
  - `+1` -> `null`（无空格）
  - `+ ` 与作用域前缀 `t xxx` 互斥优先级
- `pnpm typecheck` / `pnpm test` 必过
- 视改动面执行 `pnpm --filter @lazycat/desktop build:web`

### 手测路径

1. 复制一段 JSON -> Ctrl+Space 呼出 -> 首位是「JSON 格式化（{"...）」-> Enter -> JSON 工具打开且已预填、已格式化（看 watchPendingInput 消费侧行为）
2. Ctrl+Space -> 输入 `+ 写周报` -> Enter -> Todo 列表新增一条；主窗口未开时仍创建成功
3. Ctrl+Space -> 输入 `h prod` -> Enter -> Hosts 切换；主窗口已开时弹 ElMessage；主窗口关闭时弹系统通知
4. 剪贴板为 unknown 内容（如随便一段中文）：呼出后无建议项，沿用现有空查询展示
5. 用户在设置中关闭 `clipboard_detection`：建议项不出现

## 后续可演进（不在本版范围）

- 建议项支持 Tab 切换候选工具（detectClipboardContent 已能返回多 actions）
- `+ ` 速建增加可选优先级语法（如 `+ ! 任务` 表示高优）
- Spotlight 内嵌轻量计算 / 表达式
- 复用现有 snippet / 正则模板作为新 provider
