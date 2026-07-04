# 接口调试细节与交互优化实施计划

> **For Claude:** REQUIRED SUB-SKILL: 使用 superpowers:executing-plans 按任务逐个执行本计划。
> 依据设计文档：`docs/superpowers/specs/2026-07-04-api-workbench-ux-design.md`（已通过三轮评审，事实声称均经代码核验）

**Goal:** 对接口调试工具（api-workbench）落地 18 项细节与交互优化，含多标签架构。

**Architecture:** 四个 Phase 对应设计文档四个批次，前三批为增量细节（互不依赖），Phase 4 为唯一动面板主干状态的批次（多标签 + 拖拽，两者独立提交）。所有可测逻辑先落纯函数配单测，组件只做状态编排。

**Tech Stack:** Vue 3 + TypeScript + Element Plus + Monaco（已内置）+ Rust（ureq 2.12.1）。

---

## 总览

| Phase | 目标 | 关键依赖 |
|-------|------|---------|
| Phase 1 | 编辑区细节：KV 组件化/粘贴/补全、URL 拆分、最终 URL 预览、页签徽标、Body Monaco | 无 |
| Phase 2 | 响应区：format 工具上提、状态行、JSON 树+Monaco 双模式、响应头表格 | 无 |
| Phase 3 | 工作流小项：色板、复制接口、请求设置（含后端 ureq 重定向）、认证、变量补全、cURL 弹窗、历史收敛、空态 | 无 |
| Phase 4 | 多标签重构（独立提交）、树内拖拽（独立提交） | Phase 1-3 完成后 |

每个任务收尾即提交（约定式中文提交信息）；每个 Phase 结束跑验证门。

**通用命令**（在仓库根执行）：

- 单测：`pnpm test src/utils/xxx.test.ts`（可多个路径）
- 类型：`pnpm typecheck`
- 构建：`pnpm --filter @lazycat/desktop build:web`
- Rust：`cd apps/desktop/src-tauri; cargo test api_workbench -- --nocapture`

---

## Phase 0：准备

1. 通读设计文档与本计划；确认工作区干净（`git status`）。
2. 关键现状锚点（行号为 2026-07-04 快照，执行时以搜索为准）：
   - 面板 `apps/desktop/src/components/ApiWorkbenchPanel.vue`（2133 行）；运行时 KeyValueEditor 在 `:430`；4 个使用点 `:97/:100/:112/:310`；历史区 `:161-211`；`sendRequest :1350`；`loadAll :691`。
   - `apps/desktop/src/utils/apiWorkbench.ts`：`DEFAULT_API_WORKBENCH_DRAFT :31`、`normalizeRows :83`、`buildApiWorkbenchPreviewUrl :68`。
   - 后端 `apps/desktop/src-tauri/src/tools/api_workbench.rs`：建表 `:41`、`ensure_api_workbench_history_columns :196`（ALTER 兼容模式范本）、`RequestDraft` 结构 `:175` 附近、ureq 构建 `:1766`（`AgentBuilder::new().timeout(...).redirects(0)`）、`request_save :839-901`。
   - 复用组件：`components/MonacoPane.vue`（props：`modelValue/language/readOnly`）、`components/common/JsonTreeViewer.vue`（props：`value`（解析后 JSON）、`defaultExpandDepth`、`copyText`）。
   - 设置读写：`composables/useSettings.ts` 的 `getSettingJson<T>(key, fallback)` / `setSettingJson(key, value)`。

---

## Phase 1：编辑区细节

### Task 1.1 normalizeRows 过滤空行（脏比较与保存的前置）

**文件：** 修改 `apps/desktop/src/utils/apiWorkbench.ts`；测试 `apps/desktop/src/utils/apiWorkbench.test.ts`

1. 先写失败测试：`normalizeApiWorkbenchDraft` 对 `query/headers/form` 过滤「key 与 value 均为空串（trim 后）」的行；保留「仅 value 有值」「仅 key 有值」的行；`enabled:false` 但有内容的行保留。
2. 跑 `pnpm test src/utils/apiWorkbench.test.ts` 确认失败。
3. 实现：`normalizeRows` 末尾追加 `.filter((row) => row.key.trim() !== "" || row.value.trim() !== "")`。
4. 测试通过后提交：`fix(api-workbench): 归一化过滤全空 KV 行`

### Task 1.2 KV 粘贴解析纯函数

