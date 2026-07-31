# Spotlight v0.6 实施计划

> 依据设计文档：`docs/superpowers/specs/2026-05-16-spotlight-v0.6-default-actions-design.md`
> 目标：3 个默认动作优化（剪贴板首项 / `+ ` 速建 Todo / Hosts 切换反馈）

---

## 总览

| Phase   | 目标                        | 预估   | 关键依赖  |
| ------- | --------------------------- | ------ | --------- |
| Phase 0 | 类型 / 通道 / 事件契约对齐  | 0.5 天 | 无        |
| Phase 1 | 特性 1：剪贴板智能首项      | 1 天   | Phase 0   |
| Phase 2 | 特性 2：Todo 速建           | 0.5 天 | Phase 0   |
| Phase 3 | 特性 3：Hosts 切换反馈      | 0.5 天 | Phase 0   |
| Phase 4 | 自测 + typecheck + 测试用例 | 0.5 天 | Phase 1-3 |

**Phase 1 / 2 / 3 互不依赖，可串可并。Phase 0 的 payload 扩展是 Phase 1 的前置硬依赖。**

---

## Phase 0：契约对齐

### 0.1 `spotlight_pick` payload 扩展

**文件**：`apps/desktop/src-tauri/src/main.rs:978`

**改动**：

- `spotlight_pick(app, target: String)` 增加可选参数 `text: Option<String>` 和 `source: Option<String>`
- `HotkeyNavigatePayload` 结构体（搜一下当前定义位置）增加同名可选字段
- 透传到 `window.emit("hotkey-navigate", ...)`

**注意**：保持 `target` 字段不变，向下兼容；现有调用方（vault / hosts / todo / pm provider）不传新字段即可。

### 0.2 主窗口监听器扩展

**文件**：`apps/desktop/src/App.vue` 中 `hotkey-navigate` 监听处

**改动**：

- 解构出 `text` / `source`
- navigate 完成后（即工具切换 / 视图聚焦的现有逻辑跑完），如果 `text` 非空：
  ```ts
  useClipboardSuggestion().setPendingToolInput({
    toolId: target,
    text,
    source: (source as PendingToolInput["source"]) ?? "clipboard-suggestion",
  });
  ```
- 注意 `setPendingToolInput` 是模块级单例，主窗口直接调用即可

### 0.3 Hosts 反馈事件契约

**文件**：`apps/desktop/src-tauri/src/main.rs`（如需 Rust 桥接）

**判定**：先在 Spotlight 窗口直接 `emit('hosts-applied', ...)`；如果 Tauri webview 间事件不可达，再在 Rust 端加桥接命令 `notify_main(event, payload)`。**优先尝试纯前端 emit。**

### 验证

- `pnpm typecheck` 通过
- `pnpm --filter @lazycat/desktop build:web` 通过
- 手测：现有所有 Spotlight provider 默认动作仍可用（向下兼容）

---

## Phase 1：剪贴板智能首项

### 1.1 新建 suggestion provider

**新增文件**：`apps/desktop/src/spotlight/providers/suggestion.ts`

**结构**：

- 导出 `suggestionProvider: SpotlightProvider`
- `id: "suggestion"`（注意要在 `SpotlightProviderId` 联合类型新增）
- `scopeKeys: []`（不暴露前缀）
- `weight: 100`（远高于其他 provider，但实际不参与评分流，仅占位）
- `prefetch()`：返回空数组（建议项由 SpotlightPanel 渲染层在 results 计算时直接构造）
- `defaultAction(item, ctx)`：调用 `invoke("spotlight_pick", { target: toolId, text, source: "clipboard-suggestion" })`，返回 `{ closeSpotlight: true }`

> **判断**：由于建议项需要根据剪贴板状态动态生成，且只显示一条，把它从"prefetch + scoring"流程中拿出来更简单。Provider 仅承担 `defaultAction` 路由职责。

### 1.2 类型扩展

**文件**：`apps/desktop/src/spotlight/types.ts:3`

