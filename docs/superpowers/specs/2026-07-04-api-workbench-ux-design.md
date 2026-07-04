# 接口调试（api-workbench）细节与交互优化设计

## 概述

接口调试工具（`api-workbench`）功能主干已完整：集合树、环境变量、发送链路、历史沉淀、cURL 导入导出、多类型响应预览。但日常使用中的交互细节仍显粗糙：KV 编辑器要手动加行、Body 与响应无高亮、改完参数切走即丢、一次只能开一个接口等。

本设计对该工具做一轮全面细节走查优化，共 18 项，覆盖请求编辑区、响应查看区、工作流与导航三个区域，其中多标签为唯一架构级改动。

## 已确认决策

1. 范围：全部 18 项纳入本次设计，分四批实施，多标签作为压轴批次。
2. 多标签模型：Postman 式——侧栏单击即开标签、已开则聚焦、常驻直到手动关闭、重启恢复（与应用顶部工具标签 `useTabs` 行为一致）。
3. URL 与 Query 联动：单向拆分（粘贴/失焦时把 `?a=b` 拆进 Query 页签）+ 常驻只读「最终 URL 预览」；不做双向实时同步。
4. 响应 JSON 查看：树 + Monaco 双模式——预览用现成 `JsonTreeViewer`（结构级折叠），原文/源码用 `MonacoPane` 只读（高亮 + Ctrl+F 搜索）；超阈值自动降级 Monaco 原文。
5. 跟随重定向：默认关闭，保持既有「展示原始 3xx」语义；作为请求级可选项开启。

## 目标 / 非目标

### 目标

1. 多标签同时打开多个接口，未保存修改随标签保留，重启后恢复。
2. KV 编辑器免手动加行，支持批量粘贴拆分与常用 Header 补全。
3. Body 编辑与响应查看获得语法高亮、结构折叠、响应内搜索。
4. URL 粘贴自动拆参，最终请求 URL 随时可见、可复制。
5. 未保存修改有可见标记，关闭前有确认，不再静默丢失。
6. 高频操作补齐：复制接口、拖拽排序/移动、cURL 大弹窗、历史操作收敛、空态引导。
7. 超时与跟随重定向可按请求配置。
8. 逻辑尽量落入纯函数并配单测，组件只做状态编排。

### 非目标

1. 不做请求脚本（pre-request / tests）、GraphQL、WebSocket、multipart 文件上传。
2. 不做响应 diff、与 api-mock 的联动。
3. 不改历史记录、示例响应的存储模型与后端 action 集合（仅 send 链路加重定向参数）。
4. Body 的 Monaco 编辑器内不做 `{{变量}}` 补全（v1 仅 URL 输入框与 KV value 列）。
5. 不做跨集合拖拽、标签条内拖拽排序。
6. 不引入第三方拖拽或 JSON 查看依赖（复用项目内现成实现）。
7. 不动响应缓存、二进制/Office 预览既有机制。

## 多标签架构（核心改动）

### 状态模型

新增 `composables/useApiWorkbenchTabs.ts` 管理标签数组与激活标签。每个标签：

```ts
interface ApiWorkbenchTab {
  id: number;                     // 本地自增，会话内唯一
  kind: "request" | "temp";       // 已保存接口 / 临时草稿
  requestId: number | null;        // kind=request 时非空
  collectionId: number;            // 归属集合
  folderId: number | null;
  name: string;
  draft: ApiWorkbenchRequestDraft;
  response: ApiWorkbenchSendResult | null;  // 不持久化
  savedSnapshot: { name: string; draft: ApiWorkbenchRequestDraft } | null; // 脏比较基准
  sourceHistoryId: number | null;
  editorTab: string;               // query/headers/body，会话内
  responseTab: string;             // response/headers/history，会话内
}
```

面板现有的 `draft` / `requestName` / `response` / `selectedRequestId` 等单例 ref 改为激活标签的计算引用；发送、保存等函数逻辑不变，数据来源改为激活标签。

