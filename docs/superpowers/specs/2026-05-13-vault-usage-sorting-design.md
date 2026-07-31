# Vault Usage Sorting Design

## 目标

统计密码库条目的查看次数与复制次数，以此作为排序依据，按使用频次从高到低排列。

## 需求摘要

- **查看**：在列表中点击眼睛图标显示密码明文，计为 1 次查看
- **复制**：点击复制按钮复制密码字段到剪贴板，计为 1 次复制
- **排序**：按 `view_count + copy_count` 合计从大到小排列；合计相同时按 `updated_at DESC` 兜底

## 数据模型变更

### vault_entries 新增列

```sql
ALTER TABLE vault_entries ADD COLUMN view_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE vault_entries ADD COLUMN copy_count INTEGER NOT NULL DEFAULT 0;
```

`ensure_schema()` 中通过 ALTER TABLE 迁移，遵循已有模式（`let _ = conn.execute_batch(...)` 静默忽略"列已存在"错误），已有行默认值为 0。

## 后端变更

### 新增 action: `record_usage`

- 通道：`tool:vault:record-usage`
- 入参：`{ id: number, type: "view" | "copy" }`
- 逻辑：
  - 校验 session key
  - `view` → `UPDATE vault_entries SET view_count = view_count + 1 WHERE id = ?`
  - `copy` → `UPDATE vault_entries SET copy_count = copy_count + 1 WHERE id = ?`
- 返回值：`{ success: true }`

### 修改 list 排序

```sql
-- 旧
ORDER BY updated_at DESC
-- 新
ORDER BY (view_count + copy_count) DESC, updated_at DESC
```

### vault::execute 内部分发

`vault::execute` 内部 match 新增 `"record_usage" => cmd_record_usage(payload)`。

注：`mod.rs` 无需修改——桥接通道 `tool:vault:record-usage` 已解析为 `{ domain: "vault", action: "record_usage" }`，会命中现有的 `"vault" => vault::execute(action, payload)` 路由。

## 前端变更

### Bridge（tauri.ts）

```typescript
"tool:vault:record-usage": { domain: "vault", action: "record_usage" }
```

### VaultPanel.vue

两处 fire-and-forget 调用点：

1. **显示密码后**（`onTogglePassword`，`revealedPasswords.set(entry.id, pw)` 之后）：调用 `invokeToolByChannel("tool:vault:record-usage", { id: entry.id, type: "view" })`。注意仅在**揭示**分支记录，隐藏分支（`revealedPasswords.delete`）不记录。
2. **复制密码后**（`onDirectCopyPassword`，`ElMessage.success("密码已复制")` 之后）：调用 `invokeToolByChannel("tool:vault:record-usage", { id: entry.id, type: "copy" })`

fire-and-forget 原因：计数不阻塞 UI，失败不影响核心操作。

## 影响文件

| 文件                                          | 改动                                                       |
| --------------------------------------------- | ---------------------------------------------------------- |
| `apps/desktop/src-tauri/src/tools/helpers.rs` | ALTER TABLE 追加两列                                       |
| `apps/desktop/src-tauri/src/tools/vault.rs`   | 新增 `cmd_record_usage` 函数及内部分发，修改 list 排序 SQL |
| `apps/desktop/src/bridge/tauri.ts`            | 新增 `tool:vault:record-usage` 通道                        |
| `apps/desktop/src/components/VaultPanel.vue`  | 两处 recordUsage 调用                                      |

## 不涉及

- 不新增 UI 元素（不展示计数）
- 不新增排序模式切换（完全替换旧排序）
- 不新增设置项