**文件：** 新增 `apps/desktop/src/utils/apiWorkbenchKvPaste.ts` + `apiWorkbenchKvPaste.test.ts`

```typescript
export interface ApiWorkbenchKvPasteResult {
  rows: ApiWorkbenchKeyValueRow[];
}
/** 返回 null 表示不满足拆分条件（按普通粘贴处理） */
export function parseApiWorkbenchKvPaste(text: string): ApiWorkbenchKvPasteResult | null;
```

规则（写成测试用例，先测后实现）：

| 输入 | 期望 |
|------|------|
| `a=1&b=2`（单行含 `&`） | 2 行，query-string 拆分，不做 URL 解码 |
| `Content-Type: application/json\nAccept: */*` | 2 行，按首个 `:` 拆，value trim 前导空格 |
| `a=1\nb=2` | 2 行，按首个 `=` 拆 |
| `plain`（单行无 `&`/`=`/`:`/换行） | `null`（不拆分） |
| 行内无分隔符（如多行中的 `oops`） | 整行作为 key，value 为空（不丢内容） |
| 空行 | 跳过 |

提交：`feat(api-workbench): KV 批量粘贴解析纯函数`

### Task 1.3 常用 Header 常量表

**文件：** 新增 `apps/desktop/src/utils/apiWorkbenchHeaders.ts`

导出 `COMMON_HEADER_NAMES: string[]`（约 20 项：`Accept`、`Accept-Encoding`、`Accept-Language`、`Authorization`、`Cache-Control`、`Content-Type`、`Cookie`、`If-Modified-Since`、`If-None-Match`、`Origin`、`Referer`、`User-Agent`、`X-Requested-With`、`X-Request-Id` 等）与 `COMMON_CONTENT_TYPES: string[]`（`application/json`、`application/x-www-form-urlencoded`、`multipart/form-data`、`text/plain`、`text/html`、`application/xml`、`application/octet-stream`）。纯常量无需单测。随 Task 1.4 一起提交。

### Task 1.4 ApiWorkbenchKeyValueEditor 组件化

**文件：** 新增 `apps/desktop/src/components/ApiWorkbenchKeyValueEditor.vue`；修改 `ApiWorkbenchPanel.vue`（删除运行时 KeyValueEditor `:430-479`，替换 4 个使用点 `:97/:100/:112/:310`）

Props / 行为：

```typescript
defineProps<{
  modelValue: ApiWorkbenchKeyValueRow[];
  variant?: "query" | "headers" | "form" | "env";  // headers 启用补全
  variableNames?: string[];                          // Phase 3 变量补全用，先声明
}>();
```

- 自动追加：渲染时若最后一行 key 或 value 非空（或列表为空），在组件内部展示层追加一个空行占位；用户在占位行输入时才 emit 追加真实行。展示层空行不写回 modelValue。
- 删除：行悬停显示 `Delete` 图标按钮（`el-button text :icon`）。
- 粘贴：Key 输入框 `@paste` 中调 `parseApiWorkbenchKvPaste`，命中则 `preventDefault`，用解析行替换当前行并 `ElMessage.success("已拆分 N 行")`。
- 补全：`variant === "headers"` 时 Key 列用 `el-autocomplete`（候选 `COMMON_HEADER_NAMES` 前缀过滤，不区分大小写）；该行 key 为 `Content-Type` 时 Value 列同样用 `el-autocomplete`（候选 `COMMON_CONTENT_TYPES`）。
- 保持既有 UI 语义：启用开关、四列网格、样式沿用面板现有 `.api-workbench-kv-*` class（样式随组件迁移为 scoped）。

替换后删除面板中 `ElSwitch` 等不再使用的导入。环境管理弹窗（`:310`）用 `variant="env"`。

验证：`pnpm typecheck`；手动核对四处页签仍可编辑。提交：`feat(api-workbench): KV 编辑器组件化（自动加行/粘贴拆分/Header 补全）`

### Task 1.5 URL 拆分纯函数 + 接线

**文件：** 修改 `apps/desktop/src/utils/apiWorkbench.ts` + 测试；修改 `ApiWorkbenchPanel.vue`（URL 输入框）

```typescript
export interface ApiWorkbenchUrlSplitResult {
  url: string;                          // 去掉 ?xx 后的部分
  rows: ApiWorkbenchKeyValueRow[];      // 拆出的参数（enabled: true）
}
/** 无 ? 或 ? 后为空时返回 null */
export function splitApiWorkbenchUrlQuery(rawUrl: string): ApiWorkbenchUrlSplitResult | null;
```