**改动**：`SpotlightProviderId` 增加 `"suggestion"`

### 1.3 SpotlightPanel 集成

**文件**：`apps/desktop/src/components/SpotlightPanel.vue`

**新增剪贴板状态**：

```ts
import { detectClipboardContent } from "../utils/clipboard-detect";
import { isRealToolId, getToolById } from "../composables/toolCatalog"; // 视实际导出补全

const clipboardSuggestion = ref<{
  toolId: string;
  toolName: string;
  text: string;
  preview: string;
} | null>(null);

async function refreshClipboardSuggestion() {
  if (getSetting("clipboard_detection") === "false") {
    clipboardSuggestion.value = null;
    return;
  }
  try {
    const text = await navigator.clipboard.readText();
    if (!text) {
      clipboardSuggestion.value = null;
      return;
    }
    const detected = detectClipboardContent(text);
    const toolAction = detected?.actions.find((a) => a.kind === "tool");
    if (!toolAction || !isRealToolId(toolAction.toolId)) {
      clipboardSuggestion.value = null;
      return;
    }
    clipboardSuggestion.value = {
      toolId: toolAction.toolId,
      toolName: toolAction.toolName,
      text,
      preview: text.replace(/\n/g, " ").slice(0, 32) + (text.length > 32 ? "…" : ""),
    };
  } catch {
    clipboardSuggestion.value = null;
  }
}
```

**调用时机**：

- `onMounted` 中 `prefetchAll()` 之后调用一次
- `spotlight-reset` 监听器中再调用一次

**results 计算改造**：

```ts
const results = computed(() => {
  // 速建模式（Phase 2）优先
  // ...
  const text = parsed.value.query;
  const baseResults = !text.trim()
    ? (itemsByProvider.value.get("tool") ?? [])
        .slice(0, RESULT_LIMIT)
        .map((item) => ({ item, score: 0 }))
    : searchItems(text, itemsByProvider.value, { scope: scope.value, limit: RESULT_LIMIT });

  // 仅空查询且建议项有效时前置插入
  if (!text.trim() && clipboardSuggestion.value) {
    const s = clipboardSuggestion.value;
    const suggestionItem: SpotlightItem = {
      providerId: "suggestion",
      itemId: `suggestion:${s.toolId}`,
      title: `${s.toolName}（剪贴板：${s.preview}）`,
      subtitle: "Enter 打开并预填剪贴板内容",
      badge: { short: "建议", tone: "warn" },
      searchFields: [],
      payload: { toolId: s.toolId, text: s.text },
    };
    return [{ item: suggestionItem, score: 0 }, ...baseResults].slice(0, RESULT_LIMIT);
  }
  return baseResults;
});
```

**注册 provider 导入**：

```ts
import "../spotlight/providers/suggestion";
```

**SCOPE_LABEL 不需要补充**（suggestion 不暴露 scope）。

### 1.4 commitDefault 适配

`commitDefault` 现有实现按 `item.providerId` 找 provider，suggestion provider 已注册，`defaultAction` 中读取 `item.payload.toolId` / `item.payload.text` 即可。**不需要改 commitDefault。**

### 1.5 验证

- 手测：复制 JSON / SQL / 时间戳 / Base64 / JWT，呼出 Spotlight，首位是建议项
- 手测：Enter 后主窗口对应工具打开且内容预填、自动格式化（依赖目标面板自身的 watchPendingInput 行为）
- 手测：`clipboard_detection=false` 时不显示
- 手测：剪贴板为空 / 中文段落 -> 不显示，沿用现有空查询展示

---

## Phase 2：Todo 速建

### 2.1 解析层

**文件**：`apps/desktop/src/utils/spotlight-query.ts`

**新增**：

```ts
export interface QuickCommandTodoCreate {
  kind: "todo-create";
  text: string;
}
export type QuickCommand = QuickCommandTodoCreate;

export function parseQuickCommand(raw: string): QuickCommand | null {
  if (!raw.startsWith("+ ")) return null;
  const text = raw.slice(2).trim();
  return { kind: "todo-create", text };
}
```

