# 任务与项目描述富文本化（TipTap）设计文档

日期：2026-04-20

## 1. 背景与目标

### 1.1 背景

LazyCat 当前有三处"描述"字段以 `el-input type="textarea"` 呈现：

- `PmProjectDialog.vue:8`：项目描述
- `PmItemDialog.vue:154`：工作项描述
- `TodoDetailEdit.vue:386`：Todo 描述（上方另有一套土法 Markdown 工具栏）

仅支持纯文本。用户在实际使用中需要：

- 粘贴截图或图片文件
- 插入外链（URL 自动识别）
- 基础排版（标题、粗体、斜体、列表、引用、代码、分割线）
- 未来：链接本地文件、上传文件、浏览附件（P2）

### 1.2 目标

用 TipTap 3 替换三处 textarea，形成统一的富文本描述体验；后端增加 `attachments` 基础设施以承载图片与未来的文件附件。

### 1.3 非目标（当前版本不做）

- 文件附件 Node（P2）
- 附件库 / 浏览抽屉（P2）
- description 全文搜索兼容（P2，后续可通过影子 text 列解决）
- 协作编辑、版本历史、diff
- OCR、图像压缩、格式转换
- 定时孤儿清理（P2 再加）

## 2. 选型结论

### 2.1 编辑器：TipTap 3

- Vue 3 一等支持（`@tiptap/vue-3`）
- headless 架构，可用 Element Plus 自拼 toolbar，保证浅色主题一致
- MIT 协议、npm 全本地打包，满足"离线、不依赖 CDN"约束
- ProseMirror 内核，扩展生态最成熟

### 2.2 描述存储格式：JSON

用 `editor.getJSON()` 的 stringified 结果存入 `description TEXT` 字段。理由：

- 未来 P2 上 FileAttachment Node（7 个自定义属性），JSON 原生保 attrs，不需要手写 parseHTML / renderHTML 样板
- 避免"HTML → JSON"半年后的切换阵痛（切换期要在只读、清理、搜索三处分别写格式判定分支，容易遗漏）
- 旧纯文本懒升级：第一次编辑保存时自动写回 JSON，未编辑项保持原样
- 只读渲染通过 `@tiptap/static-renderer` 转 HTML，渲染前 walk 节点重写本地路径

## 3. 关键决策汇总

| # | 议题 | 决策 |
|---|---|---|
| 1 | 附件物理目录结构 | hash 扁平：`attachments/<hash>.<ext>` |
| 2 | Image 节点 attrs | `{ attId, src: <相对路径>, alt? }` 双字段 |
| 3 | description 空值 | 空字符串 `''`（不触发升级） |
| 4 | Viewer 解析本地路径 | 渲染前 walk JSON 把 `src` 重写为 `convertFileSrc` 结果 |
| 5 | 字节传输 | base64 over JSON，单文件 5 MB 上限 |
| 6 | 主表删除联动 | pm/todo 的 delete 函数末尾显式调 `attachments::delete_by_owner`；`project_delete` 事务内显式清理 `pm_items`（无 FK CASCADE） |
| 7 | tempId 生命周期 | 新建场景组件内部生成 `tmp-<uuid>`，submit 后父组件调 rebind，cancel 调 cleanup |
| 8 | CSP / assetProtocol | 追加 `asset:` + `http://asset.localhost`，`Cargo.toml` 的 `tauri` 依赖追加 `protocol-asset` feature，main.rs 运行时 `allow_directory(attachments_dir)` |
| 9 | 外链 Link 配置 | 白名单 `['http','https','mailto']` + `rel="noopener noreferrer"`；`openOnClick: false`，点击走 `tool:system:open_external` |
| 10 | 图片粘贴上限 | 5 MB，超限前端弹窗拦截 |
| 11 | 旧数据升级 | 懒升级，仅在用户编辑时自动写回 JSON |
| 12 | rebind 是否 MVP | 是，保证 owner_id 准确，为 P2 附件库铺路 |
| 13 | 只读 Viewer 实现 | `@tiptap/static-renderer` + 预处理本地路径 |

## 4. 数据模型

### 4.1 新增表：`attachments`

```sql
CREATE TABLE IF NOT EXISTS attachments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_type TEXT NOT NULL,            -- 'pm_project' | 'pm_item' | 'todo'
    owner_id   TEXT NOT NULL,            -- 允许字符串，兼容 'tmp-<uuid>' 暂存
    rel_path   TEXT NOT NULL,            -- 相对 <data_dir>/attachments/，不含前导 '/'
    original_name TEXT NOT NULL DEFAULT '',
    mime       TEXT NOT NULL DEFAULT '',
    size       INTEGER NOT NULL DEFAULT 0,
    hash       TEXT NOT NULL DEFAULT '', -- blake3 前 16 字节 hex = 32 字符
    kind       TEXT NOT NULL DEFAULT 'file',  -- 'image' | 'file'
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_attachments_owner ON attachments(owner_type, owner_id);
CREATE INDEX IF NOT EXISTS idx_attachments_hash  ON attachments(hash);
```

