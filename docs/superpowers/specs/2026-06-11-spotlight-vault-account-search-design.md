# Spotlight 凭据账号搜索：仅密码加密的存储重构

> 关联：`2026-05-17-spotlight-vNext-restructure-design.md`、`2026-03-20-vault-unlock-smoothness-design.md`、`2026-05-13-vault-usage-sorting-design.md`
> 主轴：让 Spotlight 能按**账号等非密码字段**搜索凭据条目（锁定状态也可搜），并在结果中展示账号、支持「复制账号」动作。为此把 vault 存储模型重构为「**只有密码加密**」：`encrypted_blob` 仅存 `{"password": ...}`，其余字段移入新增明文列 `plain_fields`。存量数据在首次解锁时静默回填迁移。

## 概述

当前 Spotlight 的凭据搜索源只能按 标题/标签/分类/环境 匹配（这些是 `vault_entries` 表的明文列），而账号（account）、地址、库名、备注等字段与密码一起加密在 `encrypted_blob` 中，锁定状态下后端无法解密，Spotlight 无法按账号定位条目。典型痛点：多个条目同名（如三个「生产数据库」），只记得账号 `root@10.x` 时搜不到目标。

本设计经用户确认采用**存储重构**方案：加密块只保留密码，其余字段（含备注）全部明文化并纳入搜索索引。用户已明确接受相应的安全权衡（见「安全声明」）。

## 目标 / 非目标

### 目标

1. Spotlight 凭据搜索源支持按 账号/URL/地址/库名/schema/服务器类型/数据库类型/备注 模糊搜索（含拼音首字母），**锁定状态同样可搜**。
2. 结果条目副标题展示账号：「分类 · 环境 · 账号」，同名条目一眼可区分。
3. Tab 动作菜单新增「复制账号」：无需主密码，直接复制；Enter 默认动作（复制密码，需解锁）保持不变。
4. 存量条目在升级后**首次解锁**（主面板或 Spotlight 任一入口）时静默回填迁移，迁移失败不阻断解锁。
5. 所有 IPC 通道返回结构对前端保持兼容：`VaultPanel.vue` 零改动。

### 非目标 / YAGNI

- 不改变密码字段的加密方案（AES-256-CBC + PBKDF2 不动）。
- 不在锁定状态下改变 `VaultPanel` 行为（锁定时主面板仍显示解锁界面，不利用明文新能力展示列表）。
- 不为账号复制调度 30 秒剪贴板自动清空（账号已是明文索引，不按密级处理；密码复制的清空逻辑不变）。
- 不支持「锁定时可搜但展示脱敏」的折中形态（用户已选择完整明文）。
- 不新增设置项/开关：明文化是存储模型变更，不做按用户配置的双模式维护。
- 端口（port）不参与搜索（纯数字匹配噪音大于价值），但随 `plain_fields` 存储。
- vault-tag 关键字路径（`keyword-resolver.ts` 的 `produceVaultTag`，独立调用 `meta-list` 自建条目）本期不扩展账号搜索与账号展示，保持现状。

## 现状回顾

### 后端（`apps/desktop/src-tauri/src/tools/vault.rs`）

- 字段构造 `build_fields()`（:1089）：按分类产出完整字段 JSON（app: url/account/password/notes；server: address/serverType/account/password/notes；database: dbType/address/port/account/password/schema/dbName/notes），整体加密进 `encrypted_blob`。
- `cmd_list`（:513）：要求会话，逐行解密提取 `account` 与 `make_summary()` 摘要——重构后这两者可直接从明文列读取。
- `cmd_get`（:610）/ `cmd_reveal_one`（:998）：解密 blob 返回完整 `fields`。
- `cmd_create`（:648）/ `cmd_update`（:698）：`build_fields` → 整体加密入库。
- `cmd_change_password`（:379）：事务内逐行解密旧 key → 新 key 重加密。
- `cmd_meta_list`（:925）：**不要求会话**，返回明文元数据（id/category/title/environment/计数/tags），是 Spotlight 锁定态检索的数据源。
- `cmd_unlock`（:310）：canary 验证通过后建立 `VAULT_SESSION`。

