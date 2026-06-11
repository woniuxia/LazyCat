# Spotlight 凭据账号搜索 实施计划

> 依据设计文档：`docs/superpowers/specs/2026-06-11-spotlight-vault-account-search-design.md`
> 目标：vault 存储重构（仅密码加密）+ Spotlight 按账号等字段搜索 + 「复制账号」动作

---

## 总览

| Phase | 目标 | 预估 | 关键依赖 |
|-------|------|------|---------|
| Phase 0 | 表迁移 + `split_fields`/`merge_fields` 纯函数与单测 | 0.5 天 | 无 |
| Phase 1 | 读写路径切换（create/update/get/list/reveal_one/record_usage） | 1 天 | Phase 0 |
| Phase 2 | 迁移回填（unlock + change_password 两条触达路径） | 0.5 天 | Phase 1 |
| Phase 3 | `meta_list` 扩展（plainFields 返回 + keyword 口径） | 0.25 天 | Phase 0 |
| Phase 4 | Spotlight 前端（搜索字段/副标题/复制账号 + 单测） | 1 天 | Phase 3 |
| Phase 5 | 全量验证 + 人工清单 + 提交 | 0.5 天 | Phase 1-4 |

**Phase 0 → 1 → 2 为后端主线，严格串行；Phase 3 仅依赖 Phase 0，可与 Phase 1/2 并行；Phase 4 依赖 Phase 3。**

---

## Phase 0：表迁移与纯函数

### 0.1 `plain_fields` 列迁移

**文件**：`apps/desktop/src-tauri/src/tools/helpers.rs`（紧邻 :483-486 的 `view_count`/`copy_count` 迁移先例）

```rust
let _ = conn.execute_batch("ALTER TABLE vault_entries ADD COLUMN plain_fields TEXT DEFAULT NULL;");
```

### 0.2 拆分/合成纯函数

**文件**：`apps/desktop/src-tauri/src/tools/vault.rs`（放在 `build_fields` :1089 附近）

```rust
/// 完整字段 JSON -> (加密部分, 明文部分)
/// 加密部分固定 {"password": fields["password"]}；明文部分为其余所有键。
fn split_fields(fields: &Value) -> (Value, Value)

/// blob 是否旧格式（含 password 以外的键）
fn blob_is_legacy(blob_fields: &Value) -> bool

/// 明文列 + 解密后 blob -> 完整字段 JSON
/// 旧格式 -> 直接返回 blob（忽略 plain，避免陈旧键污染）；
/// 新格式 -> plain 解析结果（None/解析失败视为 {}）+ blob 的 password。
fn merge_fields(plain_fields_text: Option<&str>, blob_fields: &Value) -> Value
```

### 0.3 单测（`#[cfg(test)] mod tests`，无 DB 依赖）

按设计文档测试计划：`test_split_fields_app/_server/_database`、`test_split_fields_empty_password`、`test_merge_fields_new_format`、`test_merge_fields_legacy_format`、`test_merge_fields_stale_plain`、`test_merge_fields_invalid_plain_text`。

### 验证

- `cargo test`（在 `apps/desktop/src-tauri` 下）新增用例全过，现有 `test_build_fields_*` 不动且通过。

---

## Phase 1：读写路径切换

全部在 `apps/desktop/src-tauri/src/tools/vault.rs`，返回 JSON 结构的既有键一律不变。

### 1.1 写路径

- **`cmd_create`（:648）**：`build_fields` 产物 → `split_fields` → 密码部分序列化加密入 `encrypted_blob`，明文部分 `serde_json::to_string` 入 `plain_fields`；INSERT 列与参数各加一项。
- **`cmd_update`（:698）**：同上，UPDATE 语句加 `plain_fields = ?`。

### 1.2 读路径