### 2.2 单测

**文件**：`apps/desktop/src/utils/spotlight-query.test.ts`

**新增用例**：

- `parseQuickCommand("+ 写周报")` -> `{ kind: "todo-create", text: "写周报" }`
- `parseQuickCommand("+ ")` -> `{ kind: "todo-create", text: "" }`
- `parseQuickCommand("+1")` -> `null`
- `parseQuickCommand("+xxx")` -> `null`（无空格）
- `parseQuickCommand("hello")` -> `null`

### 2.3 SpotlightPanel 集成

**文件**：`apps/desktop/src/components/SpotlightPanel.vue`

**新增 computed**：

```ts
const quickCommand = computed(() => parseQuickCommand(query.value.replace(/^\s+/, "")));
```

**results 计算**：在最前增加分支

```ts
if (quickCommand.value?.kind === "todo-create") {
  const text = quickCommand.value.text;
  const item: SpotlightItem = {
    providerId: "todo",
    itemId: text ? `todo-create:${text}` : "todo-create:empty",
    title: text ? `+ 新建任务：${text}` : "+ 新建任务...",
    subtitle: text ? "Enter 创建" : "输入要新建的任务标题",
    badge: { short: "新建", tone: "success" },
    searchFields: [],
    payload: { kind: "todo-create", text },
  };
  return [{ item, score: 0 }];
}
```

**commitDefault 分支处理**：

```ts
async function commitDefault(item: SpotlightItem) {
  if (item.payload?.kind === "todo-create") {
    const text = String(item.payload.text ?? "").trim();
    if (!text) return; // 空文本无操作
    await runWithRunner(() => createTodoDraft(text));
    return;
  }
  // 原有逻辑
  ...
}
```

**作用域 chip 隐藏**：模板中 `v-if="scope && !quickCommand"`

### 2.4 todo provider 增加 helper

**文件**：`apps/desktop/src/spotlight/providers/todo.ts`

**导出**：

```ts
export async function createTodoDraft(text: string): Promise<SpotlightExecuteResult> {
  try {
    await invokeToolByChannel("tool:todo:item-create", {
      title: text,
      status: "pending",
      list_id: null,
    });
    return {
      closeSpotlight: true,
      toast: { message: `已创建：${text}`, type: "success" },
    };
  } catch (err) {
    return { errorMessage: err instanceof Error ? err.message : String(err) };
  }
}
```

> **注意**：调用前查清 `tool:todo:item-create` 的实际参数命名（snake_case / camelCase），跟 `bridge/tauri.ts:214` 后端期望对齐。如有 `list_id` 必填校验，按 todo provider 现有 prefetch 逻辑找一个默认 list 兜底。

### 2.5 验证

- `pnpm test` 包含新增解析单测，必过
- 手测：`+ 写周报` Enter -> Todo 列表新增一条；主窗口未开仍创建成功
- 手测：`+ ` 单独时 Enter 无操作
- 手测：`+ ` 与 `t xxx` 互不干扰

---

## Phase 3：Hosts 切换反馈

### 3.1 hosts provider 触发事件

**文件**：`apps/desktop/src/spotlight/providers/hosts.ts`

**改动**：在 `defaultAction` apply 成功路径之后：

```ts
import { emit } from "@tauri-apps/api/event";
// ...
await emit("hosts-applied", { name: profileName });
```

`profileName` 从 item.title / payload 取，沿用 provider 内已有命名。

### 3.2 主窗口监听 + 降级

**文件**：`apps/desktop/src/App.vue`

**新增监听**：