测试用例：`/api?a=1&b=2`；`https://x.com/p?a={{ID}}`（变量原样保留）；`/p?flag`（无 `=` → key=`flag`，value 空）；`/p?`（返回 null）；`/p`（null）；不做 URL 解码。

面板接线：URL `el-input` 增加 `@blur` 与 `@paste`（paste 用 `nextTick` 后取值）调用统一 handler——命中则 `draft.url = result.url`、`draft.query.push(...result.rows)`、`ElMessage.success`。

提交：`feat(api-workbench): URL 参数自动拆分进 Query 页签`

### Task 1.6 变量替换纯函数 + 最终 URL 常驻预览

**文件：** 修改 `apps/desktop/src/utils/apiWorkbenchVariables.ts` + 测试；修改 `ApiWorkbenchPanel.vue`

1. 检查 `apiWorkbenchVariables.ts` 是否已有模板替换函数；没有则新增：

```typescript
export function resolveApiWorkbenchTemplate(
  text: string,
  variables: ApiWorkbenchVariable[][],   // 优先级从高到低：[环境, 全局]
): { text: string; missing: string[] };
```

测试：命中环境变量、环境覆盖全局、缺失保留 `{{NAME}}` 并进 missing、无变量原样返回。

2. 面板：在 `.api-workbench-utility-row` 内（替换现有 `baseUrlEffectText` 文案位置）渲染常驻预览行：`computed` 先 `buildApiWorkbenchPreviewUrl(baseUrl, draft.url, draft.query)` 再 `resolveApiWorkbenchTemplate`；缺失变量段用警示色 `<span>`；整行点击 `copyText`。URL 为空时显示占位灰字。

提交：`feat(api-workbench): 最终 URL 常驻预览（变量替换/缺失警示/点击复制）`

### Task 1.7 页签计数徽标

**文件：** 修改 `apps/desktop/src/utils/apiWorkbench.ts` + 测试；`ApiWorkbenchPanel.vue`

纯函数 `countApiWorkbenchActiveRows(rows): number`（enabled 且 key 非空）+ `hasApiWorkbenchBody(draft): boolean`（bodyType 非 none 且（form 计数>0 或 body.trim() 非空））。面板 `el-tab-pane` 改用 `#label` 插槽渲染 `Query (2)` / `Headers (3)` / `Body (·)`（0 时不显示徽标）。

提交：`feat(api-workbench): 编辑页签内容计数徽标`

### Task 1.8 Body Monaco 编辑

**文件：** 修改 `ApiWorkbenchPanel.vue`（Body 页签 `:102-120`）

- `bodyType === "json" | "text"` 时以 `<MonacoPane v-model="draft.body" :language="draft.bodyType === 'json' ? 'json' : 'plaintext'" />` 替换 textarea，容器固定高度（约 280px，`min-height` 保底）。
- 工具条（仅 json 显示）：格式化 = `JSON.parse` + `JSON.stringify(v, null, 2)`，失败 `ElMessage.error` 带 `err.message`（含位置信息）且不改内容；压缩 = `JSON.stringify(JSON.parse(v))` 同样处理。逻辑入面板内小函数即可（两行 try/catch，无需抽 utils）。
- 暂不移除禁用的「跟随重定向」占位开关（Phase 3 处理）。

提交：`feat(api-workbench): Body 编辑器 Monaco 化（高亮/格式化/压缩）`

### Phase 1 验证门