### 表结构（`apps/desktop/src-tauri/src/tools/helpers.rs`）

- `vault_entries`（:203-232 区域建表）：`id/category/title/environment/iv/encrypted_blob/view_count/copy_count/created_at/updated_at`。
- 既有列迁移先例（:483-486）：`let _ = conn.execute_batch("ALTER TABLE vault_entries ADD COLUMN view_count ...")` 忽略「列已存在」错误，幂等。

### Spotlight 前端（`apps/desktop/src/spotlight/providers/vault.ts`）

- `prefetchVault()`（:114）：`tool:vault:status` + `tool:vault:meta-list` 构建条目。
- `buildItem()`（:83）：searchFields = 标题 1.2 / 标签 1.0 / 分类 0.6 / 环境 0.7；副标题 = 「分类 · 环境 · #标签」（:65 `buildSubtitle`）。
- `copyPasswordFlow()`（:139）：状态检查 → 未解锁走 `ctx.ensureVaultUnlocked()` → `tool:vault:get` 解密 → `writeSecretToClipboard` + 30 秒清空 → `record-usage`。
- 动作菜单（:194 `buildActions`）：复制密码 / 跳转到凭据工具。
- vitest 收集 `src/**/*.test.ts`，`src/spotlight/` 下已有测试先例（`config-store.test.ts` 等）。

## 存储模型变更

### 表结构

`helpers.rs` 紧邻 view_count/copy_count 迁移处追加：

```rust
let _ = conn.execute_batch("ALTER TABLE vault_entries ADD COLUMN plain_fields TEXT DEFAULT NULL;");
```

`plain_fields`：JSON 文本，存该条目除 `password` 外的全部字段（键名与 `build_fields` 产物一致：account/url/address/port/serverType/dbType/dbName/schema/notes，按分类取子集）。

### 新旧格式判定与统一读写

| 状态             | `plain_fields`  | `encrypted_blob` 解密后      |
| ---------------- | --------------- | ---------------------------- |
| 旧格式（未迁移） | `NULL`          | 完整字段 JSON（含 password） |
| 新格式           | 非密码字段 JSON | `{"password": "..."}`        |

`vault.rs` 新增两个纯函数（可单测）：

```rust
/// 拆分：完整字段 JSON -> (加密部分, 明文部分)
fn split_fields(fields: &Value) -> (Value, Value) {
    // 加密部分固定为 {"password": fields["password"]}
    // 明文部分为其余所有键
}

/// 合成：明文列 + 解密后的 blob -> 完整字段 JSON
/// 规则：若 blob 含 password 以外的键（旧格式，含降级期间旧版写入）->
///   直接以 blob 为准、忽略 plain_fields（避免并入陈旧明文键，如降级期变更分类后的残留字段）；
/// 否则（新格式，blob 仅 password）->
///   以 plain_fields 解析结果为底（NULL/解析失败视为 {}），加上 blob 的 password。
fn merge_fields(plain_fields_text: Option<&str>, blob_fields: &Value) -> Value
```

两种格式共用一条读取路径；「旧格式以 blob 整体为准」使降级期间的任何旧版编辑（含变更分类）都不会被陈旧明文污染。

## 迁移设计（首次解锁静默回填）

`vault.rs` 新增：

```rust
/// 扫描全部条目，逐行：解密 -> 若 blob 含 password 以外的键（旧格式写入）->
/// split_fields -> 新 IV 重加密密码部分 -> UPDATE iv/encrypted_blob/plain_fields；
/// blob 仅含 password 的行跳过（已是新格式）。
/// 行级容错：单行失败跳过（下次解锁重试），并以 eprintln! 记录条目 id 便于诊断；
/// 函数自身不返回 Err，不阻断解锁。
/// UPDATE 不触碰 updated_at，避免迁移扰动「最近使用」排序。
fn backfill_plain_fields(conn: &Connection, key: &[u8; KEY_LEN])
```