脏状态 = `normalizeApiWorkbenchDraft(draft)` 与 `savedSnapshot.draft` 归一化对比，或名称变化；`temp` 标签只要有内容改动即视为脏。比较、恢复归一化、关闭后邻接选择（右邻 > 左邻）等逻辑放入 `utils/apiWorkbenchTabs.ts` 纯函数并配单测。

### 标签生命周期

- 侧栏单击接口：该 `requestId` 已开则聚焦，否则 `request-get` 后新开标签。
- 新建接口、载入历史、导入 cURL：一律新开 `temp` 标签（标题渲染尾缀 `*`，不入库）。载入历史不再弹「覆盖当前草稿」确认。
- 关闭：`×`、中键、右键菜单（关闭、关闭其他、关闭左侧、关闭右侧）。关闭脏标签逐个 `ElMessageBox` 确认；批量关闭遇脏标签跳过并提示数量。
- 保存：`request-save` 成功后更新 `savedSnapshot` 清脏；`temp` 标签保存后转为 `request` 标签。
- 侧栏删除接口：其标签若脏则转 `temp` 保留内容（防误删丢工作），干净则直接关闭。
- 删除集合：该集合的干净标签关闭，脏标签转 `temp` 并归属当前选中集合。
- 标签上限 20 个，超出时提示先关闭部分标签。

### 集合与环境归属

保持「单一当前集合」心智：切换到某标签时，若其 `collectionId` 与当前选中集合不同，侧栏与环境下拉自动跟随切换。发送时使用标签自己的 `collectionId` 与该集合当前激活环境。`temp` 标签归属创建时的当前集合。

### 持久化与恢复

- `user_settings` key：`api-workbench:tabs`，JSON：`{ version: 1, activeTabId, tabs: [...] }`；每个标签存 `kind`、`requestId`、`collectionId`、`folderId`、`name`、完整 `draft`、`savedSnapshot`、脏标记。`response` 不持久化（体积大，历史可查），恢复后响应区为空态。
- 恢复时逐个校验：JSON 解析失败或版本不符 → 放弃恢复从空白开始；`requestId` 已不存在 → 转 `temp` 标签；集合已不存在 → 转 `temp` 归属当前集合。恢复上限 20 个。
- 保存时机：标签集合变化（开/关/切换/改名/保存）时防抖写入。

### 标签条 UI

编辑区顶部一条横向可滚动标签栏：Method 颜色徽标 + 名称 + 脏标记 `●` + `×`；尾部 `＋` 新建临时标签。激活标签高亮，样式对齐应用顶部工具标签。

## 请求编辑区设计

### KV 编辑器组件化

把面板内运行时 `defineComponent` 的 KeyValueEditor 抽为正式组件 `ApiWorkbenchKeyValueEditor.vue`：

- 末行任一输入框有内容时自动追加空行；空行由既有 `normalizeRows` 过滤，不影响发送与保存。
- 删除改为行悬停显示的图标按钮；启用开关保留。
- 批量粘贴：在 Key 列粘贴的文本包含换行、`&` 或 `:` 时，按纯函数 `parseApiWorkbenchKvPaste` 拆分为多行（支持 `a=1&b=2` query-string、逐行 `key: value`、逐行 `key=value` 三种形态；不做 URL 解码，保持字符原样），拆分后 `ElMessage` 提示行数；Value 列粘贴不拆分。
- Query / Headers / 环境变量 / form 四处共用该组件。

### Header 自动补全

Headers 模式下 Key 列换用 `el-autocomplete`，候选来自常用头名常量表（`Content-Type`、`Authorization`、`Accept`、`User-Agent`、`Cookie` 等约 20 项）；Key 为 `Content-Type` 时 Value 列给常用 MIME 候选。常量表放 `utils/apiWorkbenchHeaders.ts`。

### Body Monaco 编辑

`bodyType` 为 `json` / `text` 时以 `MonacoPane`（language: json / plaintext）替换 textarea。工具条：格式化（`JSON.parse` + 缩进 2 重排，失败提示错误位置且不改动内容）、压缩。JSON 语法错误由 Monaco 内建诊断即时标出。