```
pnpm test src/utils/apiWorkbench.test.ts src/utils/apiWorkbenchKvPaste.test.ts src/utils/apiWorkbenchVariables.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

---

## Phase 2：响应查看区

### Task 2.1 utils/format.ts（字节/耗时/相对时间）

**文件：** 新增 `apps/desktop/src/utils/format.ts` + `format.test.ts`；修改 `apps/desktop/src/utils/apiMock.ts`

```typescript
export function formatByteSize(size: number): string;      // 逻辑上提自 apiMock.formatMockFileSize
export function formatDurationMs(ms: number): string;       // <1000 → "356 ms"；≥1000 → "1.4 s"；≥60000 → "1 m 5 s"
export function formatRelativeTime(iso: string, now?: Date): string;
// 刚刚(<60s) / N 分钟前 / N 小时前 / 昨天 HH:mm / MM-DD HH:mm（跨年 YYYY-MM-DD）
```

步骤：先写测试（含 0/负值/非法输入/超大值边界；relative time 注入 `now` 保证确定性）→ 失败 → 实现 → `formatMockFileSize` 改为转调 `formatByteSize`（保留导出名）→ `pnpm test src/utils/format.test.ts src/utils/apiMock.test.ts` 全绿。

提交：`feat(utils): 新增共享格式化工具并上提字节格式化`

### Task 2.2 状态行增强

**文件：** 修改 `apps/desktop/src/utils/apiWorkbench.ts` + 测试；`ApiWorkbenchPanel.vue`（`.response-summary`）

纯函数 `getApiWorkbenchStatusTone(status: number | null, error: string | null): "success" | "warning" | "danger" | "info"`（2xx→success，3xx→warning，4xx/5xx→danger，null/error→info）。状态条 `el-tag :type` 接色阶；`durationMs`/`bodySize` 换 `formatDurationMs`/`formatByteSize`。历史行的状态展示同步换色阶（小改随本任务）。

提交：`feat(api-workbench): 响应状态行色阶与人性化单位`

### Task 2.3 JSON 树 + Monaco 双模式

**文件：** 修改 `apps/desktop/src/components/ApiWorkbenchResponseViewer.vue`

- 预览模式 `viewerKind === "json"`：`computed` 尝试 `bodyText.length <= 1_000_000 && JSON.parse(bodyText)`（try/catch 返回 `{ ok, value, reason }`）。成功 → `<JsonTreeViewer :value="parsed.value" :default-expand-depth="2" :copy-text="previewText" />`；失败/超限 → 降级 Monaco 只读原文 + 顶部 `el-alert` 说明原因（"响应体超过 1 MB，已切换原文模式" / "JSON 解析失败，已切换原文模式"）。
- 原文/源码模式：`<MonacoPane :model-value="rawText" :read-only="true" :language="rawLanguage" />`；`rawLanguage` 由 `viewerKind` 映射：json→`json`、html→`html`、xml MIME→`xml`、其余 `plaintext`（映射入 `utils/apiWorkbenchResponsePreview.ts` 纯函数 + 测试）。
- 图片/PDF/Office/二进制分支不动；`formatApiWorkbenchPreviewBody` 继续供复制用。

验证：`pnpm test src/utils/apiWorkbenchResponsePreview.test.ts`；手动发 JSON/HTML/大响应各一次。提交：`feat(api-workbench): 响应 JSON 树预览与 Monaco 原文双模式`

### Task 2.4 响应头表格化

**文件：** 修改 `ApiWorkbenchPanel.vue`（响应头页签 `:148-160`）

`pre.headers-view` 改两列网格（key 列 `--lc-font-mono`，行悬停尾部「复制值」text button 调 `copyText`）；页签 `#label` 显示 `响应头 (N)`（`response.responseHeaders.length`）；保留「复制响应头」整体按钮。空态 `el-empty`。

提交：`feat(api-workbench): 响应头表格化与单值复制`

### Phase 2 验证门

```
pnpm test src/utils/format.test.ts src/utils/apiMock.test.ts src/utils/apiWorkbench.test.ts src/utils/apiWorkbenchResponsePreview.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

---

## Phase 3：工作流小项

### Task 3.1 Method 色板

**文件：** 修改 `ApiWorkbenchPanel.vue`、`ApiWorkbenchSidebar.vue`（`:64/:105` 的 `<strong>{{ method }}</strong>`）；如动 Element Plus 变量，同步检查 `src/styles/element-overrides.css` 与 `src/styles/theme-light.css`

- 纯函数 `getApiWorkbenchMethodClass(method): string`（`method-get` 等，utils/apiWorkbench.ts，含未知方法兜底 `method-default`）+ 单测。
- 一套 class 色板（GET `#1a7f37` 绿 / POST `#bc4c00` 橙 / PUT `#0969da` 蓝 / PATCH `#1b7c83` 青 / DELETE `#cf222e` 红 / HEAD、OPTIONS 灰），定义在面板全局样式块，侧栏/历史/Method 下拉共用。
- 浅色主题核查后提交：`feat(api-workbench): Method 颜色徽标统一色板`

### Task 3.2 复制接口

**文件：** 修改 `types/api-workbench.ts`（`ApiWorkbenchNavCommand` 增 `"request:duplicate"`）、`ApiWorkbenchSidebar.vue`（右键菜单项）、`ApiWorkbenchPanel.vue`（handler）