- **触达路径 1**：`cmd_unlock` 在 canary 验证通过后调用，**先回填、再建立会话**（密钥为 `Copy` 数组，此顺序可关闭并发 IPC 在回填中途经 `list` 读到混合状态的理论窗口）。条目量为个人凭据库规模（几十量级），逐行 AES 解密开销远小于解锁本身的 PBKDF2（60 万次迭代），同步执行无感知。
- **触达路径 2**：`cmd_change_password` 重加密循环顺手完成拆分——解密后若 blob 含非密码键则走 `split_fields`，重加密仅密码部分并写入 `plain_fields`；已是新格式的行按现状整体重加密。
- `cmd_setup` 无需回填（新库无条目）。
- **幂等与自愈**：判定条件是「blob 含非密码键」而非 `plain_fields IS NULL`——既覆盖首次迁移，也覆盖**降级期间旧版编辑**导致的「blob 为完整字段、`plain_fields` 陈旧」状态（旧版编辑会整体加密写回且不更新明文列），升级后首次解锁自动修复。

### 混合状态（升级后未解锁期间）

- 未迁移行在 `meta_list` 中 `plainFields` 为 `null`，Spotlight 对该条目退化为现状搜索（标题/标签/分类/环境），副标题无账号段。
- `get` / `reveal_one` 经 `merge_fields` 统一读取，新旧格式行为一致；`list` 走明文列快路径（需会话，而回填在 `unlock` 中同步先行执行，正常流程不可达陈旧态；个别回填失败行退回解密路径）；`change_password` 按触达路径 2 处理。现有功能全程可用。
- 已知残留窗口（三重罕见条件叠加，可接受）：已迁移行经降级期旧版编辑（`plain_fields` 陈旧非 NULL）、再升级后该行回填又恰好失败时，`list` 快路径会短暂展示陈旧的 account/summary，直至下次解锁回填成功；`get`/`reveal_one` 因「旧格式以 blob 为准」始终正确。

## 后端接口变更（均不改返回结构的既有键）

| 命令                 | 变更                                                                                                                                                                                                                                               |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `meta_list`（:925）  | SELECT 增加 `plain_fields`；返回项追加 `"plainFields": <解析后的 JSON 对象，解析失败或未迁移均返回 null>`；keyword 过滤从 `title LIKE ?` 扩展为 `(title LIKE ? OR IFNULL(plain_fields,'') LIKE ?)`（口径一致性修补——当前所有调用方均不传 keyword） |
| `create` / `update`  | `build_fields()` 产物经 `split_fields` 拆两路：密码部分加密入 `encrypted_blob`，明文部分序列化入 `plain_fields`；INSERT/UPDATE 语句增加该列                                                                                                        |
| `get` / `reveal_one` | 解密 blob 后经 `merge_fields` 合成完整 `fields` 返回，结构不变                                                                                                                                                                                     |
| `list`（:513）       | 仍要求会话（鉴权语义不变）；`account`/`summary` 优先从 `plain_fields` 直接读取（省去逐行解密），`plain_fields IS NULL` 的行退回现状解密路径                                                                                                        |
| `change_password`    | 见迁移设计触达路径 2                                                                                                                                                                                                                               |
| `unlock`             | 会话建立后调用 `backfill_plain_fields`                                                                                                                                                                                                             |
| `record_usage`       | **取消活跃会话要求**（现要求会话，vault.rs:850）——它仅递增明文计数列 `view_count`/`copy_count`，与 `meta_list` 免会话口径一致；否则锁定态「复制账号」的计数会静默丢失                                                                              |

`build_fields()` 本身不改（仍产出完整字段 JSON，拆分发生在其后），现有 `test_build_fields_*` 单测不受影响。

## Spotlight 前端变更（`spotlight/providers/vault.ts`）

### 类型与数据

```ts
interface VaultPlainFields {
  account?: string;
  url?: string;
  address?: string;
  port?: number;
  serverType?: string;
  dbType?: string;
  dbName?: string;
  schema?: string;
  notes?: string;
}
// VaultMetaEntry 追加：
plainFields?: VaultPlainFields | null;
```