### URL 拆分与最终 URL 预览

- 拆分：URL 输入框粘贴或失焦时检测 `?`，将参数追加进 Query 页签（保留既有行），URL 栏只留路径部分；`{{var}}` 按普通字符串处理；无法按 `=` 配对的片段整段作为 key 保留。纯函数 `splitApiWorkbenchUrlQuery`。
- 预览：请求栏下方常驻一行只读「最终 URL」，复用 `buildApiWorkbenchPreviewUrl` 并做变量替换（当前环境 + 全局），缺失变量以 `{{name}}` 高亮警示色展示；单击整行复制。

### 页签计数徽标

编辑区页签标题显示内容计数：`Query (2)`、`Headers (3)`、`Body (·)`。计数口径与发送一致：enabled 且 key 非空的行数；Body 非 none 且内容非空时显示 `·`。

### 请求设置（超时 / 重定向）

请求栏增加 `⚙` 按钮弹 popover：

- 超时毫秒：数字输入，绑定既有 `draft.timeoutMs`（后端已有 `timeout_ms` 列与 `clamp_timeout_ms`，仅补前端入口）。
- 跟随重定向：开关，绑定新增 `draft.followRedirects`，默认关。移除现在 Body 工具条上永久禁用的占位开关。

### 认证辅助

Headers 页签工具条「快速认证」按钮弹 popover：Bearer（token 输入）/ Basic（用户名 + 密码，前端 base64）两种，确认后生成或更新 `Authorization` 行。纯前端实现，生成逻辑入纯函数。

### 变量自动补全

URL 输入框与 KV value 列输入 `{{` 时，弹出轻量候选浮层（统一组件 `ApiWorkbenchVariablePopover.vue`）列出当前环境 + 全局变量名，选中插入 `{{NAME}}`。候选数据复用 `summarizeApiWorkbenchVariables` 的变量来源。

## 响应查看区设计

### JSON 树 + Monaco 双模式

`ApiWorkbenchResponseViewer.vue` 内部改造，对外接口不变：

- `viewerKind === "json"` 的预览模式：`JSON.parse(bodyText)` 成功且 `bodyText` 长度 ≤ 1 MB 时渲染 `JsonTreeViewer`（复用其折叠、展开全部、折到 2 层、复制工具）；解析失败或超阈值自动降级为 Monaco 只读原文并提示原因。
- 原文/源码模式：`MonacoPane` 只读（json / html / xml / plaintext 按 MIME 选语言），自带高亮与 Ctrl+F 搜索。
- 图片、PDF、Office、二进制分支维持现状。

### 状态行增强

- 状态码色阶：2xx 绿（success）、3xx 橙（warning）、4xx/5xx 红（danger）、网络错误灰底 `ERR`。映射入纯函数。
- 大小与耗时人性化：`1.2 KB`、`356 ms` / `1.4 s`。字节格式化将 `apiMock.ts` 的 `formatMockFileSize` 上提为 `utils/format.ts` 的 `formatByteSize`（原函数转调），耗时格式化与相对时间一并新增于该文件。

### 响应头表格化

响应头页签由 `pre` 改为两列网格：行悬停显示「复制值」按钮；页签标题带数量「响应头 (12)」；保留整体复制按钮。

## 工作流与导航设计

### 接口复制

侧栏接口右键菜单增加「复制接口」：前端组合 `request-get` + `request-save`（id 传空、名称「原名 副本」、同文件夹末尾），无新后端 action；成功后在新标签打开副本。

### Method 色板

统一 CSS class（`method-get` 等）：GET 绿、POST 橙、PUT 蓝、PATCH 青、DELETE 红、HEAD/OPTIONS 灰。侧栏树、标签条、历史列表、请求栏 Method 选择器共用。浅色主题下同步检查 `element-overrides.css` 与 `theme-light.css`。

### 树内拖拽