设计说明：

- `owner_type` 用字符串枚举而非外键，避免三表分离；常量由 Rust 侧导出
- `owner_id` 为 TEXT 以容纳 `tmp-<uuid>`；普通 owner 用数字 id 的字符串形式
- `hash` 作为物理文件名（去扩展名），同一文件内容多 owner 引用仅存一份
- `rel_path` = `attachments/<hash>.<ext>`，跨平台统一用 `/` 分隔

### 4.2 现有表不改

- `pm_projects.description`、`pm_items.description`、`todo_items.description` 类型保持 `TEXT NOT NULL DEFAULT ''`
- 语义变为 stringified JSON；兼容空字符串和 legacy 纯文本

## 5. 目录结构与路径约定

### 5.1 物理目录

```
<data_dir>/
├── lazycat.sqlite
├── hosts-backups/
└── attachments/
    ├── 0a1b2c3d4e5f60718293a4b5c6d7e8f9.png
    ├── 7f8e9d0c1b2a3948576655a4b3c2d1e0.pdf
    └── ...
```

`<data_dir>` 默认 `~/.lazycat`，用户可在设置中自定义。

### 5.2 路径解析

| 场景 | 计算 |
|---|---|
| 后端落盘绝对路径 | `<data_dir>/attachments/<hash>.<ext>` |
| DB `rel_path` 存储 | `attachments/<hash>.<ext>`（无前导斜杠，用 `/`） |
| 节点 `attrs.src` | 等于 `rel_path`，用 `/` 分隔 |
| 前端渲染 URL | `convertFileSrc(<data_dir>/attachments/<hash>.<ext>)` |

其中 `<hash>` 为 blake3 digest 前 16 字节的 hex 形式（= 32 字符）。

前端启动后通过 `tool:system:get_paths` 一次性缓存 `dataDir` 与 `attachmentsDir`，之后拼路径不再查后端。

## 6. 后端 API 契约

### 6.1 新增 `tool:attachments:*` 通道

| Channel | 入参 | 出参 |
|---|---|---|
| `tool:attachments:save` | `{ ownerType, ownerId, fileName, mime, kind, bytesBase64 }` | `{ id, relPath, hash, size }` |
| `tool:attachments:list` | `{ ownerType, ownerId }` | `Attachment[]` |
| `tool:attachments:remove` | `{ id }` | `{ removedFile: boolean }` |
| `tool:attachments:rebind` | `{ ownerType, fromOwnerId, toOwnerId }` | `{ updated: number }` |
| `tool:attachments:cleanup_orphans` | `{ ownerType, ownerId, keepIds: number[] }` | `{ removedCount, removedFiles }` |
| `tool:attachments:delete_by_owner` | `{ ownerType, ownerId }` | `{ removedCount, removedFiles }` |

### 6.2 新增 `tool:system:*` 通道

| Channel | 入参 | 出参 |
|---|---|---|
| `tool:system:get_paths` | `{}` | `{ dataDir, attachmentsDir }` |
| `tool:system:open_external` | `{ url }` | `{ ok: true }` |

（独立出 `system` 域是为了未来承接更多"环境探测 / 跨平台系统交互"类查询；`open_external` 内部实现调 `open::that`，仅接受 `http/https/mailto` 协议白名单，其他协议直接 `Err`）

### 6.3 关键算法

#### save

```
1. base64 decode → Vec<u8>
2. 校验 size <= 5 MB，超限 Err("single image exceeds 5 MB")
3. blake3 hash → hex 前 16 字节 (= 32 字符)
4. 计算 ext：优先从 mime（image/png → png），回退从 fileName；禁止 image/svg+xml
5. 先 SELECT rel_path FROM attachments WHERE hash=? LIMIT 1
   命中：复用已有 rel_path（避免同内容不同扩展名产生双份）
   未命中：rel_path = format!("attachments/{}.{}", hash, ext)
6. abs_path = <data_dir>/<rel_path>
7. 若 abs_path 不存在，fs::write
8. INSERT attachments; 返回 { id, relPath, hash, size }
```

#### remove

```
1. SELECT hash, rel_path FROM attachments WHERE id=?
2. DELETE FROM attachments WHERE id=?
3. SELECT COUNT(*) FROM attachments WHERE hash=?
4. 若 count=0，fs::remove_file(abs_path)，返回 removedFile=true
5. 否则 removedFile=false
```

#### cleanup_orphans

```
1. SELECT id, hash, rel_path FROM attachments
     WHERE owner_type=? AND owner_id=? AND id NOT IN (keepIds)
2. DELETE FROM attachments WHERE id IN (待删 ids)
3. 对每个被删 hash，再次 SELECT COUNT(*) WHERE hash=?，=0 则 remove_file
4. 返回 { removedCount, removedFiles }
```

