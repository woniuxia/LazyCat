# 收纳箱设计与实现计划

日期：2026-03-16

## 1. 背景与复核结论

LazyCat 当前已经有：

- `todo`：任务执行与提醒
- `vault`：敏感信息保管
- `launcher`：外部动作入口
- `useClipboardSuggestion`：窗口获焦时的轻量剪贴板识别与工具推荐

但仍缺少一个统一的“先收进来，再决定怎么处理”的入口。继续把“个人事务管理”做深，最自然的补位不是再加一个孤立面板，而是增加一个中转层：

- 先自动接住最近复制过的内容
- 再把有价值的内容升格为长期保存的条目
- 最后从这里继续分发到 `todo`、`vault`、后续便签等模块

本次方案将“个人收件箱 / 稍后处理箱”和“剪贴板历史 / 临时中转站”合并为一个新功能：`收纳箱`。

复核后的关键结论如下：

- 功能方向成立，且与现有 `todo / vault / launcher` 能形成闭环
- 交互上必须明确区分 `历史流` 和 `收件箱` 的寿命与操作权限
- 后台持续记录只在 LazyCat 进程存活时生效，不做系统级常驻服务
- 大文本、大图片、大二进制不能默认完整回放，必须使用分级存储与延迟加载
- 跨工具动作统一走“打开预填草稿”，不做静默直接写入，避免误操作和数据污染

## 2. 产品定义

### 2.1 功能名称

`收纳箱`

### 2.2 核心定位

一个面向个人事务管理的“后台剪贴板收件箱”。

它包含两层：

- `历史流`：自动记录最近剪贴板变化，偏短期、可淘汰
- `收件箱`：把重要内容升格为长期保存、可整理、可继续处理的项目

### 2.3 解决的问题

- 复制过的内容容易丢，之后想再找只能重做
- 复制过来的文本、图片、文件引用缺少统一整理入口
- 当前 `todo`、`vault` 等模块只能接收“已经决定好怎么处理”的输入，缺少一个上游缓冲区

## 3. 范围与非目标

### 3.1 第一版支持范围

第一版支持的“全量类型”按常见 Windows 剪贴板类型解释，不承诺保存任意私有二进制格式。明确支持：

- 纯文本
- HTML
- RTF
- URL / URI 文本
- 文件 / 文件夹引用
- 位图图片

对于未知或私有格式：

- 尽量记录格式元数据
- 不默认保存原始二进制
- 在 UI 上明确标识为“仅元数据”

### 3.2 第一版不做

- OCR
- 文件全文索引
- 图片内容识别
- 任意未知二进制格式完整保存
- 系统级常驻后台服务
- 便签工具本体实现

## 4. 信息架构与交互逻辑

### 4.1 面板结构

新增 `InboxPanel.vue`，固定三栏结构：

- 左栏：分区与筛选
- 中栏：摘要列表
- 右栏：详情与动作

左栏固定包含：

- 历史流
- 收件箱
- 已归档
- 类型筛选
- 星标筛选
- 仅显示外部存储
- 仅显示仅摘要

### 4.2 历史流与收件箱的区别

`历史流`：

- 由后台自动采集生成
- 默认按保留规则自动清理
- 可删除、复制、升格、转发
- 不要求用户补充标题和备注

`收件箱`：

- 由历史项升格或未来手动创建产生
- 默认长期保留，不受按天数清理影响
- 允许编辑标题、备注、星标、归档
- 是后续进入 `todo / vault / 便签` 的主入口

### 4.3 核心交互规则

1. 用户复制内容后，后台监控检测到变化并入 `历史流`
2. 中栏列表只展示摘要，不展示完整正文
3. 用户点击某条摘要后，右栏才按需加载详情
4. 用户点击“转入收件箱”时，不复制新记录，只把当前记录从 `history` 升格为 `inbox`
5. 用户点击“转任务清单”“存入密码库”等跨工具动作时，统一打开目标工具的预填草稿，由用户确认后保存
6. 用户将收件箱条目标记归档后，条目进入 `archived` 分区，不再出现在默认收件箱列表

### 4.4 列表交互

中栏摘要卡片固定展示：