```ts
import { listen } from "@tauri-apps/api/event";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import { ElMessage } from "element-plus";
import { getCurrentWindow } from "@tauri-apps/api/window";

// onMounted 中：
await listen<{ name: string }>("hosts-applied", async (event) => {
  const { name } = event.payload;
  const win = getCurrentWindow();
  const visible = await win.isVisible();
  const focused = await win.isFocused();
  if (visible && focused) {
    ElMessage.success(`已应用 Hosts 配置：${name}`);
    return;
  }
  // 降级 Notification
  if (getSetting("hosts_notification_denied") === "true") return;
  let granted = await isPermissionGranted();
  if (!granted) {
    const perm = await requestPermission();
    granted = perm === "granted";
    if (!granted) {
      setSetting("hosts_notification_denied", "true");
      return;
    }
  }
  await sendNotification({ title: "LazyCat", body: `已应用 Hosts 配置：${name}` });
});
```

> **注意**：先确认 `@tauri-apps/plugin-notification` 是否已在依赖中。如未安装，且不希望引入新依赖，**降级方案：仅在主窗口可见时弹 ElMessage，主窗口未开时静默**。这是更轻的降级路径，可能优先采用。

### 3.3 决策点（开工时确认）

- 引入 `plugin-notification` -> 完整降级链
- 不引入 -> 只走 ElMessage，主窗口未开时静默

**默认推荐：不引入新插件，走轻降级**。理由：与 CLAUDE.md "保持简单" 一致；切 Hosts 后用户大概率会去主窗口看效果。

### 3.4 验证

- 手测：Spotlight 切 Hosts -> 主窗口已开 -> 弹 ElMessage
- 手测：Spotlight 切 Hosts -> 主窗口未开 -> （引入插件方案）系统通知 / （轻降级方案）静默
- 验证 `hosts-applied` 事件不会被 Spotlight 自身收到（emit 默认全 webview 广播，主窗口监听即可；如果 Spotlight 也监听了会被噪声触发，本设计不监听）

---

## Phase 4：收尾

### 4.1 Lint / Type / Test

- `pnpm typecheck`
- `pnpm test`
- `pnpm --filter @lazycat/desktop build:web`

### 4.2 自测清单（按设计文档第「验证」节）

1. JSON 剪贴板 -> 建议项首位 -> Enter -> JSON 工具预填且自动格式化
2. SQL / 时间戳 / JWT / Base64 同理
3. `+ 写周报` 速建成功
4. `+ ` 空文本 Enter 无操作
5. Hosts 切换主窗口已开弹 ElMessage
6. 关闭 `clipboard_detection` 后建议项不出现
7. 现有所有 provider 默认动作仍可用（回归）

### 4.3 提交规范

按 CLAUDE.md `08.1`：

- `feat(spotlight): 剪贴板内容呼出后自动建议匹配工具`
- `feat(spotlight): 支持 + 前缀速建 Todo`
- `feat(spotlight): Hosts 切换后主窗口提示`

可合并为一个 `feat(spotlight): 默认动作优化 v0.6` 提交，附三条 bullet。

---

## 风险与回退

| 风险                                                  | 触发条件                                        | 回退策略                                                                  |
| ----------------------------------------------------- | ----------------------------------------------- | ------------------------------------------------------------------------- |
| Tauri webview 间 emit 不可达                          | Phase 3 的 `emit('hosts-applied')` 主窗口收不到 | 在 Rust 端加桥接命令 `notify_main(event, payload)`                        |
| `tool:todo:item-create` 字段命名/必填项与设计假设不符 | Phase 2 创建报错                                | 按报错信息调整 payload；或引用 todo provider 已有 prefetch 中的 list 兜底 |
| 建议项 `text` 含特殊字符（emoji / 多字节）显示溢出    | 极端长文本                                      | preview 已截断 32 字符；CSS 单行省略号已处理                              |
| 主窗口监听 `hotkey-navigate` 已有逻辑被 text 字段干扰 | text 在某些 navigate 路径下被误用               | 仅当 `source === 'clipboard-suggestion'` 才调 `setPendingToolInput`       |

## 下一步

按 Phase 0 -> 1 -> 2 -> 3 -> 4 顺序推进。每个 Phase 结束做一次最小验证再进入下一个。如需并行，Phase 1 / 2 / 3 之间可分支并行，最后合并到 Phase 4。