Handler：`request-get` → `request-save`（`id: null`、`name: 原名 + " 副本"`、同 `folderId`，draft 原样）→ `loadAll()` → `loadRequest(newId)`。副本不含示例响应（后端 INSERT 天然不写 `example_response_json`）。菜单文案「复制接口」。

提交：`feat(api-workbench): 右键复制接口`

### Task 3.3 后端 follow_redirects（先后端后前端）

**文件：** 修改 `apps/desktop/src-tauri/src/tools/api_workbench.rs`

1. 建表语句（`:41`）加列 `follow_redirects INTEGER NOT NULL DEFAULT 0`；新增 `ensure_api_workbench_request_columns`（参照 `:196` 的 history 版本）做 ALTER 兼容迁移并在初始化处调用。
2. `RequestDraft` 结构体加 `#[serde(default)] follow_redirects: bool`；`request_save` 的 INSERT/UPDATE（`:839-901`）与 `request_get`/历史快照序列化带上该字段（camelCase `followRedirects`）。
3. `send` 路径（`:1766`）：`.redirects(if draft.follow_redirects { 10 } else { 0 })`；成功分支 `final_url` 取 `response.get_url().to_string()`。
4. Rust 测试（本文件 `#[cfg(test)]`）：
   - serde：无 `followRedirects` 字段的旧 JSON 反序列化为 false；
   - 迁移：对旧 schema 连续执行两次 `ensure_*` 幂等；
   - 重定向语义：std `TcpListener` 起本地 stub（新增测试辅助，无新依赖）——`302 + POST` 开启跟随 → 转 GET 到达终点、`finalUrl` 为终点；`307 + POST` 开启跟随 → 返回原始 307；`follow_redirects=false` → 返回原始 302。
5. `cargo test api_workbench -- --nocapture` 全绿。

提交：`feat(api-workbench): send 支持按请求跟随重定向（ureq 语义）`

### Task 3.4 前端请求设置（⚙ 超时/重定向）

**文件：** 修改 `types/api-workbench.ts`（`ApiWorkbenchRequestDraft` 加 `followRedirects: boolean`）、`utils/apiWorkbench.ts`（`DEFAULT_API_WORKBENCH_DRAFT :31` 与 `normalizeApiWorkbenchDraft` 白名单补字段，漏加会被静默丢弃）、`ApiWorkbenchPanel.vue`

- 请求栏保存按钮左侧加 `⚙` 按钮（`el-popover`）：超时 `el-input-number`（ms，min 1000 / max 120000 步长 1000，绑 `draft.timeoutMs`）+「跟随重定向」`el-switch`（绑 `draft.followRedirects`），下方灰字注明「301/302/303 按标准跟随；307/308 带请求体不跟随」。
- 移除 Body 工具条禁用的占位 `el-switch`（`:110`）。
- 单测：normalize 补字段的用例（默认 false、显式 true 保留）。

提交：`feat(api-workbench): 请求设置入口（超时/跟随重定向）`

### Task 3.5 认证辅助

**文件：** 修改 `utils/apiWorkbench.ts` + 测试；`ApiWorkbenchPanel.vue`（Headers 页签工具条）

```typescript
export function buildApiWorkbenchAuthHeader(
  input: { type: "bearer"; token: string } | { type: "basic"; username: string; password: string },
): string;  // "Bearer xxx" / "Basic base64(user:pass)"，Basic 先 UTF-8 编码再 base64（TextEncoder → btoa on latin1 bytes）
```

测试含非 ASCII 密码（如中文）。UI：Headers 页签上方工具条「快速认证」按钮 → popover（类型 radio + 对应输入）→ 确认后 upsert `Authorization` 行（已存在则替换 value 并置 enabled）。

提交：`feat(api-workbench): 快速认证生成 Authorization 头`

### Task 3.6 变量自动补全

**文件：** 新增 `apps/desktop/src/components/ApiWorkbenchVariablePopover.vue`；修改 `ApiWorkbenchKeyValueEditor.vue`（value 列接入，经 `variableNames` prop）、`ApiWorkbenchPanel.vue`（URL 输入框接入并传候选）