- 类型图标
- 标题或自动生成标题
- 单行预览
- 捕获时间
- 大小
- 命中次数
- 是否已升格
- 存储模式徽标：`内联` / `外部存储` / `仅摘要` / `仅元数据`

列表默认规则：

- 默认按 `last_seen_at DESC`
- 分页加载，每页 `50` 条
- 仅保留最近 `100` 条已渲染摘要节点，避免前端 DOM 堆积

### 4.5 详情交互

右栏详情按类型展示：

文本类：

- 标题
- 多行预览或完整正文
- 原始大小
- 存储方式
- 内容来源与时间

图片类：

- 缩略图
- 图片尺寸
- 文件大小
- 是否保留原图

文件引用类：

- 文件名或文件夹名
- 原始路径
- 大小
- 修改时间
- 打开位置按钮

未知格式：

- 格式标识
- 基础元数据
- “该格式未持久化原始内容”的明确提示

### 4.6 跨工具动作

本轮只定义交互，不做静默写入：

- `转任务清单`：打开 `todo` 创建态草稿，标题与正文预填
- `存入密码库`：打开 `vault` 创建态草稿，文本预填到备注或待解析字段
- `转便签`：预留动作与数据协议，不在本轮真正落库

统一原则：

- 所有跨工具动作都必须经过用户确认
- 收纳箱不直接替用户落业务数据

## 5. 后台采集模型

### 5.1 生效时机

后台持续记录的定义固定为：

- LazyCat 进程存活时持续记录
- 窗口隐藏到托盘后继续记录
- 退出应用后停止记录

不做系统服务，不在操作系统启动后独立常驻。

### 5.2 检测方式

后端在 `main.rs` 启动独立线程，使用 Win32 剪贴板变更序号检测。

复用当前 `hotkey.rs` 已经使用的 `GetClipboardSequenceNumber` 思路：

- 轮询间隔固定 `700ms`
- 序号未变化时，不打开剪贴板，不做解析
- 序号变化时，才进入一次实际读取

**与现有 `useClipboardSuggestion` 的协调方案**：

采用**方案 A：统一到 Rust 后台**

- 当前 `useClipboardSuggestion.ts` 没有独立轮询，只在 `App.vue` 窗口获焦时调用 `detectClipboard()`
- 收纳箱后台线程负责所有剪贴板监控
- 通过 Tauri event `clipboard-changed` 向前端推送变化
- `useClipboardSuggestion` 改为订阅后台事件，接收推送后执行工具推荐逻辑
- 移除 `App.vue` 中窗口获焦时的 `detectClipboard()` 调用

实现细节：

```rust
// main.rs 中启动监控线程
fn start_clipboard_monitor(app_handle: AppHandle) {
    static CLIPBOARD_MONITOR_RUNNING: AtomicBool = AtomicBool::new(false);

    if CLIPBOARD_MONITOR_RUNNING.swap(true, Ordering::SeqCst) {
        return; // 已在运行
    }

    std::thread::spawn(move || {
        let mut last_seq = unsafe { GetClipboardSequenceNumber() };

        while CLIPBOARD_MONITOR_RUNNING.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(700));

            let current_seq = unsafe { GetClipboardSequenceNumber() };
            if current_seq != last_seq {
                last_seq = current_seq;

                // 1. 持久化到收纳箱（inbox.rs）
                if let Err(e) = process_clipboard_change(&app_handle) {
                    eprintln!("clipboard monitor error: {e}");
                }

                // 2. 推送事件给前端（用于工具推荐）
                let _ = app_handle.emit("clipboard-changed", ());
            }
        }
    });
}
```

```typescript
// useClipboardSuggestion.ts 改为事件订阅模式
import { listen } from '@tauri-apps/api/event'

export function useClipboardSuggestion() {
  onMounted(async () => {
    // 订阅后台推送的剪贴板变化事件
    const unlisten = await listen('clipboard-changed', async () => {
      const text = await navigator.clipboard.readText()
      if (text && text !== lastClipboardText.value) {
        lastClipboardText.value = text
        // 执行工具推荐逻辑
        detectAndSuggest(text)
      }
    })

    onUnmounted(() => {
      unlisten()
    })
  })
}
```