#### rebind

```
1. UPDATE attachments SET owner_id=:to
     WHERE owner_type=:ot AND owner_id=:from
2. 返回 updated 行数
（无文件移动，因为物理文件按 hash 扁平存放）
```

#### delete_by_owner

```
1. 等同于 cleanup_orphans(ownerType, ownerId, keepIds=[])
```

### 6.4 主表删除联动

修改 `pm.rs` / `todo.rs` 中的：

- `project_delete` → 事务内显式清理 `pm_items`（**当前 `pm_items` 无 `FOREIGN KEY ... ON DELETE CASCADE`，且 `project_delete` 也不显式删 `pm_items`，本方案一并修复该孤儿问题**），具体流程见 §13
- `item_delete`（PM） → 清该 item 附件
- `item_delete`（Todo，含 `delete_item_by_id` 兜底路径） → 清该 todo 附件

实现方式：在每个 delete 函数末尾（事务内或之后）调 `attachments::delete_by_owner_internal(&conn, owner_type, owner_id)`，提供一个不经过 JSON payload 的内部函数签名避免重复序列化。

## 7. 前端组件契约

### 7.1 `RichDescriptionEditor.vue`

```ts
defineProps<{
  modelValue: string                          // stringified JSON / '' / legacy 纯文本
  ownerType: 'pm_project' | 'pm_item' | 'todo'
  ownerId?: string | number | null            // 仅 null/undefined 生成 tempId；0 / '' 视为合法 id
  placeholder?: string
  maxImageMb?: number                         // 默认 5
}>()

defineEmits<{
  'update:modelValue': [value: string]
  'attachment-added': [attId: number]
  'oversize': [mb: number]
}>()

defineExpose<{
  focus: () => void
  blur: () => void
  getAttachmentIds: () => number[]            // 遍历当前 doc 收集所有 attId
  getEffectiveOwnerId: () => string           // realId 或 tempId 的字符串
}>()
```

内部行为：

- 挂载时，若 `ownerId == null`（仅 `null` / `undefined`，不含 `0` / `''`），内部生成 `tempId = 'tmp-' + crypto.randomUUID()`，保存在 `ref`
- `normalizeLegacy(raw)` 返回初始 doc：
  - `raw` 为空 → 空 doc
  - `raw.trim().startsWith('{')` → `JSON.parse`，失败则按纯文本处理
  - 其他 → `{ type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: raw }] }] }`
- `onUpdate` 时 emit `JSON.stringify(editor.getJSON())`
- toolbar 使用 `el-button-group` + `el-tooltip`，样式写 `scoped`，变量引用 `theme-light.css`

**粘贴 / 拖拽图片的异步 UX**（必须有占位 + loading）：

1. `onPaste` 拿到 File：若 `file.size > maxImageMb * 1024 * 1024`，`ElMessage.warning` 并中止
2. 立即 `URL.createObjectURL(file)` 作为临时 blob URL，插入一个带 `uploadingId: <uuid>` 的 Image 节点：`setImage({ src: blobUrl, uploadingId })`；该占位节点 CSS 加 loading 蒙层样式
3. 并行 invoke `tool:attachments:save`
4. **成功**：遍历 doc 找到对应 `uploadingId` 的节点，原地改为 `{ src: relPath, attId, uploadingId: null }`，`URL.revokeObjectURL(blobUrl)`
5. **失败**：遍历 doc 移除该占位节点，`URL.revokeObjectURL(blobUrl)`，`ElMessage.error(err)`
6. 组件 `onBeforeUnmount` 时遍历未完成的 `uploadingId`，统一 `revokeObjectURL`；进行中的 save 结果被丢弃

连续粘贴多图时，`uploadingId` 保证插入顺序与回填顺序解耦，不依赖 save 的返回顺序。

### 7.2 `RichDescriptionViewer.vue`

```ts
defineProps<{
  value: string
}>()
```

内部行为：

- `computed parsedDoc`：尝试 `JSON.parse(value)`；失败则构造 paragraph→text fallback
- `computed rewrittenDoc`：walk parsedDoc，将 `image` / `fileAttachment`（P2）的 `attrs.src` 从相对路径重写为 `convertFileSrc(<dataDir>/<src>)`
- `computed html`：`renderToHTMLString({ extensions: sharedExtensions, content: rewrittenDoc })`
- 模板：`<div class="rte-prose" v-html="html" />`

### 7.3 `useRichDescriptionLifecycle.ts`（composable）