- 组件职责：给定绑定的 `<input>` 元素与候选名列表；监听输入，光标前文本尾部匹配 `/\{\{([A-Za-z0-9_.-]*)$/` 时定位显示候选（前缀过滤，键盘上下 + Enter 选择，Esc 关闭）；选中后替换为 `{{NAME}}` 并补 `}}`（若右侧已有 `}}` 则不重复）。
- 候选来源：面板 computed（当前环境变量名 + 全局变量名，环境优先去重）传入。
- 匹配/替换的字符串逻辑抽纯函数（`utils/apiWorkbench.ts`：`matchApiWorkbenchVariablePrefix` / `applyApiWorkbenchVariableCompletion`）+ 单测；浮层组件本身不写测试。

提交：`feat(api-workbench): {{ 变量自动补全（URL 与 KV value）`

### Task 3.7 cURL 导入专用弹窗

**文件：** 新增 `apps/desktop/src/components/ApiWorkbenchCurlImportDialog.vue`；修改 `ApiWorkbenchPanel.vue`（`importCurl :1237` 改为打开弹窗）

- 布局：`el-dialog`（`width: min(960px, calc(100vw - 32px))`），左 `el-input type="textarea"`（12 行）；右侧实时解析区：`watch` 输入 debounce 200ms 调 `parseApiWorkbenchCurl`（try/catch），成功显示 Method/URL/Query 数/Headers 列表/Body 摘要 + warnings（`el-alert`），失败显示错误信息。
- 确认按钮（解析成功才可用）：沿用现行为——`ElMessageBox.confirm` 覆盖当前草稿（Phase 4 切为开新临时标签，此处留 `// Phase 4: 改为 openTempTab` 注释锚点）。
- 删除旧 `ElMessageBox.prompt` 路径与 `curlPreviewText`（若仅此处使用）。

提交：`feat(api-workbench): cURL 导入专用弹窗（大输入区/实时解析预览）`

### Task 3.8 历史列表收敛 + 空状态引导

**文件：** 修改 `ApiWorkbenchPanel.vue`（历史区 `:161-211`、编辑区空态）

历史行改造：

- 单击行主体 = 载入（现状保留）；行尾三个操作：星标 icon button（实心/空心随 `pinned`）、重放 icon button（loading/disabled 逻辑沿用）、`el-dropdown`「⋯」（保存为接口、备注）。
- 摘要行时间用 `formatRelativeTime(item.createdAt)`，外层 `:title` 放绝对时间。

空状态：

- `collections.length === 0`：编辑区替换为引导卡（`el-empty` + 三步文案「新建集合 → 新建接口 → Ctrl+Enter 发送」+ 主按钮「新建集合」调 `createCollection`）。
- 有集合但未选中接口且草稿为空白：轻量提示「从左侧选择接口，或直接填写 URL 发送」+ 快捷键说明（Ctrl+Enter 发送 / Ctrl+S 保存）。判断用 computed（`selectedRequestId === null && !draft.url && requestName === ""`）。

提交：`feat(api-workbench): 历史操作收敛与空状态引导`

### Phase 3 验证门

```
pnpm test src/utils/apiWorkbench.test.ts src/utils/format.test.ts
cd apps/desktop/src-tauri; cargo test api_workbench -- --nocapture; cd ../../..
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

手动冒烟：⚙ 设置保存后重开仍在；开重定向请求一个 302 地址看 finalUrl；认证头 upsert；cURL 弹窗粘贴真实命令。

---

## Phase 4A：多标签重构（独立提交）

### Task 4.1 标签纯函数层

**文件：** 新增 `apps/desktop/src/types/api-workbench-tabs.ts`（或并入 `types/api-workbench.ts`，二选一后统一）、`apps/desktop/src/utils/apiWorkbenchTabs.ts` + `apiWorkbenchTabs.test.ts`

类型（与设计文档一致）：

```typescript
export interface ApiWorkbenchTab {
  id: number;
  kind: "request" | "temp";
  requestId: number | null;
  collectionId: number | null;   // 仅集合全删兜底为 null
  folderId: number | null;
  name: string;
  draft: ApiWorkbenchRequestDraft;
  response: ApiWorkbenchSendResult | null;
  savedSnapshot: { name: string; draft: ApiWorkbenchRequestDraft } | null;
  sourceHistoryId: number | null;
  editorTab: string;
  responseTab: string;
}
export interface ApiWorkbenchTabsPersist { version: 1; activeTabId: number | null; tabs: PersistedTab[]; }
```

纯函数（先测后实现，每个函数 3-6 个用例）：

- `isApiWorkbenchTabDirty(tab)`：request 标签 = normalize(draft)+name 与 snapshot 对比；temp 标签 = draft 非空白或名称非默认。
- `pickApiWorkbenchNeighborTabId(tabs, closingId)`：右邻 > 左邻 > null。
- `normalizeApiWorkbenchRestoredTabs(raw, ctx)`：`raw` 为 `getSettingJson` 结果，`ctx = { collectionIds: Set<number>, requestIds: Set<number> }`；version 不符 → `[]`；单标签校验失败丢弃该标签；requestId 失效 → 转 temp；collectionId 失效 → 转 temp 归属 `ctx.fallbackCollectionId ?? null` 且 folderId 置 null；截断至 20。
- `backfillApiWorkbenchTabFolderIds(tabs, collections)`：request 标签按树数据回填 folderId，接口不存在或文件夹失效 → null（temp 标签不动）。

提交：`feat(api-workbench): 多标签纯函数层（脏比较/恢复/邻接/folderId 回填）`

### Task 4.2 useApiWorkbenchTabs composable

**文件：** 新增 `apps/desktop/src/composables/useApiWorkbenchTabs.ts`

API（面板消费面）：

```typescript
const { tabs, activeTabId, activeTab, openRequestTab, openTempTab, activateTab,
        closeTab, closeOthers, closeToLeft, closeToRight, markSaved,
        persistNow, restoreFromSettings } = useApiWorkbenchTabs();