优点：
- 统一管理，避免前后端重复监控
- 窗口失焦时也能持续记录
- 前端只需订阅事件，无需轮询

优化点：
- 需要重构现有 `useClipboardSuggestion.ts`（共 95 行，暴露 6 个 API）
  - 仅改检测触发方式（从"窗口获焦手动调用"改为"后台事件订阅"），不改 API 签名
  - `watchPendingInput` 被 6 个面板使用（BcryptPanel、EncodePanel、FormatterPanel、JwtPanel、JsonProcessPanel、TimestampPanel）
  - 需要回归验证这 6 个面板的预填行为
- 需要修改 `App.vue`，移除窗口获焦时的手动调用（第 455 行）

### 5.3 去重规则

连续相同内容合并，不新增重复记录。

规则固定为：

- 比较 `item_type + normalized_hash`
- 在 `30 秒` 内再次出现时，只更新 `last_seen_at` 和 `seen_count`
- 超出 `30 秒` 视为一次新捕获

### 5.4 暂停与开关

新增设置项：

- 启用收纳箱后台采集
- 保留天数
- 暂停采集 5 分钟
- 托盘运行时继续采集

首次启用时展示一次隐私说明，说明：

- 历史流会记录最近复制内容
- 可以随时关闭或暂停
- 敏感内容默认有抑制规则

## 6. 数据模型与接口

### 6.1 数据表

追加下一号 migration。

当前 `helpers.rs` 的最新 migration 为 25，因此本功能预计新增 migration 26。

新增主表：`inbox_items`

核心字段：

- `id` INTEGER PRIMARY KEY
- `bucket` TEXT NOT NULL：`history | inbox | archived`
- `item_type` TEXT NOT NULL：`text | html | rtf | image | file | unknown`
- `storage_kind` TEXT NOT NULL：`inline | external | metadata_only`
- `title` TEXT
- `preview` TEXT
- `search_text` TEXT
- `payload_ref` TEXT：外部文件路径或内联内容
- `byte_size` INTEGER NOT NULL
- `content_hash` TEXT NOT NULL
- `captured_at` TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
- `last_seen_at` TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
- `seen_count` INTEGER NOT NULL DEFAULT 1
- `note` TEXT
- `starred` INTEGER NOT NULL DEFAULT 0
- `meta_json` TEXT

新增从表：`inbox_file_refs`

用途：

- 保存文件 / 文件夹 / URI 列表引用
- 不复制文件本体

字段：

- `id` INTEGER PRIMARY KEY
- `inbox_item_id` INTEGER NOT NULL
- `file_path` TEXT NOT NULL
- `file_name` TEXT NOT NULL
- `file_size` INTEGER
- `modified_at` TEXT
- FOREIGN KEY (inbox_item_id) REFERENCES inbox_items(id) ON DELETE CASCADE

**新增引用计数表：`inbox_asset_refs`**

用途：

- 管理外部文件的引用计数
- 多个记录可能引用同一文件（通过 hash 去重）
- 删除记录时减少引用计数，为 0 时删除文件

字段：

- `content_hash` TEXT PRIMARY KEY
- `file_path` TEXT NOT NULL：相对于数据目录的路径
- `ref_count` INTEGER NOT NULL DEFAULT 1
- `byte_size` INTEGER NOT NULL
- `created_at` TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP

外部文件路径规范：

- 根目录：`<data_dir>/inbox-assets/`
- 二级目录结构：`<data_dir>/inbox-assets/<hash前2位>/<完整hash>`
- 示例：`inbox-assets/a3/a3f2b8c9d1e4f5...`（避免单目录文件过多）

新增虚表：`inbox_fts`

索引字段：

- `title`
- `preview`
- `note`
- `search_text`

完整 SQL：