`buildItem` 的 `payload` 追加 `account: string`（空串表示无账号/未迁移）。

### 搜索字段与权重（均经 `makeField` 生成拼音首字母；空串字段过滤不入索引）

> 空串过滤对全部字段生效（含存量的标签/分类/环境字段——现状空串也会入索引），属顺带的轻微行为变更，无实际匹配影响，由单测锁定。

| 字段                                | 权重    | 说明             |
| ----------------------------------- | ------- | ---------------- |
| 标题                                | 1.2     | 现状不变         |
| **账号 account**                    | **1.1** | 新增，仅次于标题 |
| 标签                                | 1.0     | 现状不变         |
| **url / address / dbName / schema** | **0.8** | 新增             |
| 环境                                | 0.7     | 现状不变         |
| 分类                                | 0.6     | 现状不变         |
| **serverType / dbType**             | **0.6** | 新增             |
| **备注 notes**                      | **0.5** | 新增，权重最低   |

### 副标题

`buildSubtitle` 改为「分类 · 环境 · 账号」（各段为空则省略）。标签从副标题移除但仍参与搜索——账号的区分价值高于标签，且副标题空间有限。

### 「复制账号」动作

`buildActions` 在「复制密码」与「跳转到凭据工具」之间插入：

```ts
{ id: "copy_account", label: "复制账号", icon: "copy" }
```

（动作菜单当前不渲染 icon 字段，仅按既有约定填写。）

`executeAction` 增加 `copy_account` 分支：

1. 从 `payload.account` 取账号；空串 → `{ errorMessage: "该条目没有账号（旧条目需先解锁一次完成迁移）" }`（未迁移行与真无账号行同形，提示统一覆盖两种情况）。
2. `writeSecretToClipboard(account)`（含 `suppressClipboardCapture`，与密码复制同路径、与 Vault 面板复制账号行为一致），**不**调度 30 秒清空。
3. 复用 `recordCopy(entryId)` 记一次 copy 使用计数。
4. 返回 `{ closeSpotlight: true, toast: { message: "账号已复制到剪贴板", type: "success" } }`。

Enter 默认动作（`copyPasswordFlow`）完全不动。

## 安全声明与权衡

1. **明文化范围（用户已确认）**：除密码外的全部字段——账号、URL、地址、端口、服务器/数据库类型、库名、schema、**备注**——以明文存于 `plain_fields`。数据库文件（`lazycat.sqlite`）泄露时这些信息直接暴露，仅密码本身仍受主密码保护。备注中若存有其他密钥/临时密码将失去保护（设计阶段已单独向用户确认此点，用户选择备注明文可搜）。
2. **降级后果**：迁移后回退旧版本，旧版会把 `{"password": ...}` 当作完整 fields——密码可正常复制，但账号等字段在旧版界面显示为空；不丢数据（明文仍在 `plain_fields` 列，旧版忽略未知列）。降级期间若旧版编辑了条目（整体加密写回），`merge_fields` 的「旧格式以 blob 为准」规则保证读取正确（即使分类被变更也无陈旧键污染），再次升级后首次解锁由回填自愈（见迁移设计）。
3. **账号复制计入 copy 计数**：与密码复制共用 `record-usage`（本设计取消其会话要求，锁定态计数同样落库），轻微影响使用频次排序；Vault 面板的账号复制目前不计数，此差异可接受（Spotlight 场景下复制账号同样代表「该条目高频」）。
4. **`meta_list` 不要求会话**：现状如此（Spotlight 锁定态检索的前提），`plainFields` 经此通道暴露是本设计的预期行为，非泄露。
5. **性能**：`prefetch` 仍为单查询；逐行 JSON 解析开销可忽略；`list` 反而省去逐行 AES 解密。

## 测试计划

### Rust（`vault.rs` `#[cfg(test)]`，纯函数，无 DB 依赖）