```

- 状态为模块级单例（同 `useTabs` 模式）；`openRequestTab(detail)` 已开则激活，未开则建标签（上限 20，超出 `ElMessage.warning` 并拒绝）。
- 持久化：watch 标签结构（深度足够的关键字段）debounce 500ms `setSettingJson("api-workbench:tabs", persistShape)`；`response`/`editorTab`/`responseTab` 不入持久化。
- 脏确认交互放面板层（composable 不弹窗，只暴露 `isTabDirty` 判断），保持可测性。

提交：`feat(api-workbench): 多标签状态 composable（持久化/上限/激活管理）`

### Task 4.3 ApiWorkbenchTabsBar 组件

**文件：** 新增 `apps/desktop/src/components/ApiWorkbenchTabsBar.vue`

Props：`tabs`、`activeTabId`；emits：`activate/close/close-others/close-left/close-right/new-temp`。渲染：横向滚动条（溢出 `overflow-x: auto`）、Method 徽标（复用 Task 3.1 class）、名称（temp 尾缀 `*`）、脏标记 `●`、`×`；中键 `@mousedown.middle` 关闭；右键复用 `ApiWorkbenchContextMenu.vue` 弹菜单；尾部 `＋`。样式对齐应用顶部工具标签（参考 `App.vue` 标签样式变量）。

提交：`feat(api-workbench): 请求标签条组件`

### Task 4.4 面板接线（本 Phase 核心，改动集中一次提交）

**文件：** 修改 `ApiWorkbenchPanel.vue`（主干）、`ApiWorkbenchResponseViewer` 不动

替换规则（保持函数体，换数据源）：

1. 单例 ref（`draft/requestName/requestDescription/response/selectedRequestId/selectedRequestFolderId/sourceHistoryId/editorTab/responseTab`）改为基于 `activeTab` 的 `computed`（get/set 写回标签字段）。无激活标签时这些 computed 返回只读默认值，编辑区渲染空态。
2. 入口改造：
   - 侧栏 `loadRequest` → `request-get` 后 `openRequestTab(detail)`（含 `savedSnapshot` 初始化）。
   - `startNewRequest` / `loadHistoryIntoTemporaryEditor` / cURL 确认 → `openTempTab(...)`（历史载入移除覆盖确认弹窗；cURL 弹窗内 Phase 3 注释锚点改掉）。
   - `saveRequest` 成功 → `markSaved(tabId, savedName, normalizedDraft)`；temp 转 request（补 `requestId/collectionId/folderId`）。
3. 生命周期挂钩：
   - `deleteRequest`：命中标签→脏转 temp（保留内容）/干净关闭。
   - `deleteCollection`：干净关闭、脏转 temp 归属删除后新选中集合（无集合则 `collectionId=null`）、folderId 置 null。
   - `loadAll` 末尾调 `backfillApiWorkbenchTabFolderIds(tabs, collections)`。
   - `activateTab` 时若标签 `collectionId` 与当前选中不同 → `selectCollection(标签集合)`（环境下拉随集合联动，沿用现逻辑）。
4. 发送归属：`sendRequest` 用 `activeTab.collectionId` 判空拦截（沿用「请先选择环境」路径）；响应回填以发送时快照的 `tabId` 写回，标签已关则丢弃（防旧响应写错标签）。
5. 挂载：`onMounted` 里 `loadAll` 成功后 `restoreFromSettings(ctx)`；恢复空则显示空态。
6. 关闭交互：面板层包装 close 系列——脏标签 `ElMessageBox.confirm`；批量关闭跳过脏标签并 `ElMessage.info("已跳过 N 个未保存标签")`。
7. 空态：无标签时编辑区显示 Task 3.8 的「未选中」提示升级版（「从左侧选择接口，或 ＋ 新建临时请求」）。

验证（重点手动冒烟，逐条过）：

- 开 3 个接口标签切换编辑互不串扰；改 A 不保存切 B 再回 A 内容还在且 A 有 `●`。
- 关脏标签有确认；关闭其他跳过脏标签。
- 重启应用标签恢复、脏标记保留、响应区为空。
- 删除已开接口/集合的兜底转 temp。
- 历史载入/cURL 导入进新临时标签。
- 移动接口到其他文件夹后，后台标签保存不再拖回旧文件夹（folderId 回填生效）。

```
pnpm test src/utils/apiWorkbenchTabs.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