```sql
CREATE TABLE inbox_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bucket TEXT NOT NULL CHECK(bucket IN ('history', 'inbox', 'archived')),
    item_type TEXT NOT NULL,
    storage_kind TEXT NOT NULL CHECK(storage_kind IN ('inline', 'external', 'metadata_only')),
    title TEXT,
    preview TEXT,
    search_text TEXT,
    payload_ref TEXT,
    byte_size INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    captured_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    seen_count INTEGER NOT NULL DEFAULT 1,
    note TEXT,
    starred INTEGER NOT NULL DEFAULT 0,
    meta_json TEXT
);

CREATE INDEX idx_inbox_items_bucket ON inbox_items(bucket);
CREATE INDEX idx_inbox_items_hash ON inbox_items(content_hash);
CREATE INDEX idx_inbox_items_captured ON inbox_items(captured_at);

CREATE TABLE inbox_file_refs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    inbox_item_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_size INTEGER,
    modified_at TEXT,
    FOREIGN KEY (inbox_item_id) REFERENCES inbox_items(id) ON DELETE CASCADE
);

CREATE TABLE inbox_asset_refs (
    content_hash TEXT PRIMARY KEY,
    file_path TEXT NOT NULL,
    ref_count INTEGER NOT NULL DEFAULT 1,
    byte_size INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE VIRTUAL TABLE inbox_fts USING fts5(
    title,
    preview,
    note,
    search_text,
    content='inbox_items',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);

-- FTS 同步触发器
CREATE TRIGGER inbox_fts_insert AFTER INSERT ON inbox_items BEGIN
    INSERT INTO inbox_fts(rowid, title, preview, note, search_text)
    VALUES (new.id, new.title, new.preview, new.note, new.search_text);
END;

CREATE TRIGGER inbox_fts_update AFTER UPDATE ON inbox_items BEGIN
    INSERT INTO inbox_fts(inbox_fts, rowid, title, preview, note, search_text)
    VALUES('delete', old.id, old.title, old.preview, old.note, old.search_text);
    INSERT INTO inbox_fts(rowid, title, preview, note, search_text)
    VALUES(new.id, new.title, new.preview, new.note, new.search_text);
END;

CREATE TRIGGER inbox_fts_delete AFTER DELETE ON inbox_items BEGIN
    INSERT INTO inbox_fts(inbox_fts, rowid, title, preview, note, search_text)
    VALUES('delete', old.id, old.title, old.preview, old.note, old.search_text);
END;
```

### 6.2 存储模式

`inline`：

- 内容直接存入 SQLite
- 适合中小文本

`external`：

- 正文或图片存到数据目录外部文件
- SQLite 只保存摘要和引用路径

`metadata_only`：

- 只保留摘要、大小、hash、类型和必要元数据
- 不支持完整回放

### 6.3 前端类型

新增：

- `InboxItemSummary`
- `InboxItemDetail`
- `InboxListQuery`
- `InboxItemType`
- `InboxStorageKind`
- `InboxBucket`

建议放置：

- `apps/desktop/src/types/inbox.ts`

### 6.4 IPC 通道

新增 `tool:inbox:*` 域：

- `tool:inbox:list`
- `tool:inbox:get`
- `tool:inbox:search`
- `tool:inbox:promote`
- `tool:inbox:update-meta`
- `tool:inbox:archive`
- `tool:inbox:delete`
- `tool:inbox:cleanup`
- `tool:inbox:capture-status`
- `tool:inbox:capture-pause`

## 7. 性能与隐私边界

### 7.1 文本阈值

文本类内容的分层规则固定为：

- `<= 256KB`：直接内联存储
- `256KB ~ 8MB`：写外部文件，数据库只存摘要与引用
- `> 8MB`：降级为 `metadata_only`，只保留前后各 `2KB` 摘要、总长度和 hash

HTML / RTF 同样遵守该规则，并额外生成纯文本 `search_text`。

### 7.2 图片阈值

图片类规则固定为：

- 列表只读缩略图
- 长边统一生成 `320px` 缩略图
- 原图仅在 `<= 10MB` 且像素不超过 `1200 万` 时保留
- 超限图片自动降级，只保留缩略图和元数据

### 7.3 文件引用规则

文件类仅保存：

- 路径
- 名称
- 扩展名
- 大小
- 最后修改时间

不复制文件内容，不读取全文，不生成全文索引。

### 7.4 列表加载规则

列表接口永远只返回摘要：

- 不返回正文
- 不返回原图
- 不返回大块 blob