```ts
export function useRichDescriptionLifecycle(opts: {
  ownerType: string
  editorRef: Ref<RichDescriptionEditorExposed | null>
  getRealId: () => string | number | null
}) {
  async function afterSubmit(realId: string | number) {
    const editor = opts.editorRef.value
    if (!editor) return
    const tempId = editor.getEffectiveOwnerId()
    if (!tempId.startsWith('tmp-')) return          // 编辑场景无需 rebind
    await invokeTool('tool:attachments:rebind', {
      ownerType: opts.ownerType,
      fromOwnerId: tempId,
      toOwnerId: String(realId),
    })
  }

  async function onCancel() {
    const editor = opts.editorRef.value
    if (!editor) return
    const ownerId = editor.getEffectiveOwnerId()
    if (!ownerId.startsWith('tmp-')) return         // 编辑场景：清掉被删除的附件
    await invokeTool('tool:attachments:cleanup_orphans', {
      ownerType: opts.ownerType,
      ownerId,
      keepIds: [],
    })
  }

  async function beforeCloseEdit() {
    // 编辑场景：保存前清理被用户删除的附件
    const editor = opts.editorRef.value
    if (!editor) return
    const realId = opts.getRealId()
    if (!realId) return
    await invokeTool('tool:attachments:cleanup_orphans', {
      ownerType: opts.ownerType,
      ownerId: String(realId),
      keepIds: editor.getAttachmentIds(),
    })
  }

  return { afterSubmit, onCancel, beforeCloseEdit }
}
```

### 7.4 共享 extensions 模块

新增 `apps/desktop/src/rich/extensions.ts`，导出 `sharedExtensions`：

```
StarterKit（含 Paragraph/Heading/Bold/Italic/Strike/Code/BulletList/OrderedList/
            Blockquote/CodeBlock/HardBreak/HorizontalRule/Dropcursor/Gapcursor/
            History）
Image.extend({ addAttributes: { attId, src, alt, uploadingId } })
Link.configure({
  openOnClick: false,
  autolink: true,
  defaultProtocol: 'https',
  protocols: ['http', 'https', 'mailto'],     // 显式白名单，拒绝 javascript: / data: / file:
  HTMLAttributes: { rel: 'noopener noreferrer' },
})
Placeholder.configure({ placeholder: '... 动态传入 ...' })
```

Viewer 渲染前的 `walkAttIds` / `rewriteLocalSrc` 合并到一次 walk 中，同时对 `link` mark 的 `attrs.href` 做 `sanitizeHref`：若 `trim().toLowerCase()` 以 `javascript:` / `data:` 开头则清空（降级为纯文本）。

Editor 和 Viewer 共用，保证 schema 完全一致。

### 7.5 链接点击行为

`Link.configure({ openOnClick: false })` 关闭默认打开。在 Viewer 层给 `<a>` 加点击监听：

```ts
// 点击链接调系统浏览器，不在 webview 内跳转
onMounted(() => {
  viewerEl.addEventListener('click', (e) => {
    const a = (e.target as Element).closest('a[href]')
    if (!a) return
    e.preventDefault()
    const href = a.getAttribute('href') ?? ''
    // 再次防御：仅放行 http/https/mailto
    if (!/^(https?:|mailto:)/i.test(href)) return
    invokeTool('tool:system:open_external', { url: href })
  })
})
```

（`tool:system:open_external` 见 §6.2；Rust 端实现为协议白名单 + `open::that`）

## 8. TipTap 依赖清单

### 8.1 前端 `apps/desktop/package.json` 新增

```
@tiptap/vue-3
@tiptap/pm
@tiptap/starter-kit
@tiptap/extension-image
@tiptap/extension-link
@tiptap/extensions             # Placeholder 所在包
@tiptap/static-renderer        # Viewer 专用
```

所有包 MIT，纯 ESM，Vite 可直接打包进应用，符合"离线、不依赖 CDN"。

### 8.2 后端 `apps/desktop/src-tauri/Cargo.toml`

已有的 `uuid`、`blake3`、`base64`、`rusqlite`、`dirs`、`serde_json`、`open` 全部够用，**无需新增 crate**。

但**必须修改 `tauri` 依赖的 feature 列表**，追加 `protocol-asset`：

```diff
-tauri = { version = "2", features = ["tray-icon", "devtools"] }
+tauri = { version = "2", features = ["tray-icon", "devtools", "protocol-asset"] }
```

理由：Tauri 2 下 `convertFileSrc` 所依赖的 `asset://` / `http://asset.localhost` 协议由 `protocol-asset` feature 提供；仅在 `tauri.conf.json` 中把 `assetProtocol.enable` 置 true 不足以启用真正的协议处理器，请求会被窗口拒绝。

## 9. 数据流时序

### 9.1 新建 PM Item（含粘贴图片）