提交：`feat(api-workbench): 多标签打开接口（Postman 式/脏保护/重启恢复）`

---

## Phase 4B：树内拖拽（独立提交）

### Task 4.5 侧栏拖拽

**文件：** 修改 `ApiWorkbenchSidebar.vue`（主）、`ApiWorkbenchPanel.vue`（新增两个 command 处理或复用既有 move/reorder handler——优先复用：拖拽落点直接调用面板既有 `moveRequest`/`reorderRequest` 系函数需要目标参数，故在 Sidebar emit 新命令 `request:drop` / `folder:drop` 携带落点，由面板直调通道后 `loadAll`）

参照 `LauncherPanel.vue` 原生拖拽模式（`draggable="true"` + `dragstart/dragover/dragleave/drop`，`DataDictionaryPanel` 是 SortableJS 不作参照）：

- `dragstart`：`dataTransfer.setData("application/x-lazycat-apiwb", JSON.stringify({ type: "request"|"folder", id }))`。
- 目标高亮两种：悬停文件夹行中部 → 「移入」高亮（`drop-into` class）；悬停行上/下 1/4 区 → 间隙指示线（`drop-before/after`）。计算落点的判定函数（给定 `offsetY/rowHeight` 返回 `"into"|"before"|"after"`）入 `utils/apiWorkbenchTree.ts` + 单测。
- drop 分派：
  - request → folder(into)/未分组：`tool:api-workbench:request-move`；
  - request → 同层间隙：由现序列算新 `orderedIds`（复用 `moveApiWorkbenchOrderedId` 思路，新增 `reorderApiWorkbenchIdsByDrop(orderedIds, dragId, targetId, position)` 纯函数 + 单测）调 `request-reorder`；
  - folder → folder(into)：先用 `buildApiWorkbenchFolderMoveTargets` 校验合法（禁自身后代），非法落点不高亮不响应；合法调 `folder-move`；
  - folder → 同层间隙：`folder-reorder`。
- 跨集合拖拽不响应（drag data 带 collectionId 校验）；成功后 `loadAll()` 刷新（folderId 回填随 4.4 已挂）。右键菜单原路径保留。

```
pnpm test src/utils/apiWorkbenchTree.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

手动冒烟：接口拖入文件夹/拖到未分组/同层排序、文件夹拖入与排序、拖自身后代无响应、拖拽后右键菜单排序仍正确。

提交：`feat(api-workbench): 侧栏树拖拽排序与移动`

---

## 收尾

1. 全量验证：`pnpm test`、`pnpm typecheck`、`pnpm --filter @lazycat/desktop build:web`、`cd apps/desktop/src-tauri; cargo test api_workbench -- --nocapture`。
2. 清理：确认无遗留调试日志、未使用导入、Phase 3 的注释锚点已消化。
3. `process.md` 记录本次经验（多标签状态迁移模式、ureq 重定向语义、KV 组件化）。
4. 若需发布验证再跑 `pnpm test:e2e`。