- **`cmd_get`（:610）**：SELECT 增加 `plain_fields`（`Option<String>`），解密 blob 后 `merge_fields` 合成 `fields` 返回。
- **`cmd_reveal_one`（:998）**：同上（注意保持 `key.zeroize()` 时序不变）。
- **`cmd_list`（:513）**：SELECT 增加 `plain_fields`；逐行优先走快路径——`plain_fields` 非 NULL 时解析 JSON 直接取 `account` 与 `make_summary(cat, &plain)`，**不解密**；NULL 行退回现状解密路径。`make_summary`（:491）签名与逻辑不变（所需 url/address/port 等键都在明文部分）。

### 1.3 `cmd_record_usage`（:850）

删除 `let _key = get_session_key()?;` 一行（免会话，仅递增明文计数列）。

### 验证

- `cargo test` 通过。
- 手测（dev）：新建/编辑/查看/列表/即时解锁单条全链路正常；新建条目后直接查 sqlite 确认 `plain_fields` 已写入且 blob 解密后仅含 password。

---

## Phase 2：迁移回填

**文件**：`apps/desktop/src-tauri/src/tools/vault.rs`

### 2.1 `backfill_plain_fields`

```rust
/// 扫描全部条目：解密 -> blob_is_legacy 为真（含首次迁移与降级期旧版编辑）->
/// split_fields -> 新 IV 重加密密码部分 -> UPDATE iv/encrypted_blob/plain_fields；
/// 新格式行跳过。单行失败 eprintln! 记录 id 后跳过；函数不返回 Err。
/// UPDATE 不触碰 updated_at（避免扰动「最近使用」排序）。
fn backfill_plain_fields(conn: &Connection, key: &[u8; KEY_LEN])
```

### 2.2 `cmd_unlock`（:310）接入

canary 验证通过、`derive_key` 之后，**先 `backfill_plain_fields(&conn, &key)`，再建立 `VAULT_SESSION`**（key 为 `Copy` 数组；此顺序关闭并发 IPC 经 `list` 读到混合状态的理论窗口）。

### 2.3 `cmd_change_password`（:379）触达路径 2

重加密循环：SELECT 增加 `plain_fields`；逐行解密后判 `blob_is_legacy`——旧格式走 `split_fields`，新 key 仅重加密密码部分并写 `plain_fields`；新格式按现状整体重加密（blob 本就只含 password）。

### 验证

- 构造旧格式库（用迁移前版本建几条数据，或手工 SQL 置 `plain_fields = NULL` + 旧 blob）→ 解锁一次 → 确认 `plain_fields` 回填、blob 仅含 password、`updated_at` 未变。
- 重复解锁：无重复写入（幂等）。
- 改主密码：新旧格式混合库全部条目密码可解。

---

## Phase 3：`meta_list` 扩展

**文件**：`apps/desktop/src-tauri/src/tools/vault.rs` `cmd_meta_list`（:925）

- SELECT 增加 `plain_fields`。
- 返回项追加 `"plainFields"`：`plain_fields` 文本 `serde_json::from_str` 成功 → 对象；NULL 或解析失败 → `json!(null)`。
- keyword 过滤改为 `(title LIKE ? OR IFNULL(plain_fields,'') LIKE ?)`，同一关键字推两次参数（口径一致性修补，当前调用方均不传 keyword）。

### 验证

- `cargo test`；手测 Spotlight 锁定态 prefetch 正常（plainFields 为 null 的旧行为不回归）。

---

## Phase 4：Spotlight 前端

**文件**：`apps/desktop/src/spotlight/providers/vault.ts`，新增 `apps/desktop/src/spotlight/providers/vault.test.ts`

### 4.1 类型与数据

```ts
export interface VaultPlainFields {
  account?: string; url?: string; address?: string; port?: number;
  serverType?: string; dbType?: string; dbName?: string; schema?: string; notes?: string;
}
// VaultMetaEntry 追加 plainFields?: VaultPlainFields | null;
// buildItem 的 payload 追加 account: string（plainFields?.account ?? ""）
```

### 4.2 `buildSubtitle`（:65）

改为「分类 · 环境 · 账号」，各段为空则省略；标签段移除。

### 4.3 `buildItem`（:83）搜索字段

按设计权重表追加：account 1.1、url/address/dbName/schema 0.8、serverType/dbType 0.6、notes 0.5；最终 `searchFields` 统一 `.filter((f) => f.text)` 过滤空串（对存量字段同样生效，属既定行为变更）。