```
用户打开 PmItemDialog
  └── <RichDescriptionEditor :ownerId="null" />
       └── 组件内部 tempId = 'tmp-abcd'

用户粘贴截图
  └── onPaste → File (PNG, 120KB)
       └── invoke('tool:attachments:save', {
              ownerType: 'pm_item', ownerId: 'tmp-abcd',
              fileName: 'image.png', mime: 'image/png',
              kind: 'image', bytesBase64: '...'
            })
            返回 { id: 42, relPath: 'attachments/0a1b...4f9.png', ... }
       └── editor.chain().setImage({ src: 'attachments/0a1b...4f9.png', attId: 42 })
       └── emit 'update:modelValue' → 父组件 form.description 更新

用户点击"确定"
  └── 父组件 submit → invoke pm:item_create({ description: formData.description })
                      返回 realId = 123
  └── lifecycle.afterSubmit(123)
       └── invoke attachments:rebind('pm_item', 'tmp-abcd', '123')
```

### 9.2 编辑 PM Item

```
用户点击某项 → 父组件填充 form.description = existingJson
打开 PmItemDialog
  └── <RichDescriptionEditor :ownerId="123" :modelValue="existingJson" />
       └── 内部不生成 tempId，直接用 '123'

用户删除了一张图片
  └── editor doc 中该 image 节点被移除，但 attachments 表记录还在

用户点击"确定"
  └── lifecycle.beforeCloseEdit()
       └── keepIds = editor.getAttachmentIds() = [42, 57]
       └── invoke attachments:cleanup_orphans('pm_item', '123', [42, 57])
            物理文件按 hash 引用计数决定是否删
  └── 父组件 submit → invoke pm:item_update
```

### 9.3 取消对话框（含附件已上传）

```
用户粘贴了图片，但点"取消"
  └── lifecycle.onCancel()
       └── tempId='tmp-abcd' 存在
       └── invoke attachments:cleanup_orphans('pm_item', 'tmp-abcd', [])
            删 DB 行 + 无引用的物理文件
```

### 9.4 只读渲染（详情面板）

```
PmDetailPanel 展示某 item
  └── <RichDescriptionViewer :value="item.description" />
       └── parsedDoc = JSON.parse(item.description)
       └── rewrittenDoc = walk(parsedDoc, 把 image.attrs.src
            从 'attachments/0a1b...4f9.png' 改为 convertFileSrc('<dataDir>/attachments/0a1b...4f9.png'))
       └── html = renderToHTMLString({ extensions, content: rewrittenDoc })
       └── v-html
```

### 9.5 删除 PM Item

```
用户点击"删除项目"
  └── 前端 invoke pm:item_delete({ id: 123 })
       Rust 端：
       ├── BEGIN TRANSACTION
       ├── SELECT ... FROM pm_items WHERE id=123
       ├── DELETE FROM pm_items WHERE id=123
       ├── attachments::delete_by_owner_internal(&conn, "pm_item", "123")
       ├── COMMIT
       └── 对被删记录的 hash，逐一判断引用计数后删物理文件
```

## 10. CSP 与 assetProtocol 配置

### 10.1 `tauri.conf.json` 修改

```diff
 "security": {
-  "csp": "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: http://127.0.0.1:*; font-src 'self' data:; connect-src 'self' http://127.0.0.1:* https://127.0.0.1:*; frame-src http://127.0.0.1:*; worker-src 'self' blob:"
+  "csp": "default-src 'self'; script-src 'self' 'unsafe-inline' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: http://127.0.0.1:* http://asset.localhost asset:; font-src 'self' data:; connect-src 'self' http://127.0.0.1:* https://127.0.0.1:* http://asset.localhost ipc: http://ipc.localhost; frame-src http://127.0.0.1:*; worker-src 'self' blob:"
+  ,
+  "assetProtocol": {
+    "enable": true,
+    "scope": ["**/.lazycat/attachments/**", "**/attachments/**"]
+  }
 }
```

说明：

- `http://asset.localhost` 是 Windows 下 `convertFileSrc` 的 origin 形式
- `asset:` 是 macOS/Linux 形式，跨平台都放行
- `connect-src` 也要加 `http://asset.localhost`，否则 fetch 场景会被挡
- `scope` 静态配置给出两个通配（默认目录和自定义目录后备）

### 10.2 `main.rs` 运行时动态 scope

在 `setup` 阶段补一行：

```rust
use tauri::Manager;

app.asset_protocol_scope()
   .allow_directory(&get_attachments_dir()?, true /* recursive */)
   .map_err(|e| format!("allow attachments dir failed: {e}"))?;
```

保证自定义数据目录也能加载。

### 10.3 capabilities 无需改动

`dialog:allow-open` / `allow-save` 已开，未来 P2 文件浏览直接用。

## 11. 旧数据兼容

### 11.1 兼容原则

