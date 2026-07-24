# 密码管理服务器端口 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 Vault 服务器凭据增加默认值为 22 的端口字段，并确保端口不进入列表摘要或 Spotlight 检索。

**Architecture:** 复用 `VaultEntryDialog.vue` 已有的通用 `port` 表单状态和 `plain_fields` JSON 存储，不新增数据库列或 IPC。前端按凭据类型设置端口默认值，Rust 的 `build_fields` 负责服务器端口的最终默认和持久化；检索继续使用现有显式字段白名单，不加入端口。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Vitest、Rust、Serde JSON、Cargo Test

---

## 文件结构

- Create: `apps/desktop/src/components/VaultEntryDialog.test.ts` — 约束服务器端口表单、默认值和保存载荷。
- Modify: `apps/desktop/src/components/VaultEntryDialog.vue` — 展示端口，处理分类默认值与旧数据回填，并提交端口。
- Modify: `apps/desktop/src/spotlight/providers/vault.test.ts` — 固化端口不进入 Spotlight 搜索字段的边界。
- Modify: `apps/desktop/src-tauri/src/tools/vault.rs` — 保存服务器端口，并用 Rust 单元测试覆盖显式端口、默认值和明密文拆分。

### Task 1: 后端持久化服务器端口

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/vault.rs:1211`
- Test: `apps/desktop/src-tauri/src/tools/vault.rs:1681`
- Test: `apps/desktop/src-tauri/src/tools/vault.rs:1831`

- [ ] **Step 1: 先修改服务器字段测试，要求保存显式端口和默认端口**

将 `test_build_fields_server` 改为：

```rust
#[test]
fn test_build_fields_server() {
    let p = json!({
        "address": "10.0.0.1",
        "port": 2200,
        "serverType": "Windows",
        "account": "root",
        "password": "p"
    });
    let f = build_fields("server", &p);
    assert_eq!(f["serverType"], "Windows");
    assert_eq!(f["port"], 2200);
}

#[test]
fn test_build_fields_server_defaults_port_to_22() {
    let f = build_fields("server", &json!({}));
    assert_eq!(f["port"], 22);
}
```

在 `test_split_fields_server` 的输入中加入 `"port": 2200`，并增加：

```rust
assert_eq!(plain["port"], 2200);
```

- [ ] **Step 2: 运行定向测试并确认 RED**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::vault::tests::test_build_fields_server
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::vault::tests::test_split_fields_server
```

Expected: `test_build_fields_server` 和 `test_build_fields_server_defaults_port_to_22` 因 `f["port"]` 为 `Null` 失败；拆分测试因明文字段没有端口失败。

- [ ] **Step 3: 写最小后端实现**

在 `build_fields` 的 `server` 分支加入：

```rust
"port": payload["port"].as_u64().unwrap_or(22),
```

最终服务器分支为：

```rust
"server" => json!({
    "address": payload["address"].as_str().unwrap_or(""),
    "port": payload["port"].as_u64().unwrap_or(22),
    "serverType": payload["serverType"].as_str().unwrap_or("Linux"),
    "account": payload["account"].as_str().unwrap_or(""),
    "password": payload["password"].as_str().unwrap_or(""),
    "notes": payload["notes"].as_str().unwrap_or(""),
}),
```