详情区选中后再按需请求原始内容或外部资产。

### 7.5 历史保留与硬上限

用户选择的主规则为“按天数保留”，默认值定为 `14 天`。

同时加两条硬上限，避免数据库和资产目录无限膨胀：

- 资产目录总量超过 `1GB` 时，从最旧 `history` 项开始裁剪
- `history` 项超过 `10000` 条时，从最旧项开始裁剪

`inbox` 和 `archived` 默认不参与按天数清理。

### 7.6 敏感内容抑制

`vault` 复制出的敏感内容默认不回流进收纳箱。

实现原则：

- `vault` 在复制账号或密码时写入一次性抑制指纹
- 后台采集线程在 `10 秒` 内忽略完全匹配的下一次内容

这样既不破坏 `vault` 现有复制逻辑，也避免用户刚复制密码就被历史流再次保存。

**实现细节**：

由于 vault 的复制操作在前端（`navigator.clipboard.writeText`），需要新增 Tauri command 供前端调用。

后端实现（main.rs 或 helpers.rs）：

```rust
use std::sync::{Arc, Mutex};
use std::time::{Instant, Duration};

struct SuppressedClipboard {
    content_hash: String,
    expires_at: Instant,
}

static SUPPRESSED_CLIPBOARD: LazyLock<Arc<Mutex<Vec<SuppressedClipboard>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(Vec::new())));

#[tauri::command]
fn suppress_clipboard_capture(content: String) -> Result<(), String> {
    let hash = blake3::hash(content.as_bytes()).to_hex().to_string();
    let expires_at = Instant::now() + Duration::from_secs(10);

    if let Ok(mut list) = SUPPRESSED_CLIPBOARD.lock() {
        // 清理过期项
        list.retain(|item| item.expires_at > Instant::now());

        // 限制列表长度，避免内存泄漏
        if list.len() >= 100 {
            list.remove(0);
        }

        // 添加新抑制项
        list.push(SuppressedClipboard { content_hash: hash, expires_at });
    }

    Ok(())
}

// inbox.rs 监控线程中检查
pub fn should_suppress_capture(content: &str) -> bool {
    let hash = blake3::hash(content.as_bytes()).to_hex().to_string();

    if let Ok(mut list) = SUPPRESSED_CLIPBOARD.try_lock() {
        // 清理过期项
        list.retain(|item| item.expires_at > Instant::now());

        // 检查是否在抑制列表中
        if let Some(pos) = list.iter().position(|item| item.content_hash == hash) {
            list.remove(pos); // 一次性消费
            return true;
        }
    }

    false
}
```

前端调用（VaultPanel.vue）：

> **注意**：以下为示例代码。实际 VaultPanel.vue 的复制逻辑是内联的（非独立函数），包含先通过 IPC 获取密码明文、复制后启动 30 秒定时器自动清空剪贴板等逻辑。实施时需注入到 `navigator.clipboard.writeText(pw)` **之前**（VaultPanel.vue:817），且不能干扰后续的 30 秒自动清空逻辑。

```typescript
import { invoke } from '@tauri-apps/api/core'

async function copyPassword(password: string) {
    // 先调用抑制接口
    await invoke('suppress_clipboard_capture', { content: password })

    // 再复制到剪贴板
    await navigator.clipboard.writeText(password)

    ElMessage.success('已复制到剪贴板')
}

async function copyAccount(account: string) {
    // 账号也需要抑制
    await invoke('suppress_clipboard_capture', { content: account })
    await navigator.clipboard.writeText(account)
    ElMessage.success('已复制到剪贴板')
}
```

调用时机：

- 密码库中点击"复制密码"按钮时
- 密码库中点击"复制账号"按钮时
- 密码库中点击"复制备注"按钮时（如果备注包含敏感信息）

边界条件处理：

- 使用 `Instant` 而非 `SystemTime`（不受系统时钟调整影响）
- 抑制列表最多 100 项，超出时删除最旧的
- 锁策略：
  - 监控线程（`should_suppress_capture`）使用 `try_lock()`，锁冲突时跳过本次检查，避免阻塞
  - Tauri command 端（`suppress_clipboard_capture`）使用 `.lock()`，可以等待