侧栏树原生 HTML5 drag（参照 `DataDictionaryPanel` 既有模式，不引依赖）：

- 接口拖到文件夹（或「未分组」）上高亮 → 调 `request-move`；拖到同层行间隙 → 生成新顺序调 `request-reorder`。
- 文件夹拖到文件夹上 → `folder-move`（复用 `buildApiWorkbenchFolderMoveTargets` 的合法性校验，禁止拖入自身后代）；同层间隙 → `folder-reorder`。
- 限同集合内；右键菜单的移动/上移/下移保留兜底。

### cURL 导入弹窗

替换 `ElMessageBox.prompt` 为专用 `ApiWorkbenchCurlImportDialog.vue`：左侧大输入区，右侧随输入实时解析（复用 `parseApiWorkbenchCurl`）展示 Method / URL / Query / Headers / Body 结构化预览与警告列表；解析失败即时提示。确认后导入到新临时标签。

### 历史列表收敛

- 每行主操作：单击载入到新临时标签（原「载入」按钮语义，不再需要覆盖确认）。
- 按钮收敛为三个：星标图标（切换标星）、重放、`⋯` 更多下拉（保存为接口、备注）。
- 时间显示相对格式（刚刚 / N 分钟前 / N 小时前 / 昨天 HH:mm / MM-DD HH:mm），悬停 title 显示绝对时间；纯函数 `formatRelativeTime` 入 `utils/format.ts`。

### 空状态引导

- 无集合：编辑区显示引导卡（「新建集合 → 新建接口 → 发送」三步 + 新建集合按钮）。
- 有集合但无打开标签：显示「从左侧选择接口，或新建临时请求」空态 + 快捷键提示（Ctrl+Enter 发送、Ctrl+S 保存）。

## 前端架构

### 组件拆分（新增）

| 组件 | 职责 |
|------|------|
| `ApiWorkbenchTabsBar.vue` | 标签条渲染与右键菜单，事件上抛 |
| `ApiWorkbenchKeyValueEditor.vue` | KV 编辑（自动加行/粘贴拆分/补全），替换运行时组件 |
| `ApiWorkbenchCurlImportDialog.vue` | cURL 导入专用弹窗 |
| `ApiWorkbenchVariablePopover.vue` | `{{` 变量候选浮层 |
| `composables/useApiWorkbenchTabs.ts` | 标签数组、激活标签、持久化防抖 |

请求设置 `⚙` 与快速认证以 popover 形式实现在面板/Headers 工具条内，不单独拆文件（体量小）。

### 纯函数（配单测）

- `utils/apiWorkbenchTabs.ts`：脏比较、恢复归一化与降级（requestId 失效转 temp）、关闭邻接选择、聚焦查找。
- `utils/apiWorkbenchKvPaste.ts`：粘贴文本三形态解析。
- `utils/apiWorkbench.ts` 增补：`splitApiWorkbenchUrlQuery`、认证头生成、页签计数、状态码色阶映射。
- `utils/format.ts`（新）：`formatByteSize`（上提自 apiMock）、`formatDurationMs`、`formatRelativeTime`。

### 复用清单

- `MonacoPane.vue`（Body 编辑、响应原文只读）
- `common/JsonTreeViewer.vue`（JSON 响应预览）
- `buildApiWorkbenchPreviewUrl`、`normalizeApiWorkbenchDraft`、`parseApiWorkbenchCurl`、`summarizeApiWorkbenchVariables`、`moveApiWorkbenchOrderedId`
- 既有通道：`request-move` / `request-reorder` / `folder-move` / `folder-reorder`（拖拽落点复用，无新通道）
- `DataDictionaryPanel` 拖拽事件处理模式、应用级 `useTabs` 的关闭邻接语义（参考实现，不直接复用）

## 后端改动

仅一处行为扩展，无新 action：