- [ ] **Step 4: 重跑定向测试并确认 GREEN**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::vault::tests::test_build_fields_server
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::vault::tests::test_split_fields_server
```

Expected: 两组测试全部通过。

- [ ] **Step 5: 提交后端改动**

```powershell
git add apps/desktop/src-tauri/src/tools/vault.rs
git commit -m "feat: 保存密码库服务器端口"
```

### Task 2: 前端表单展示、默认和提交端口

**Files:**
- Create: `apps/desktop/src/components/VaultEntryDialog.test.ts`
- Modify: `apps/desktop/src/components/VaultEntryDialog.vue:100`
- Modify: `apps/desktop/src/components/VaultEntryDialog.vue:260`
- Modify: `apps/desktop/src/components/VaultEntryDialog.vue:310`
- Modify: `apps/desktop/src/components/VaultEntryDialog.vue:342`
- Modify: `apps/desktop/src/components/VaultEntryDialog.vue:370`
- Modify: `apps/desktop/src/components/VaultEntryDialog.vue:461`
- Test: `apps/desktop/src/spotlight/providers/vault.test.ts:14`

- [ ] **Step 1: 创建失败的组件契约测试，并补充检索边界测试**

创建 `VaultEntryDialog.test.ts`：

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

const source = readFileSync(new URL("./VaultEntryDialog.vue", import.meta.url), "utf8");
const serverFields = source.slice(
  source.indexOf("<!-- Server fields -->"),
  source.indexOf("<!-- Database fields -->"),
);
const saveFlow = source.slice(source.indexOf("async function onSave"));

describe("VaultEntryDialog server port", () => {
  it("renders a bounded server port input", () => {
    expect(serverFields).toContain('label="端口"');
    expect(serverFields).toContain('v-model="form.port"');
    expect(serverFields).toContain(':min="1"');
    expect(serverFields).toContain(':max="65535"');
  });

  it("defaults server ports to 22 for new and legacy entries", () => {
    expect(source).toContain("const SERVER_DEFAULT_PORT = 22;");
    expect(source).toContain('newCat === "server"');
    expect(source).toContain("form.port = SERVER_DEFAULT_PORT;");
    expect(source).toMatch(/form\.category === "server"\s*\? SERVER_DEFAULT_PORT/u);
  });

  it("submits the server port", () => {
    const serverSave = saveFlow.slice(
      saveFlow.indexOf('form.category === "server"'),
      saveFlow.indexOf('form.category === "database"'),
    );
    expect(serverSave).toContain("payload.port = form.port;");
  });
});
```

在 `apps/desktop/src/spotlight/providers/vault.test.ts` 的 `vault provider buildItem` 分组增加：

```ts
it("port 不进入搜索索引", () => {
  const item = buildItem(
    entry({ category: "server", plainFields: { address: "10.0.0.8", port: 22 } }),
    true,
  );
  expect(item.searchFields.some((field) => field.text === "22")).toBe(false);
});
```

- [ ] **Step 2: 运行前端测试并确认 RED**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- VaultEntryDialog.test.ts vault.test.ts
```

Expected: Spotlight 的既有白名单边界通过；`VaultEntryDialog.test.ts` 因服务器模板没有端口、没有 `SERVER_DEFAULT_PORT` 且保存载荷没有端口而失败。

- [ ] **Step 3: 在服务器表单增加端口输入**

将服务器字段的第一部分调整为与数据库一致的类型/端口行，随后保留完整地址输入：

```vue
<div class="vault-form-row">
  <el-form-item label="服务器类型" class="vault-form-item-select">
    <el-select v-model="form.serverType" style="width: 100%">
      <el-option value="Linux" />
      <el-option value="Windows" />
      <el-option value="macOS" />
    </el-select>
  </el-form-item>
  <el-form-item label="端口" class="vault-form-item-port">
    <el-input-number v-model="form.port" :min="1" :max="65535" controls-position="right" style="width: 100%" />
  </el-form-item>
</div>
<el-form-item label="地址">
  <el-input v-model="form.address" placeholder="IP 或域名">
    <template #prefix>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="vault-input-icon">
        <rect x="2" y="2" width="20" height="8" rx="2" />
        <rect x="2" y="14" width="20" height="8" rx="2" />
        <circle cx="6" cy="6" r="1" fill="currentColor" />
        <circle cx="6" cy="18" r="1" fill="currentColor" />
      </svg>
    </template>
  </el-input>