- 抑制项一次性消费（匹配后立即删除），避免同一密码多次复制时只有第一次被抑制

## 8. 文件落点

本功能的主要改动文件预期如下：

**后端（Rust）**：
- `apps/desktop/src-tauri/src/main.rs`（监控线程 + suppress command）
- `apps/desktop/src-tauri/src/tools/mod.rs`（域注册）
- `apps/desktop/src-tauri/src/tools/helpers.rs`（migration 26，当前最新为 25）
- `apps/desktop/src-tauri/src/tools/inbox.rs`（核心逻辑，新建）
- `apps/desktop/src-tauri/Cargo.toml`（新增 blake3 和 walkdir）

**前端（Vue + TypeScript）**：
- `apps/desktop/src/types/inbox.ts`（类型定义，新建）
- `apps/desktop/src/components/InboxPanel.vue`（主面板，新建）
- `apps/desktop/src/bridge/tauri.ts`（通道映射）
- `apps/desktop/src/tool-registry.ts`（组件注册）
- `apps/desktop/src/App.vue`（侧边栏入口 + 移除第 455 行的 detectClipboard 调用）
- `apps/desktop/src/composables/useClipboardSuggestion.ts`（重构为事件订阅）
- `apps/desktop/src/components/VaultPanel.vue`（抑制集成）
- `apps/desktop/src/components/SettingsPanel.vue`（设置项）

**参考文件**：
- `apps/desktop/src/components/CapturePanel.vue`（虚拟滚动实现参考）
- `apps/desktop/src-tauri/src/tools/hotkey.rs`（GetClipboardSequenceNumber 使用参考）

## 9. 实现策略

**一次性完整实现**，不分批次。

实施顺序建议：

1. **数据层**（helpers.rs）
   - 新增 migration 26
   - 创建 `inbox_items`、`inbox_file_refs`、`inbox_asset_refs`
   - 创建 `inbox_fts` 及同步触发器

2. **后端核心逻辑**（inbox.rs）
   - 实现分级存储逻辑（inline / external / metadata_only）
   - 实现引用计数管理（新增/删除时维护 `inbox_asset_refs`）
   - 实现图片缩略图生成（引入 `image` crate）
   - 实现清理策略（按天数、按数量、按大小、孤儿文件）
   - 实现 IPC action：list / get / search / promote / update-meta / archive / delete / cleanup

3. **后台监控线程**（main.rs）
   - 启动剪贴板监控线程（`GetClipboardSequenceNumber` 轮询）
   - 实现内容读取、归一化、去重和入库
   - 实现敏感内容抑制机制（全局 `SUPPRESSED_CLIPBOARD` 列表）
   - 新增 Tauri command：`suppress_clipboard_capture`
   - 推送 `clipboard-changed` 事件给前端

4. **工具域注册**（mod.rs + tauri.ts）
   - 在 `tools/mod.rs` 注册 `inbox` 域
   - 在 `bridge/tauri.ts` 增加 `tool:inbox:*` 通道
   - **说明**：inbox 域的 `capture-status` / `capture-pause` 等 action 通过全局 `AtomicBool` / `Mutex` 控制监控线程状态，不需要走 `execute_tool_with_app`（不需要 AppHandle）

5. **前端面板**（InboxPanel.vue + inbox.ts）
   - 实现三栏布局（左：筛选，中：摘要列表，右：详情）
   - 实现虚拟滚动（复用 `CapturePanel.vue` 的自定义虚拟滚动实现，见注释 "Virtual scroll"）
   - 实现分页加载（每页 50 条，使用 `IntersectionObserver` 监听滚动到底部）
   - 实现详情懒加载（点击后才请求完整内容）
   - 实现升格、归档、删除、星标等操作
   - **前置任务**：扩展 TodoPanel 和 VaultPanel 的外部预填支持
     - TodoPanel 需补上 `watchPendingInput` 支持（参考 `JwtPanel.vue:86-87` 的模式）
     - VaultPanel 需补上 `watchPendingInput` 支持
     - 复用 `useClipboardSuggestion.ts` 的 `applyAction` + `watchPendingInput` 模式
   - 实现跨工具草稿传递（转任务清单、存入密码库）