- 任何时刻都不做全表迁移
- 懒升级：只在用户打开编辑器、触发 `onUpdate` 时才把 JSON 写回主表
- Viewer 永远接受三种输入：合法 JSON / 空字符串 / legacy 纯文本
- **对老版 Todo 描述中存量的 Markdown 标记（如 `**粗体**`、`# 标题`、列表符号等）不做 md→JSON 解析，统一按纯文本段落显示**；用户一旦在新编辑器里修改并保存，原 md 标记将以字面形式固化为段落文本。已知存在视觉退化，MVP 接受此降级，不补偿

### 11.2 Editor 的 normalizeLegacy 算法

```ts
function normalizeLegacy(raw: string): JSONContent {
  const EMPTY = { type: 'doc', content: [{ type: 'paragraph' }] }
  const t = raw?.trim() ?? ''
  if (!t) return EMPTY
  if (t.startsWith('{')) {
    try { return JSON.parse(t) } catch { /* fall through */ }
  }
  // 纯文本（含换行）→ 每行一个 paragraph
  const paragraphs = t.split(/\r?\n/).map(line => ({
    type: 'paragraph',
    content: line ? [{ type: 'text', text: line }] : [],
  }))
  return { type: 'doc', content: paragraphs }
}
```

### 11.3 Viewer 的 fallback

```ts
const parsed = computed(() => {
  const t = props.value?.trim() ?? ''
  if (!t) return null
  if (t.startsWith('{')) {
    try { return JSON.parse(t) } catch { return null }
  }
  return null  // 走纯文本 fallback
})

// 模板中：
// <div v-if="parsed" class="rte-prose" v-html="renderedHtml" />
// <div v-else class="rte-prose rte-legacy">{{ value }}</div>
```

`rte-legacy` 用 `white-space: pre-wrap` 保留换行，视觉上与富文本段落一致。

## 12. 错误处理与边界

| 场景 | 处理 |
|---|---|
| 粘贴图片超 5 MB | Editor 弹 `ElMessage.warning('单张图片不能超过 5 MB')`，不进入 save 流程 |
| save 失败（磁盘满/权限） | 移除占位节点 + `URL.revokeObjectURL(blobUrl)` + `ElMessage.error(err)` |
| Image 节点的 attId 引用的附件被删 | Viewer 渲染时 src 文件不存在 → 浏览器显示破图标，不报错（接受这种降级） |
| 用户在 Link 里手填 `javascript:` / `data:` | 编辑器保存后 Viewer 的 sanitizeHref 清空 href；点击监听也二次拦截 |
| 粘贴 SVG 或 image/svg+xml | `attachments:save` 直接 Err("svg not supported")；前端捕获后移除占位 |
| JSON.parse 失败 | Editor 走 legacy 纯文本路径；Viewer 走 legacy fallback |
| rebind 无任何行更新 | 静默接受（可能是 onCancel 已先走） |
| cleanup_orphans 物理文件删除失败 | 记 warning 不阻塞 DB 删除 |
| 组件被销毁时粘贴仍在进行 | 用 `onBeforeUnmount` 释放 editor + revokeObjectURL；未完成的 save 其结果被丢弃 |
| 数据目录迁移 | 现有 `action_migrate_data_dir` 目前只复制 `lazycat.sqlite` 与 `hosts-backups/`，**本方案在 `settings.rs:251` 附近同步追加 `attachments/` 的递归复制** |

## 13. 联动清理清单

在实施期需要改的 Rust 删除函数（均在 delete 路径末尾增加 `delete_by_owner_internal` 调用）：

| 文件 | 函数 | owner_type |
|---|---|---|
| `pm.rs` | `item_delete` | `pm_item` |
| `pm.rs` | `project_delete` | 先逐一清子 items 的 `pm_item`，再 `DELETE FROM pm_items`，最后清 `pm_project` |
| `todo.rs` | `item_delete` + 内部 `delete_item_by_id` | `todo` |

### 13.1 `project_delete` 的级联写法（修复现有孤儿 bug）

**现状（pm.rs:278-298）**：`project_delete` 只做 todo 归属校验后 `DELETE FROM pm_projects`，不清理 `pm_items`，也没 FK CASCADE，会产生孤儿 items。本方案一并修复：

```rust
fn project_delete(payload: &Value) -> Result<Value, String> {
    let id = parse_i64(payload, "id").ok_or("id is required")?;
    let conn = db_conn()?;

    // 校验 todo 归属（保持现有语义）
    let todo_count: i64 = /* ... 同原逻辑 ... */;
    if todo_count > 0 { return Err(/* 现有文案 */); }

    // 开启事务
    let tx = conn.unchecked_transaction().map_err(...)?;

    // 1. 收集子 pm_items.id，逐一清附件
    let item_ids: Vec<i64> = tx.prepare("SELECT id FROM pm_items WHERE project_id = ?1")?
        .query_map(params![id], |r| r.get(0))?
        .filter_map(|r| r.ok())
        .collect();
    for item_id in &item_ids {
        attachments::delete_by_owner_internal(&tx, "pm_item", &item_id.to_string())?;
    }

    // 2. 删子 items 行（无 FK CASCADE，必须显式）
    tx.execute("DELETE FROM pm_items WHERE project_id = ?1", params![id])?;

    // 3. 删项目自身附件
    attachments::delete_by_owner_internal(&tx, "pm_project", &id.to_string())?;

    // 4. 删项目行
    tx.execute("DELETE FROM pm_projects WHERE id = ?1", params![id])?;

    tx.commit().map_err(...)?;
    Ok(json!({ "ok": true }))
}
```