</el-form-item>
```

- [ ] **Step 4: 实现分类默认值和旧数据兼容**

在 `DB_DEFAULT_PORT` 前增加：

```ts
const SERVER_DEFAULT_PORT = 22;
```

将 `defaultForm` 的端口改为：

```ts
port: SERVER_DEFAULT_PORT,
```

分类监听先处理编辑态字段迁移，再按目标类型设置端口，并使用同步监听确保 `show()` 随后写入的已保存端口不会被异步默认值覆盖：

```ts
watch(() => form.category, (newCat, oldCat) => {
  if (isEdit.value) {
    if (oldCat === "app" && (newCat === "server" || newCat === "database")) {
      if (!form.address && form.url) form.address = form.url;
    } else if ((oldCat === "server" || oldCat === "database") && newCat === "app") {
      if (!form.url && form.address) form.url = form.address;
    }
  }
  if (newCat === "server") {
    form.port = SERVER_DEFAULT_PORT;
  } else if (newCat === "database" && form.dbType in DB_DEFAULT_PORT) {
    form.port = DB_DEFAULT_PORT[form.dbType];
  }
}, { flush: "sync" });
```

数据库类型监听同样增加 `{ flush: "sync" }`，保证编辑已有数据库记录时最终以记录端口为准：

```ts
watch(() => form.dbType, (newType) => {
  if (form.category === "database" && newType in DB_DEFAULT_PORT) {
    form.port = DB_DEFAULT_PORT[newType];
  }
}, { flush: "sync" });
```

编辑记录读取端口时使用分类默认值：

```ts
form.port = typeof f.port === "number"
  ? f.port
  : form.category === "server"
    ? SERVER_DEFAULT_PORT
    : 3306;
```

种子读取端口时保留数字或数字字符串，缺失时服务器回退 22：

```ts
const seedPort = Number(fields.port);
form.port = Number.isInteger(seedPort) && seedPort >= 1 && seedPort <= 65535
  ? seedPort
  : form.category === "server"
    ? SERVER_DEFAULT_PORT
    : form.port;
```

- [ ] **Step 5: 服务器保存载荷携带端口**

在服务器分支增加：

```ts
payload.port = form.port;
```

最终分支为：

```ts
} else if (form.category === "server") {
  payload.address = form.address;
  payload.port = form.port;
  payload.serverType = form.serverType;
}
```

- [ ] **Step 6: 重跑前端测试并确认 GREEN**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- VaultEntryDialog.test.ts vault.test.ts
```

Expected: 两个测试文件全部通过；端口 22 不出现在 Spotlight `searchFields`。

- [ ] **Step 7: 提交前端改动**

```powershell
git add apps/desktop/src/components/VaultEntryDialog.vue apps/desktop/src/components/VaultEntryDialog.test.ts apps/desktop/src/spotlight/providers/vault.test.ts
git commit -m "feat: 增加密码库服务器端口输入"
```

### Task 3: 完整验证

**Files:**
- Verify: `apps/desktop/src/components/VaultEntryDialog.vue`
- Verify: `apps/desktop/src-tauri/src/tools/vault.rs`

- [ ] **Step 1: 运行 Vault 相关前端测试**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- VaultEntryDialog.test.ts vault.test.ts
```

Expected: 全部通过，无错误或警告。

- [ ] **Step 2: 运行 Vault 后端测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::vault::tests
```

Expected: `tools::vault::tests` 全部通过。

- [ ] **Step 3: 运行类型检查**

Run:

```powershell
pnpm typecheck
```

Expected: 所有工作区类型检查通过。

- [ ] **Step 4: 构建桌面渲染层**

Run:

```powershell
pnpm --filter @lazycat/desktop build:web
```

Expected: Vite 构建成功。

- [ ] **Step 5: 检查最终差异**

Run:

```powershell
git diff --check HEAD~2
git status --short
```

Expected: `git diff --check` 无输出；`git status` 为干净状态，或只显示执行期间新增的用户改动。