`buildItem` 与 `buildSubtitle` 加 `export` 供单测。

### 4.4 「复制账号」动作

- `buildActions`（:194）在「复制密码」与「跳转到凭据工具」之间插入 `{ id: "copy_account", label: "复制账号", icon: "copy" }`（菜单当前不渲染 icon）。
- `executeAction`（:211）新增分支：
  1. `payload.account` 空串 → `{ errorMessage: "该条目没有账号（旧条目需先解锁一次完成迁移）" }`
  2. `writeSecretToClipboard(account)`，**不**调度 `scheduleClipboardClear`
  3. `recordCopy(entryId)`（Phase 1.3 后锁定态也能落库）
  4. `{ closeSpotlight: true, toast: { message: "账号已复制到剪贴板", type: "success" } }`

### 4.5 单测 `vault.test.ts`（vitest 收集 `src/**/*.test.ts`，`src/spotlight/` 已有先例）

按设计文档 TS 测试计划 5 条：account 权重与拼音首字母、新字段权重与空串过滤、plainFields=null 时字段集合与现状一致（空串过滤除外）、副标题三段式、payload.account 透传。

### 验证

- `pnpm test`、`pnpm typecheck`、`pnpm --filter @lazycat/desktop build:web`。

---

## Phase 5：收尾

### 5.1 全量验证

1. `cargo test`（src-tauri）
2. `pnpm test`
3. `pnpm typecheck`
4. `pnpm --filter @lazycat/desktop build:web`

### 5.2 人工验证清单（按设计文档「验证」节）

1. 旧库升级：解锁前按账号不可搜 → 主面板解锁一次 → 锁定 vault → 按账号可搜、副标题显示账号
2. 新建/编辑条目后立即按账号可搜（无需重新解锁）
3. 锁定态 Enter 复制密码仍弹主密码输入条；解锁态直接复制并 30 秒清空
4. Tab「复制账号」锁定/解锁态均免主密码；无账号条目报提示文案
5. 改主密码后全部条目密码可解、未迁移行被顺手迁移
6. 拼音首字母搜账号（如账号含中文备注名场景）
7. 回归：Vault 主面板增删改查、标签、即时解锁单条均正常

### 5.3 提交规范

建议两个提交（后端先行，前端随后）：

- `feat(vault): 存储重构为仅密码加密，新增明文 plain_fields 列与解锁回填迁移`
- `feat(spotlight): 凭据支持按账号等字段搜索，新增复制账号动作`

### 5.4 经验沉淀

涉及 3+ 文件的复杂任务，完成后按 CLAUDE.md `07.3` 评估记录 `process.md`（候选经验：加密数据格式演进的「统一读取函数 + 解锁时机回填」模式）。

---

## 风险与回退

| 风险 | 触发条件 | 回退策略 |
|------|---------|---------|
| 回填中断（崩溃/断电） | unlock 同步回填执行中 | 单行 UPDATE 原子；未完成行下次解锁重试（幂等判定为 blob 含非密码键） |
| e2e/测试桩依赖旧 vault 行为 | `pnpm test:e2e` 或 fixture 中有 vault 流程 | 开工时先 grep e2e 目录确认；如有则同步更新桩数据 |
| `record_usage` 免会话引发评审顾虑 | 安全审视 | 仅递增明文计数列，与 `meta_list` 免会话同口径（设计已确认） |
| 降级后旧版编辑产生陈旧 plain_fields | 用户降级又升级 | `merge_fields` 旧格式以 blob 为准 + 回填自愈（设计已覆盖，残留窗口仅限 list 摘要展示） |
| Windows 下 `cargo test` 与运行中 exe 文件锁冲突 | dev 实例未关闭 | 测试前结束 LazyCat 进程（CLAUDE.md `01.2`） |

## 下一步

按 Phase 0 → 1 → 2 → 3 → 4 → 5 推进，每个 Phase 结束做最小验证再进入下一个；Phase 3 可视进度与 Phase 1/2 并行。