物理文件的引用计数清理由 `delete_by_owner_internal` 内部负责（按 hash 二次判断）。

### 13.2 Todo 侧

`todo.rs::item_delete`（现 todo.rs:2120）及其内部的 `delete_item_by_id`（todo.rs:2258）是所有 todo 删除路径的汇点；只在 `delete_item_by_id` 末尾加一行：

```rust
attachments::delete_by_owner_internal(conn, "todo", &item_id.to_string())?;
```

即可覆盖 this_instance / future_instances / recurring 补生成等全部分支。

## 14. 实施顺序与任务拆分

```
┌── #1 附件持久化层（Rust + DB + bridge）
│    └── helpers.rs: attachments 表 + get_attachments_dir()
│    └── tools/attachments.rs: save/list/remove/rebind/cleanup_orphans/delete_by_owner
│    └── tools/system.rs: get_paths + open_external（白名单 http/https/mailto）
│    └── tools/mod.rs: 注册 attachments + system 两个新 domain
│    └── bridge/tauri.ts: 追加 attachments 的 6 条 + system 的 2 条 channel
│
├── #2 CSP + assetProtocol（tauri.conf.json + Cargo.toml + main.rs setup）
│    └── Cargo.toml: tauri features 追加 "protocol-asset"
│    └── tauri.conf.json: CSP 追加 asset: / http://asset.localhost / ipc: / http://ipc.localhost；
│                        新增 assetProtocol.enable + scope
│    └── main.rs: asset_protocol_scope().allow_directory(attachments_dir, recursive=true)
│
│    ⇣ #1 与 #2 可并行
│
├── #3 Editor + Viewer + lifecycle（依赖 #1 + #2）
│    └── 安装 TipTap 依赖
│    └── apps/desktop/src/rich/extensions.ts 共享 schema（含 Link 协议白名单）
│    └── apps/desktop/src/rich/legacy.ts（normalizeLegacy / walkAttIds / rewriteLocalSrc / sanitizeHref）
│    └── apps/desktop/src/components/RichDescriptionEditor.vue（含粘贴占位 UX）
│    └── apps/desktop/src/components/RichDescriptionViewer.vue
│    └── apps/desktop/src/composables/useRichDescriptionLifecycle.ts
│
├── #4 PM / Todo 删除路径联动（依赖 #1）
│    └── pm.rs::project_delete 重写为事务 + 显式清 pm_items + delete_by_owner（修复现有孤儿 bug，见 §13.1）
│    └── pm.rs::item_delete 末尾加 delete_by_owner('pm_item')
│    └── todo.rs::delete_item_by_id 末尾加 delete_by_owner('todo')
│    └── settings.rs::action_migrate_data_dir 追加 attachments/ 递归复制
│
├── #5 三处接入（依赖 #3）
│    └── PmProjectDialog.vue：替换 textarea + 接 lifecycle
│    └── PmItemDialog.vue：同上
│    └── TodoDetailEdit.vue：同上 + 删除老 md 工具栏
│    └── TodoPanel.vue:854 focus 方式改为组件暴露 API
│    └── 详情面板接入 Viewer（PmDetailPanel / TodoDetailView 等）
│
└── #6 兼容与验证
     └── pnpm typecheck
     └── pnpm --filter @lazycat/desktop build:web
     └── pnpm dev 手测矩阵（见 §15）
```

## 15. 验证矩阵