1. `api_workbench_requests` 建表语句与兼容迁移增加列 `follow_redirects INTEGER NOT NULL DEFAULT 0`（参照既有 ALTER 兼容模式）。
2. 请求草稿 serde 结构体增加 `follow_redirects: bool`，`#[serde(default)]` 兼容旧数据与旧历史快照。
3. `send` 路径：`follow_redirects = true` 时 reqwest 客户端配 `redirect::Policy::limited(10)`，`finalUrl` 取跳转后最终地址；`false` 保持现状（不跟随，展示原始 3xx）。超过跳转上限时错误按既有 `error` 字段透出。
4. `request-save` / `request-get` / 历史快照序列化链路带上新字段。

## 错误处理

- 标签恢复：settings JSON 损坏或版本不符 → 静默放弃恢复，从空白开始；单个标签校验失败只丢弃该标签。
- 响应树渲染：`JSON.parse` 失败或超 1 MB → 自动降级 Monaco 原文，顶部提示降级原因。
- Body 格式化：JSON 非法 → 提示错误位置，不改动内容。
- 粘贴拆分：无法按 `=`/`:` 配对的行整行作为 key 保留，不丢内容。
- 拖拽落点非法（拖入自身后代等）→ 落点不高亮、drop 无操作。
- 后端重定向超限、超时 → 沿用既有 `error` 透出与历史记录路径。
- 关闭/切换过程中发送中的请求：沿用现状（响应回填时标签已关则丢弃，以标签 id 快照绑定，防旧响应写错标签）。

## 分批实施

| 批次 | 内容 | 依赖 |
|------|------|------|
| 1 编辑区细节 | KV 组件化+粘贴+Header 补全、URL 拆分、最终 URL 预览、页签徽标、Body Monaco | 无 |
| 2 响应区 | JSON 树+Monaco 双模式、状态行色阶与格式化、响应头表格（含 `utils/format.ts` 上提） | 无 |
| 3 工作流小项 | Method 色板、复制接口、cURL 弹窗、历史收敛、空状态、请求设置（含后端重定向）、认证辅助、变量补全 | 无 |
| 4 压轴 | 多标签重构（脏保护并入）、树内拖拽 | 批次 1-3 交互落点稳定后进行 |

每批独立验证、独立提交。批次 3 中 cURL 弹窗与历史载入的「进入新临时标签」行为在批次 4 前先落为「替换当前草稿（带确认）」，批次 4 切换为开新标签。

## 测试计划

### 前端（`pnpm test`）

- 新增纯函数单测：`apiWorkbenchTabs`（脏比较/恢复降级/邻接选择）、`apiWorkbenchKvPaste` 三形态、`splitApiWorkbenchUrlQuery`（含 `{{var}}`、无值参数）、认证头生成、状态码色阶、`format.ts` 三个函数（含边界 0/负值/超大）。
- 既有 `apiWorkbench*.test.ts` 全部保持通过；`formatMockFileSize` 转调后 apiMock 测试不回归。

### Rust（`cargo test api_workbench -- --nocapture`）

- `follow_redirects` 默认值与显式开启的 send 分支、旧数据反序列化兼容、建表迁移幂等。

### 构建

- 每批次：`pnpm typecheck` + `pnpm --filter @lazycat/desktop build:web`；全部完成后跑完整 `pnpm test`。

## 风险与边界

1. `ApiWorkbenchPanel.vue` 已 2133 行，多标签重构进一步增加复杂度——通过 composable + 纯函数 + 新组件拆分对冲，面板只留状态编排；批次 4 是唯一动主干状态的批次，放最后且独立提交。
2. 标签持久化存完整草稿，`user_settings` 单 key 体积可控（20 标签上限，不含响应体）。
3. Monaco 实例数量：Body 编辑 + 响应只读最多同屏 2 个实例，与 DiffPanel 双栏先例一致，可接受。
4. 拖拽与右键菜单并存期间以后端排序为真源（沿用既有约定），拖拽只发落点请求，成功后 `loadAll` 刷新。
5. 样式改动涉及 Element Plus 变量时同步检查 `element-overrides.css` 与 `theme-light.css`。