6. **前端集成**（App.vue + tool-registry.ts + useClipboardSuggestion.ts）
   - 在 `tool-registry.ts` 注册 `InboxPanel`
   - 在 `App.vue` 的 `sidebarItems` 中"更多工具"分组（id='more'）加入收纳箱入口
   - 在 `App.vue` 移除窗口获焦时的 `detectClipboard()` 调用（第 455 行）
   - 重构 `useClipboardSuggestion.ts`：改为订阅 `clipboard-changed` 事件（当前无独立轮询）

7. **Vault 集成**（VaultPanel.vue）
   - 在复制密码/账号时调用 `suppress_clipboard_capture`

8. **设置项**（SettingsPanel.vue）
   - 启用收纳箱后台采集（默认开启）
   - 历史保留天数（默认 14 天）
   - 暂停采集 5 分钟
   - 托盘运行时继续采集（默认开启）
   - 首次启用时展示隐私说明

9. **验证与测试**
   - 类型检查：`pnpm typecheck`
   - 构建验证：`pnpm --filter @lazycat/desktop build:web`
   - 功能测试：复制文本/图片/文件，检查历史流、升格、归档、搜索
   - 性能测试：5000 条记录下的列表滚动和搜索
   - 边界测试：超大文本（100MB）、超大图片（50MB）、文件引用失效
   - 一致性测试：删除记录后检查外部文件是否正确清理

关键依赖（Cargo.toml）：

```toml
[dependencies]
# 现有依赖
rusqlite = { version = "0.32", features = ["bundled"] }  # 当前版本
serde_json = "1.0"
image = { version = "0.25", default-features = true }    # 已存在

# 新增依赖
blake3 = "1.5"      # 快速 hash
walkdir = "2.4"     # 目录遍历（清理孤儿文件）
```

**注意**：`image` crate 已在项目中（v0.25），无需新增。

## 10. 验收标准

功能验收必须满足：

- 复制文本、图片、文件引用后，应用运行态下可在 `1 秒` 左右进入历史流
- 打开收纳箱默认只请求一页摘要，不全量拉取历史
- `5000` 条历史摘要下首次打开面板不明显卡顿
- `1MB` 文本复制后主界面不阻塞
- `> 8MB` 文本自动进入“仅摘要”模式
- 文件引用失效时只影响当前详情，不拖垮整个列表
- `vault` 刚复制出的敏感内容不会被历史流记录
- 历史项升格为收件箱后，不再受按天数清理影响

## 11. 默认值与固定假设

默认值：

- 后台持续记录：开启
- 历史保留：14 天
- 托盘运行时继续采集：开启
- 暂停采集快捷动作：5 分钟
- 分页大小：50
- 虚拟滚动 DOM 节点上限：100

固定假设：

- 当前只支持 Windows
- “全量类型”指常见 Windows 剪贴板格式集合，不等于任何私有格式都完整持久化
- 不做系统服务
- 不做 OCR
- 不做文件全文索引
- 虚拟滚动复用 `CapturePanel.vue` 的自定义实现
- 剪贴板监控复用 `hotkey.rs` 的 `GetClipboardSequenceNumber()` 机制

## 12. 实现前置检查

在开始实现前，需要确认以下事项：

1. **TodoPanel 外部预填支持**：已确认不支持，需要先扩展（参考第 9 节步骤 5 的前置任务）
2. **VaultPanel 外部预填支持**：已确认不支持，需要先扩展（参考第 9 节步骤 5 的前置任务）
3. **剪贴板格式处理**：确认 Windows API 读取 CF_HTML、CF_HDROP 等格式的具体实现方式
4. **图片格式支持**：确认 `image` crate v0.25 默认支持的格式列表（PNG/JPG/BMP/GIF/WebP）

TodoPanel 和 VaultPanel 需要先扩展 `watchPendingInput` 支持，复用 `useClipboardSuggestion.ts` 的 `applyAction` + `watchPendingInput` 模式（参考 BcryptPanel、EncodePanel、FormatterPanel、JwtPanel、JsonProcessPanel、TimestampPanel 的实现）。