| 场景 | 期望结果 |
|---|---|
| 空 description 打开编辑器 | 显示 placeholder，无报错 |
| 旧纯文本 description 打开编辑器 | 自动转为段落显示，编辑保存后 DB 变 JSON |
| 旧纯文本 description 打开 Viewer | 走 legacy 路径，保留换行 |
| 旧 Todo MD 描述（`**粗体**` / `# 标题`）打开 Viewer | 按纯文本段落显示，字面保留标记（接受已知退化） |
| 在编辑器粘贴 PNG 截图 | 立即出现 loading 占位；save 完成后替换为最终 src，文件写入 attachments/ |
| 粘贴超 5 MB 图 | 弹提示，不出现占位 |
| 粘贴同一张图两次 | 第二次 hash 命中，复用已有 rel_path，不重复写文件，DB 多一行引用 |
| 粘贴 SVG | save 返回 Err，占位节点被移除 |
| 链接手填 `javascript:alert(1)` | Viewer 渲染后 href 被清空；即便通过 DOM 直接触发点击也被二次拦截 |
| 新建 PM Item 后取消 | 刚粘贴的图 tmp 记录被清，物理文件按引用计数清 |
| 新建 PM Item 后提交 | rebind 后 owner_id 从 tmp-xx 变为 realId |
| 编辑态删除一张图并保存 | cleanup_orphans 清 DB 记录；物理文件按引用计数清 |
| 删除 PM Item | 该 item 所有附件被清 |
| 删除整个 PM Project | 事务内子 item 附件 → pm_items 行 → project 附件按序清，无孤儿 |
| 数据目录迁移（含 attachments） | 新目录下同时存在 `lazycat.sqlite` / `hosts-backups/` / `attachments/`，图片仍可显示 |
| 数据目录自定义到 D:\Data | assetProtocol 动态 scope 生效，图片仍可显示 |
| Viewer 渲染 1000 字 + 3 张图 | 首次渲染 <100ms（非大列表场景） |
| 外链自动识别 | 输入 `https://example.com` 空格/回车自动成为 link mark |
| 外链点击 | 走 `tool:system:open_external` → 系统浏览器，不在 webview 跳转 |
| 主题切换 | 编辑器 toolbar / Viewer 的配色跟随 theme-light |
| TodoDetailEdit 老 md 工具栏 | 完全移除，不残留按钮 |
| 构建 | `pnpm typecheck` 和 `build:web` 通过，产物体积增加可接受（TipTap 全家桶约 150 KB gzipped） |

## 16. 风险与缓解

| 风险 | 缓解 |
|---|---|
| assetProtocol scope 通配过宽被 XSS 利用 | 静态 scope 只给 `attachments/**`，不开全盘；运行时 allow_directory 也只给 attachments |
| `tauri` feature 漏开 `protocol-asset` 导致图片 404 | Cargo.toml 与 CSP 一并在 §14 #2 同步修改；在验证矩阵 "粘贴 PNG 截图" 用例中 fail-fast 暴露 |
| TipTap 3 与 StarterKit 版本兼容 | 锁版本同时升级，package.json 用 `^3.x.y` 确切次版本 |
| JSON 序列化后超长导致 SQLite 行变慢 | 现实描述内容量级可控（单条预计 <10 KB），SQLite TEXT 无上限，不成为瓶颈 |
| Link 协议被钻空（`javascript:` / `data:`） | 三层防御：Link 扩展白名单 `['http','https','mailto']` + Viewer sanitizeHref + 点击监听二次校验；`tool:system:open_external` 后端再做一次白名单 |
| 粘贴 SVG 中内嵌脚本 | `attachments:save` 显式拒绝 image/svg+xml；Image 扩展不渲染未落盘的 src |
| Viewer `v-html` 的 XSS 面 | static-renderer 只按 schema 产出，不保留未识别标签/事件属性；Link href 经 sanitize |
| base64 大图卡 IPC | 单图上限压到 5 MB；粘贴显示 loading 占位，失败回滚不卡死编辑流 |
| 数据目录迁移忘了复制 attachments | §13 + §14 #4 明确在 `settings.rs::action_migrate_data_dir` 追加复制；§15 验证矩阵覆盖 |
| 用户在旧版本数据库上跑新版本 | `ensure_schema` 用 `CREATE TABLE IF NOT EXISTS`，向前兼容 |
| 旧 PM `project_delete` 孤儿 items 未清理 | 本方案 §13.1 改为事务内显式 `DELETE FROM pm_items WHERE project_id=?`，一次性修复历史 bug |

## 17. 后续演进（P2）

- `FileAttachment` Node：基于 `@tiptap/core` 的 Node + VueNodeViewRenderer，attrs 含 `attId/kind/name/size`
- `@tiptap/extension-file-handler`：拖拽/粘贴非图片文件
- "插入本地文件"菜单：调 `@tauri-apps/plugin-dialog.open`
- 附件库抽屉：按 owner 列出所有附件，支持重命名/下载到本地
- 定时 `cleanup_stale`：清理 `tmp-*` 且 `created_at` 超过 7 天的记录
- description 影子 text 列 `description_text`：用 `editor.getText()` 维护，支撑全文搜索

## 18. 文档同步

- 实施完成后，按 `CLAUDE.md` §07.3 的阈值判断是否沉淀到 `process.md`（本任务跨前后端 >10 文件，预期会沉淀"TipTap + 附件持久化"的通用经验）
- 本文件 `2026-04-20-rich-description-tiptap-design.md` 作为实施期的唯一真相来源；实施中如有偏离，先改本文再改代码
- `CLAUDE.md` / `AGENTS.md` 暂不需要新增规则；只有在沉淀出跨任务复用的约束时再回写