```text
test_split_fields_app / _server / _database
  - 密码部分恰为 {"password": ...}；明文部分含其余全部键、不含 password
test_split_fields_empty_password
  - password 为空串时仍归加密部分（行为一致，不特判）
test_merge_fields_new_format
  - plain_fields 文本 + {"password"} blob -> 完整字段
test_merge_fields_legacy_format
  - plain_fields None + 完整 blob -> 等于 blob
test_merge_fields_stale_plain
  - blob 含非密码键且 plain_fields 非空（降级期编辑场景）-> 以 blob 为准，陈旧明文键不混入
test_merge_fields_invalid_plain_text
  - 新格式 blob + plain_fields 非法 JSON -> 按 {} 处理，结果仅含 password
```

迁移回填（依赖 DB 与会话）不写单测，列入人工验证清单。

### TypeScript（新增 `src/spotlight/providers/vault.test.ts`，vitest 已收集该路径）

`buildItem` / `buildSubtitle` 导出为可测函数：

```text
- plainFields.account 进入 searchFields（权重 1.1）且生成拼音首字母
- url/address/dbName/schema/serverType/dbType/notes 按表中权重进入索引；空串字段被过滤
- plainFields 为 null（未迁移）时字段集合与权重与现状一致（空串字段被过滤除外）
- 副标题 = 分类 · 环境 · 账号；账号为空时省略该段；不再含标签
- payload.account 透传
```

## 影响面

| 文件                                                 | 改动                                                                                                                                                          |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/desktop/src-tauri/src/tools/helpers.rs`        | `vault_entries` 增加 `plain_fields` 列迁移（1 行）                                                                                                            |
| `apps/desktop/src-tauri/src/tools/vault.rs`          | 新增 `split_fields`/`merge_fields`/`backfill_plain_fields`；`meta_list`/`create`/`update`/`get`/`list`/`reveal_one`/`change_password`/`unlock` 适配；新增单测 |
| `apps/desktop/src/spotlight/providers/vault.ts`      | `VaultPlainFields` 类型、searchFields 扩展、副标题改版、`copy_account` 动作                                                                                   |
| `apps/desktop/src/spotlight/providers/vault.test.ts` | 新增                                                                                                                                                          |

**不动**：`VaultPanel.vue`、`SpotlightPanel.vue`、`SpotlightVaultUnlockInput.vue`、`bridge/tauri.ts`（通道已存在）、`utils/vaultClipboard.ts`、`build_fields()` 及其单测。

## 验证

1. `cargo test`（vault 模块，含新增拆分/合成单测）
2. `pnpm test`（含新增 provider 单测）
3. `pnpm typecheck`
4. `pnpm --filter @lazycat/desktop build:web`
5. 人工验证（dev 启动）：
   - 旧库升级：解锁前 Spotlight 按标题可搜、按账号不可搜；主面板解锁一次后，锁定 vault，按账号可搜到条目且副标题显示账号
   - 新建/编辑条目后立即按账号可搜（无需重新解锁）
   - 锁定态 Enter 复制密码仍弹主密码输入条；解锁态直接复制并 30 秒清空
   - Tab 菜单「复制账号」：锁定/解锁态均直接复制，无主密码提示；无账号条目报「该条目没有账号（旧条目需先解锁一次完成迁移）」
   - 改主密码后：所有条目密码可正常解密；未迁移行被顺手迁移
   - 降级冒烟（可选）：旧版本打开迁移后的库，密码复制正常、账号列为空

## 风险与回滚

- **风险等级**：中。涉及加密数据格式变更与一次性迁移，但迁移幂等、行级容错、不删原始数据语义（密码始终在 blob 中）。
- **关键防线**：`merge_fields` 统一读取使新旧格式共存期间全功能可用；回填失败仅损失「该条目不可按账号搜」。
- **回滚**：代码回退后旧版仍可读新格式的密码（blob 解析规则兼容）；`plain_fields` 列残留无副作用。已迁移行的非密码字段在旧版显示为空，如需完全复原需手工脚本（不在本设计范围，发布说明中提示）。
