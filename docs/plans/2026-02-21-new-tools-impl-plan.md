# 新工具批量实现计划

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 为 LazyCat 新增 10 个开发者工具，分 4 个批次实现。

**Architecture:** 统一走 Rust IPC（仅 JSON 压缩例外）。5 个新面板 + 5 个融入现有面板。每个新面板遵循项目现有模式：Vue 组件 → tool-registry 注册 → CHANNEL_MAP 映射 → Rust domain action。

**Tech Stack:** Tauri 2 + Vue 3 + TypeScript + Rust, Element Plus UI, serde_json, 新增 toml + bcrypt crate

---

## 关键文件路径速查

| 文件                                      | 用途                             |
| ----------------------------------------- | -------------------------------- |
| `apps/desktop/src/App.vue`                | sidebarItems 定义 (lines 81-184) |
| `apps/desktop/src/tool-registry.ts`       | 组件映射 (lines 7-45)            |
| `apps/desktop/src/bridge/tauri.ts`        | CHANNEL_MAP (lines 26-147)       |
| `apps/desktop/src-tauri/src/tools/mod.rs` | Rust 域分发 (lines 29-57)        |
| `apps/desktop/src-tauri/Cargo.toml`       | Rust 依赖 (lines 15-44)          |

## 分组映射（修正）

App.vue 中的分组实际结构：

- `id="text"`, name="数据转换" → 命名转换、配置互转 加入此组
- `id="crypto"`, name="加密与安全" → Bcrypt 加入此组
- `id="network"`, name="网络与系统" → HTTP 状态码、chmod 加入此组
- `id="calc"`, name="时间工具" → 日期计算器 加入此组

---

## 批次 1: 文本组 + 密码强度

### Task 1.1: 命名风格转换 — Rust 后端

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/text.rs` (在 execute 函数的 `_ =>` 前添加 action)

**Step 1: 写测试**

在 `text.rs` 底部 `#[cfg(test)] mod tests` 中添加：

```rust
#[test]
fn naming_convert_camel() {
    let r = execute("naming_convert", &json!({"input": "hello_world"})).unwrap();
    assert_eq!(r["camelCase"], "helloWorld");
    assert_eq!(r["pascalCase"], "HelloWorld");
    assert_eq!(r["snakeCase"], "hello_world");
    assert_eq!(r["screamingSnake"], "HELLO_WORLD");
    assert_eq!(r["kebabCase"], "hello-world");
    assert_eq!(r["dotCase"], "hello.world");
}

#[test]
fn naming_convert_from_camel() {
    let r = execute("naming_convert", &json!({"input": "helloWorld"})).unwrap();
    assert_eq!(r["snakeCase"], "hello_world");
    assert_eq!(r["kebabCase"], "hello-world");
}

#[test]
fn naming_convert_multiline() {
    let r = execute("naming_convert", &json!({"input": "hello_world\nfoo_bar"})).unwrap();
    assert_eq!(r["camelCase"], "helloWorld\nfooBar");
}
```

**Step 2: 运行测试确认失败**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::text::tests::naming_convert -- --nocapture`
Expected: FAIL — `unsupported text action: naming_convert`

**Step 3: 实现 naming_convert**

在 `text.rs` 中添加辅助函数 `split_identifier(s: &str) -> Vec<String>` 和 `naming_convert(payload)` 函数：

- `split_identifier`: 按 `_`、`-`、`.`、空格、大小写边界（`aB` → `a`, `B`）拆分，全部转小写
- 6 种风格拼接：camelCase, PascalCase, snake_case, SCREAMING_SNAKE_CASE, kebab-case, dot.case
- 多行支持：按 `\n` 分割，逐行转换

在 `execute` 函数的 match 中添加：

```rust
"naming_convert" => naming_convert(payload),
```

**Step 4: 运行测试确认通过**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::text::tests::naming_convert -- --nocapture`
Expected: 3 tests PASS

**Step 5: 提交**

```bash
git add apps/desktop/src-tauri/src/tools/text.rs
git commit -m "feat(text): add naming_convert action for case style conversion"
```

---

### Task 1.2: 命名风格转换 — 前端接入

**Files:**

- Create: `apps/desktop/src/components/NamingCasePanel.vue`
- Modify: `apps/desktop/src/tool-registry.ts` (添加组件映射)
- Modify: `apps/desktop/src/App.vue` (sidebarItems text 组添加工具)
- Modify: `apps/desktop/src/bridge/tauri.ts` (CHANNEL_MAP 添加通道)

**Step 1: 添加 IPC 通道**

在 `bridge/tauri.ts` 的 CHANNEL_MAP 中，`tool:text:presets` 之后添加：

```typescript
"tool:text:naming-convert": { domain: "text", action: "naming_convert" },
```

**Step 2: 创建 NamingCasePanel.vue**

```vue
<template>
  <div class="naming-case-panel">
    <el-input
      v-model="input"
      type="textarea"
      :rows="4"
      placeholder="输入标识符，每行一个（如 hello_world、helloWorld）"
      @input="convert"
    />
    <div class="results-grid">
      <div v-for="item in styles" :key="item.key" class="result-card">
        <div class="result-label">{{ item.label }}</div>
        <el-input :model-value="results[item.key] || ''" readonly>
          <template #append>
            <el-button @click="copy(results[item.key])">复制</el-button>
          </template>
        </el-input>
      </div>
    </div>
  </div>
</template>
```

Script: 定义 `styles` 数组（6 种风格的 key+label），`input` ref，`results` reactive 对象。
`convert()` 调用 `invokeToolByChannel("tool:text:naming-convert", { input: input.value })`，300ms 防抖。
`copy()` 用 `navigator.clipboard.writeText()`。

样式：`.results-grid` 用 `display: grid; grid-template-columns: repeat(2, 1fr); gap: 12px;`

**Step 3: 注册组件**

在 `tool-registry.ts` 的 `toolRegistry` 对象中添加：

```typescript
"naming-case": defineAsyncComponent(() => import("./components/NamingCasePanel.vue")),
```

**Step 4: 添加侧边栏入口**

在 `App.vue` 的 `id="text"` 组的 tools 数组末尾（`text-process` 之后）添加：

```typescript
{ id: "naming-case", name: "命名转换", desc: "camelCase/snake_case/PascalCase 互转" },
```

**Step 5: 类型检查**

Run: `pnpm typecheck`
Expected: PASS

**Step 6: 提交**

```bash
git add apps/desktop/src/components/NamingCasePanel.vue apps/desktop/src/tool-registry.ts apps/desktop/src/App.vue apps/desktop/src/bridge/tauri.ts
git commit -m "feat(ui): add NamingCasePanel for case style conversion"
```

---

### Task 1.3: 文本统计 — Rust 扩展 stats

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/text.rs` (扩展 stats 返回字段)

**Step 1: 写测试**

在 `text.rs` 测试模块中添加：

```rust
#[test]
fn process_returns_extended_stats() {
    let r = execute("process", &json!({
        "input": "Hello 你好\nWorld",
        "ops": { "trim": false, "removeEmpty": false, "dedupe": false, "sort": false,
                 "includeFilter": false, "excludeFilter": false, "replace": false,
                 "addPrefix": false, "addSuffix": false, "extractColumn": false },
        "lineEnding": "keep"
    })).unwrap();
    let stats = &r["stats"];
    assert!(stats["charsWithSpaces"].as_u64().unwrap() > 0);
    assert!(stats["charsNoSpaces"].as_u64().unwrap() > 0);
    assert!(stats["chineseChars"].as_u64().unwrap() >= 2);
    assert!(stats["englishWords"].as_u64().unwrap() >= 2);
    assert!(stats["bytesUtf8"].as_u64().unwrap() > 0);
    assert!(stats["longestLine"].as_u64().unwrap() > 0);
}
```

**Step 2: 运行测试确认失败**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::text::tests::process_returns_extended_stats -- --nocapture`
Expected: FAIL — `charsWithSpaces` 字段不存在

**Step 3: 实现扩展 stats**

在 `text.rs` 的 `process_text` 函数中，构建 stats JSON 时新增字段：

```rust
"charsWithSpaces": input.chars().count(),
"charsNoSpaces": input.chars().filter(|c| !c.is_whitespace()).count(),
"chineseChars": input.chars().filter(|c| ('\u{4e00}'..='\u{9fff}').contains(c)).count(),
"englishWords": input.split_whitespace().filter(|w| w.chars().any(|c| c.is_ascii_alphabetic())).count(),
"bytesUtf8": input.len(),
"longestLine": input.lines().map(|l| l.chars().count()).max().unwrap_or(0),
```

**Step 4: 运行测试确认通过**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::text::tests::process_returns_extended_stats -- --nocapture`
Expected: PASS

**Step 5: 提交**

```bash
git add apps/desktop/src-tauri/src/tools/text.rs
git commit -m "feat(text): extend process stats with char/word/byte counts"
```

---

### Task 1.4: 文本统计 — 前端显示扩展 stats

**Files:**

- Modify: `apps/desktop/src/components/TextProcessPanel.vue` (扩展摘要栏)
- Modify: `apps/desktop/src/types/text.ts` (扩展 TextProcessStats 类型)

**Step 1: 扩展类型定义**

在 `types/text.ts` 的 `TextProcessStats` 接口中添加：

```typescript
charsWithSpaces: number;
charsNoSpaces: number;
chineseChars: number;
englishWords: number;
bytesUtf8: number;
longestLine: number;
```

**Step 2: 扩展 reactive stats 对象**

在 `TextProcessPanel.vue` 的 `stats` reactive 对象中添加对应的 6 个新字段，初始值为 0。

**Step 3: 扩展摘要栏 HTML**

在 `summary-grid` div 中（现有 4 个 `summary-item` 之后）添加 6 个新的 `summary-item`：

- 字符数(含空格): `{{ stats.charsWithSpaces }}`
- 字符数(不含空格): `{{ stats.charsNoSpaces }}`
- 中文字数: `{{ stats.chineseChars }}`
- 英文单词: `{{ stats.englishWords }}`
- UTF-8 字节: `{{ stats.bytesUtf8 }}`
- 最长行: `{{ stats.longestLine }}`

**Step 4: 调整 CSS grid**

将 `.summary-grid` 的 `grid-template-columns` 从 `repeat(4, ...)` 改为：

```css
grid-template-columns: repeat(auto-fill, minmax(120px, 1fr));
```

**Step 5: 类型检查**

Run: `pnpm typecheck`
Expected: PASS

**Step 6: 提交**

```bash
git add apps/desktop/src/components/TextProcessPanel.vue apps/desktop/src/types/text.ts
git commit -m "feat(ui): display extended text stats in TextProcessPanel"
```

---

### Task 1.5: 密码强度分析 — Rust 后端

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/gen.rs` (新增 password_strength action)

**Step 1: 写测试**

在 `gen.rs` 测试模块中添加：

```rust
#[test]
fn password_strength_weak() {
    let r = execute("password_strength", &json!({"password": "123456"})).unwrap();
    assert_eq!(r["level"], "weak");
    assert!(r["score"].as_u64().unwrap() < 30);
}

#[test]
fn password_strength_strong() {
    let r = execute("password_strength", &json!({"password": "Tr0ub4dor&3xY!"})).unwrap();
    let score = r["score"].as_u64().unwrap();
    assert!(score >= 70);
}

#[test]
fn password_strength_details() {
    let r = execute("password_strength", &json!({"password": "abc"})).unwrap();
    assert!(r["details"].as_array().unwrap().len() > 0);
}
```

**Step 2: 运行测试确认失败**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::gen::tests::password_strength -- --nocapture`
Expected: FAIL

**Step 3: 实现 password_strength**

在 `gen.rs` 中添加 `password_strength(payload)` 函数：

- 评分规则（每项 0-20 分，满分 100）：
  - 长度：<6 得 0, 6-8 得 5, 8-12 得 10, 12-16 得 15, >16 得 20
  - 大小写混合：有大写+小写得 20
  - 数字：含数字得 20
  - 特殊字符：含特殊字符得 20
  - 无连续重复（如 aaa）：无重复得 20
- level 映射：<30 weak, 30-59 medium, 60-79 strong, >=80 very_strong
- details 数组：每条规则的 `{ rule, passed, message }` 中文描述

在 `execute` 的 match 中添加：

```rust
"password_strength" => password_strength(payload),
```

**Step 4: 运行测试确认通过**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::gen::tests::password_strength -- --nocapture`
Expected: 3 tests PASS

**Step 5: 提交**

```bash
git add apps/desktop/src-tauri/src/tools/gen.rs
git commit -m "feat(gen): add password_strength scoring action"
```

---

### Task 1.6: 密码强度分析 — 前端融入 UuidPanel

**Files:**

- Modify: `apps/desktop/src/components/UuidPanel.vue` (添加强度分析 UI)
- Modify: `apps/desktop/src/bridge/tauri.ts` (添加 IPC 通道)

**Step 1: 添加 IPC 通道**

在 `bridge/tauri.ts` 的 CHANNEL_MAP 中，`tool:gen:password` 之后添加：

```typescript
"tool:gen:password-strength": { domain: "gen", action: "password_strength" },
```

**Step 2: 修改 UuidPanel.vue**

在输出 textarea 和按钮之间添加密码强度卡片：

```vue
<div v-if="strengthResult" class="panel-grid-full strength-card">
  <div class="strength-header">
    <span>密码强度：{{ strengthResult.level }}</span>
    <el-progress :percentage="strengthResult.score"
      :color="strengthColor" :show-text="false" />
  </div>
  <div class="strength-details">
    <el-tag v-for="d in strengthResult.details" :key="d.rule"
      :type="d.passed ? 'success' : 'danger'" size="small">
      {{ d.message }}
    </el-tag>
  </div>
</div>
```

Script 中添加：

- `strengthResult` ref
- `analyzeStrength()` 函数：调用 `invokeToolByChannel("tool:gen:password-strength", { password })`
- 在 `generatePassword()` 成功后自动调用 `analyzeStrength()`
- 添加「分析强度」按钮，允许手动输入密码分析
- `strengthColor` computed：weak=red, medium=orange, strong=blue, very_strong=green

**Step 3: 类型检查**

Run: `pnpm typecheck`
Expected: PASS

**Step 4: 提交**

```bash
git add apps/desktop/src/components/UuidPanel.vue apps/desktop/src/bridge/tauri.ts
git commit -m "feat(ui): add password strength analysis to UuidPanel"
```

---

### Task 1.7: 批次 1 集成验证

**Step 1: 运行全部 Rust 测试**

Run: `cd apps/desktop/src-tauri && cargo test --lib`
Expected: ALL PASS

**Step 2: 前端类型检查**

Run: `pnpm typecheck`
Expected: PASS

**Step 3: 前端构建**

Run: `pnpm --filter @lazycat/desktop build:web`
Expected: PASS

**Step 4: 单元测试**

Run: `pnpm test`
Expected: PASS

---

## 批次 2: 数据转换组

### Task 2.1: 配置文件互转 — Rust 后端

**Files:**

- Modify: `apps/desktop/src-tauri/Cargo.toml` (添加 `toml` 依赖)
- Modify: `apps/desktop/src-tauri/src/tools/convert.rs` (新增 config_convert action)

**Step 1: 添加 toml 依赖**

在 `Cargo.toml` 的 `[dependencies]` 中添加：

```toml
toml = "0.8"
```

**Step 2: 写测试**

在 `convert.rs` 测试模块中添加：

```rust
#[test]
fn config_convert_properties_to_yaml() {
    let r = execute("config_convert", &json!({
        "input": "server.port=8080\nserver.host=localhost",
        "from": "properties",
        "to": "yaml"
    })).unwrap();
    let output = r["output"].as_str().unwrap();
    assert!(output.contains("server"));
    assert!(output.contains("8080"));
}

#[test]
fn config_convert_yaml_to_toml() {
    let r = execute("config_convert", &json!({
        "input": "server:\n  port: 8080",
        "from": "yaml",
        "to": "toml"
    })).unwrap();
    let output = r["output"].as_str().unwrap();
    assert!(output.contains("[server]"));
    assert!(output.contains("8080"));
}

#[test]
fn config_convert_env_to_properties() {
    let r = execute("config_convert", &json!({
        "input": "DB_HOST=localhost\nDB_PORT=5432",
        "from": "env",
        "to": "properties"
    })).unwrap();
    let output = r["output"].as_str().unwrap();
    assert!(output.contains("DB_HOST=localhost"));
}
```

**Step 3: 运行测试确认失败**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::convert::tests::config_convert -- --nocapture`
Expected: FAIL

**Step 4: 实现 config_convert**

在 `convert.rs` 中添加：

- `parse_properties(s) -> Value`: 按 `=` 分割，支持嵌套 key（`a.b.c=v` → `{a:{b:{c:v}}}`）
- `parse_env(s) -> Value`: 按 `=` 分割，扁平 key-value
- `serialize_properties(v) -> String`: 递归展平为 `a.b.c=v` 格式
- `serialize_env(v) -> String`: 扁平 `KEY=VALUE` 格式
- `config_convert(payload)`: 根据 from/to 调用对应的 parse → serde_json::Value → serialize

YAML 用已有 `serde_yaml`，TOML 用新增 `toml` crate。

在 `execute` 的 match 中添加：

```rust
"config_convert" => config_convert(payload),
```

**Step 5: 运行测试确认通过**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::convert::tests::config_convert -- --nocapture`
Expected: 3 tests PASS

**Step 6: 提交**

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src/tools/convert.rs
git commit -m "feat(convert): add config_convert for Properties/YAML/TOML/.env"
```

---

### Task 2.2: 配置文件互转 — 前端面板

**Files:**

- Create: `apps/desktop/src/components/ConfigConvertPanel.vue`
- Modify: `apps/desktop/src/tool-registry.ts`
- Modify: `apps/desktop/src/App.vue`
- Modify: `apps/desktop/src/bridge/tauri.ts`

**Step 1: 添加 IPC 通道**

在 `bridge/tauri.ts` CHANNEL_MAP 中添加：

```typescript
"tool:convert:config-convert": { domain: "convert", action: "config_convert" },
```

**Step 2: 创建 ConfigConvertPanel.vue**

UI 布局：

- 顶部工具栏：源格式下拉 + 交换按钮 + 目标格式下拉 + 转换按钮
- 下方双栏：左侧输入 textarea + 右侧输出 textarea (readonly)
- 格式选项：Properties / YAML / TOML / .env

Script: `from` ref, `to` ref, `input` ref, `output` ref。
`convert()` 调用 `invokeToolByChannel("tool:convert:config-convert", { input, from, to })`。
`swap()` 交换 from/to 并将 output 填入 input。

**Step 3: 注册组件和侧边栏**

`tool-registry.ts`:

```typescript
"config-convert": defineAsyncComponent(() => import("./components/ConfigConvertPanel.vue")),
```

`App.vue` 的 `id="text"` 组 tools 数组中添加：

```typescript
{ id: "config-convert", name: "配置互转", desc: "Properties/YAML/TOML/.env 格式互转" },
```

**Step 4: 类型检查**

Run: `pnpm typecheck`
Expected: PASS

**Step 5: 提交**

```bash
git add apps/desktop/src/components/ConfigConvertPanel.vue apps/desktop/src/tool-registry.ts apps/desktop/src/App.vue apps/desktop/src/bridge/tauri.ts
git commit -m "feat(ui): add ConfigConvertPanel for config format conversion"
```

---

### Task 2.3: YAML 校验/格式化 — Rust 后端

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/convert.rs`

**Step 1: 写测试**

```rust
#[test]
fn yaml_validate_valid() {
    let r = execute("yaml_validate", &json!({"input": "key: value\nlist:\n  - a\n  - b"})).unwrap();
    assert_eq!(r["valid"], true);
    assert!(r["error"].is_null());
}

#[test]
fn yaml_validate_invalid() {
    let r = execute("yaml_validate", &json!({"input": "key: [unclosed"})).unwrap();
    assert_eq!(r["valid"], false);
    assert!(r["error"]["message"].as_str().unwrap().len() > 0);
}

#[test]
fn yaml_format_indent() {
    let r = execute("yaml_format", &json!({"input": "key:   value\nlist:\n    - a", "indent": 2})).unwrap();
    let output = r["output"].as_str().unwrap();
    assert!(output.contains("key:"));
}
```

**Step 2: 运行测试确认失败**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::convert::tests::yaml_validate -- --nocapture`
Expected: FAIL

**Step 3: 实现**

在 `convert.rs` 中添加：

- `yaml_validate(payload)`: 用 `serde_yaml::from_str::<Value>()` 尝试解析，成功返回 `{valid:true, error:null}`，失败返回 `{valid:false, error:{message}}`
- `yaml_format(payload)`: 解析后用 `serde_yaml::to_string()` 重新序列化（serde_yaml 默认 2 空格缩进）

在 `execute` match 中添加两个 arm。

**Step 4: 运行测试确认通过**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::convert::tests::yaml -- --nocapture`
Expected: 3 tests PASS

**Step 5: 提交**

```bash
git add apps/desktop/src-tauri/src/tools/convert.rs
git commit -m "feat(convert): add yaml_validate and yaml_format actions"
```

---

### Task 2.4: YAML 校验/格式化 — 前端融入 JsonYamlPanel

**Files:**

- Modify: `apps/desktop/src/components/JsonYamlPanel.vue`
- Modify: `apps/desktop/src/bridge/tauri.ts`

**Step 1: 添加 IPC 通道**

```typescript
"tool:convert:yaml-validate": { domain: "convert", action: "yaml_validate" },
"tool:convert:yaml-format": { domain: "convert", action: "yaml_format" },
```

**Step 2: 修改 JsonYamlPanel.vue**

在按钮区添加两个新按钮：

```vue
<el-button @click="validateYaml">YAML 校验</el-button>
<el-button @click="formatYaml">YAML 格式化</el-button>
```

Script 中添加：

- `validateYaml()`: 调用 `tool:convert:yaml-validate`，成功显示 `ElMessage.success("YAML 语法正确")`，失败显示错误信息
- `formatYaml()`: 调用 `tool:convert:yaml-format`，将格式化结果写入输入框（或输出框）

**Step 3: 类型检查**

Run: `pnpm typecheck`
Expected: PASS

**Step 4: 提交**

```bash
git add apps/desktop/src/components/JsonYamlPanel.vue apps/desktop/src/bridge/tauri.ts
git commit -m "feat(ui): add YAML validate/format to JsonYamlPanel"
```

---

### Task 2.5: JSON 压缩 — 前端融入 FormatterPanel

**Files:**

- Modify: `apps/desktop/src/components/FormatterPanel.vue`

**Step 1: 修改 FormatterPanel.vue**

在格式化按钮旁添加压缩按钮（仅 JSON 时显示）：

```vue
<el-button v-if="detectedKind === 'json'" @click="minifyJson">压缩</el-button>
```

Script 中添加 `minifyJson()` 函数：

```typescript
async function minifyJson() {
  try {
    const parsed = JSON.parse(inputCode.value);
    outputCode.value = JSON.stringify(parsed);
  } catch (e: any) {
    ElMessage.error(`JSON 解析失败: ${e.message}`);
  }
}
```

**Step 2: 类型检查**

Run: `pnpm typecheck`
Expected: PASS

**Step 3: 提交**

```bash
git add apps/desktop/src/components/FormatterPanel.vue
git commit -m "feat(ui): add JSON minify button to FormatterPanel"
```

---

### Task 2.6: 批次 2 集成验证

**Step 1:** `cd apps/desktop/src-tauri && cargo test --lib`
**Step 2:** `pnpm typecheck`
**Step 3:** `pnpm --filter @lazycat/desktop build:web`
**Step 4:** `pnpm test`

Expected: ALL PASS

---

## 批次 3: 网络组

### Task 3.1: HTTP 状态码速查 — Rust 后端

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/network.rs`

**Step 1: 写测试**

在 `network.rs` 测试模块中添加：

```rust
#[test]
fn http_status_list_returns_groups() {
    let r = execute("http_status_list", &json!({})).unwrap();
    let groups = r["groups"].as_array().unwrap();
    assert_eq!(groups.len(), 5); // 1xx-5xx
    assert_eq!(groups[0]["category"], "1xx");
}

#[test]
fn http_status_lookup_by_code() {
    let r = execute("http_status_lookup", &json!({"query": "404"})).unwrap();
    let results = r["results"].as_array().unwrap();
    assert!(results.len() >= 1);
    assert_eq!(results[0]["code"], 404);
}

#[test]
fn http_status_lookup_by_text() {
    let r = execute("http_status_lookup", &json!({"query": "not found"})).unwrap();
    let results = r["results"].as_array().unwrap();
    assert!(results.iter().any(|r| r["code"] == 404));
}
```

**Step 2: 运行测试确认失败**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::network::tests::http_status -- --nocapture`
Expected: FAIL

**Step 3: 实现**

在 `network.rs` 中添加：

- `fn http_status_data() -> Vec<HttpStatus>` 返回静态数据，包含常用 HTTP 状态码（约 60 个），每个含 code/name/desc/usage 字段
- `fn http_status_list(payload) -> Result<Value, String>` 按 1xx-5xx 分组返回
- `fn http_status_lookup(payload) -> Result<Value, String>` 按 code 精确匹配或 name/desc 模糊匹配

状态码数据结构：

```rust
struct HttpStatus { code: u16, name: &'static str, desc: &'static str, usage: &'static str }
```

在 `execute` match 中添加：

```rust
"http_status_list" => http_status_list(payload),
"http_status_lookup" => http_status_lookup(payload),
```

**Step 4: 运行测试确认通过**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::network::tests::http_status -- --nocapture`
Expected: 3 tests PASS

**Step 5: 提交**

```bash
git add apps/desktop/src-tauri/src/tools/network.rs
git commit -m "feat(network): add HTTP status code lookup actions"
```

---

### Task 3.2: HTTP 状态码速查 — 前端面板

**Files:**

- Create: `apps/desktop/src/components/HttpStatusPanel.vue`
- Modify: `apps/desktop/src/tool-registry.ts`
- Modify: `apps/desktop/src/App.vue`
- Modify: `apps/desktop/src/bridge/tauri.ts`

**Step 1: 添加 IPC 通道**

```typescript
"tool:network:http-status-list": { domain: "network", action: "http_status_list" },
"tool:network:http-status-lookup": { domain: "network", action: "http_status_lookup" },
```

**Step 2: 创建 HttpStatusPanel.vue**

UI 布局：

- 顶部搜索框：`el-input` 带搜索图标，placeholder="输入状态码或描述搜索"
- 下方内容区：`el-collapse` 按 1xx/2xx/3xx/4xx/5xx 分组
- 每个状态码：`el-descriptions` 显示码值、英文名、中文说明、使用场景
- 搜索时切换为扁平列表显示匹配结果

Script:

- `onMounted` 调用 `http_status_list` 加载全部数据
- `search` ref + 300ms 防抖 watcher，非空时调用 `http_status_lookup`
- 搜索为空时显示分组视图，非空时显示搜索结果

**Step 3: 注册组件和侧边栏**

`tool-registry.ts`:

```typescript
"http-status": defineAsyncComponent(() => import("./components/HttpStatusPanel.vue")),
```

`App.vue` 的 `id="network"` 组 tools 数组中添加：

```typescript
{ id: "http-status", name: "HTTP 状态码", desc: "HTTP 状态码速查与说明" },
```

**Step 4: 类型检查**

Run: `pnpm typecheck`
Expected: PASS

**Step 5: 提交**

```bash
git add apps/desktop/src/components/HttpStatusPanel.vue apps/desktop/src/tool-registry.ts apps/desktop/src/App.vue apps/desktop/src/bridge/tauri.ts
git commit -m "feat(ui): add HttpStatusPanel for HTTP status code lookup"
```

---

### Task 3.3: chmod 计算器 — Rust 后端

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/network.rs`

**Step 1: 写测试**

```rust
#[test]
fn chmod_calc_from_numeric() {
    let r = execute("chmod_calc", &json!({"mode": "numeric", "value": "755"})).unwrap();
    assert_eq!(r["numeric"], "755");
    assert_eq!(r["symbolic"], "rwxr-xr-x");
    assert_eq!(r["owner"]["read"], true);
    assert_eq!(r["owner"]["write"], true);
    assert_eq!(r["owner"]["execute"], true);
    assert_eq!(r["group"]["write"], false);
}

#[test]
fn chmod_calc_from_symbolic() {
    let r = execute("chmod_calc", &json!({"mode": "symbolic", "value": "rw-r--r--"})).unwrap();
    assert_eq!(r["numeric"], "644");
}

#[test]
fn chmod_calc_zero() {
    let r = execute("chmod_calc", &json!({"mode": "numeric", "value": "000"})).unwrap();
    assert_eq!(r["symbolic"], "---------");
}
```

**Step 2: 运行测试确认失败**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::network::tests::chmod_calc -- --nocapture`
Expected: FAIL

**Step 3: 实现**

在 `network.rs` 中添加 `chmod_calc(payload)` 函数：

- `mode == "numeric"`: 解析 3 位八进制数，拆分为 owner/group/other 各 3 bit (r=4, w=2, x=1)
- `mode == "symbolic"`: 解析 9 字符 rwx 字符串，转为数字
- 返回 `{ numeric, symbolic, owner: {read,write,execute}, group: {...}, other: {...} }`

在 `execute` match 中添加：

```rust
"chmod_calc" => chmod_calc(payload),
```

**Step 4: 运行测试确认通过**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::network::tests::chmod_calc -- --nocapture`
Expected: 3 tests PASS

**Step 5: 提交**

```bash
git add apps/desktop/src-tauri/src/tools/network.rs
git commit -m "feat(network): add chmod_calc permission calculator"
```

---

### Task 3.4: chmod 计算器 — 前端面板

**Files:**

- Create: `apps/desktop/src/components/ChmodCalcPanel.vue`
- Modify: `apps/desktop/src/tool-registry.ts`
- Modify: `apps/desktop/src/App.vue`
- Modify: `apps/desktop/src/bridge/tauri.ts`

**Step 1: 添加 IPC 通道**

```typescript
"tool:network:chmod-calc": { domain: "network", action: "chmod_calc" },
```

**Step 2: 创建 ChmodCalcPanel.vue**

UI 布局：

- 3x3 复选框矩阵：行 = Owner/Group/Other，列 = Read/Write/Execute
- 数字输入框：3 位八进制数（如 755），双向联动
- 符号显示：只读文本（如 rwxr-xr-x）
- 快捷按钮行：644、755、777、600、400

Script:

- `perms` reactive 对象：`{ owner: {r,w,x}, group: {r,w,x}, other: {r,w,x} }`
- `numericValue` ref
- 复选框变化时 → 计算数字 → 调用 Rust 确认
- 数字输入变化时 → 调用 Rust → 更新复选框
- 快捷按钮直接设置 numericValue 并触发更新

**Step 3: 注册组件和侧边栏**

`tool-registry.ts`:

```typescript
"chmod-calc": defineAsyncComponent(() => import("./components/ChmodCalcPanel.vue")),
```

`App.vue` 的 `id="network"` 组 tools 数组中添加：

```typescript
{ id: "chmod-calc", name: "chmod 计算器", desc: "Linux 文件权限数字/符号互转" },
```

**Step 4: 类型检查**

Run: `pnpm typecheck`
Expected: PASS

**Step 5: 提交**

```bash
git add apps/desktop/src/components/ChmodCalcPanel.vue apps/desktop/src/tool-registry.ts apps/desktop/src/App.vue apps/desktop/src/bridge/tauri.ts
git commit -m "feat(ui): add ChmodCalcPanel for permission calculator"
```

---

### Task 3.5: 批次 3 集成验证

**Step 1:** `cd apps/desktop/src-tauri && cargo test --lib`
**Step 2:** `pnpm typecheck`
**Step 3:** `pnpm --filter @lazycat/desktop build:web`
**Step 4:** `pnpm test`

Expected: ALL PASS

---

## 批次 4: 时间组 + 加密组

### Task 4.1: 日期计算器 — Rust 后端

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/time.rs`

**Step 1: 写测试**

```rust
#[test]
fn date_diff_basic() {
    let r = execute("date_diff", &json!({"start": "2026-01-01", "end": "2026-02-21"})).unwrap();
    assert_eq!(r["days"], 51);
    assert!(r["hours"].as_u64().unwrap() > 0);
    assert!(r["natural"].as_str().unwrap().contains("1"));
}

#[test]
fn date_diff_same_day() {
    let r = execute("date_diff", &json!({"start": "2026-01-01", "end": "2026-01-01"})).unwrap();
    assert_eq!(r["days"], 0);
}

#[test]
fn date_add_days() {
    let r = execute("date_add", &json!({"date": "2026-02-21", "add": {"days": 30, "hours": 0, "minutes": 0}})).unwrap();
    assert_eq!(r["result"], "2026-03-23");
}

#[test]
fn date_add_negative() {
    let r = execute("date_add", &json!({"date": "2026-02-21", "add": {"days": -10, "hours": 0, "minutes": 0}})).unwrap();
    assert_eq!(r["result"], "2026-02-11");
}
```

**Step 2: 运行测试确认失败**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::time::tests::date_ -- --nocapture`
Expected: FAIL

**Step 3: 实现**

在 `time.rs` 中添加：

- `date_diff(payload)`: 用 `chrono::NaiveDate::parse_from_str` 解析两个日期，计算差值
  - 返回 `{ days, hours, minutes, seconds, natural }` 其中 natural 为 "X年X月X天" 格式
- `date_add(payload)`: 解析日期 + 加减值（days/hours/minutes），用 `chrono::Duration` 计算
  - 返回 `{ result: "YYYY-MM-DD", resultDatetime: "YYYY-MM-DDTHH:MM:SS" }`

在 `execute` match 中添加：

```rust
"date_diff" => date_diff(payload),
"date_add" => date_add(payload),
```

**Step 4: 运行测试确认通过**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::time::tests::date_ -- --nocapture`
Expected: 4 tests PASS

**Step 5: 提交**

```bash
git add apps/desktop/src-tauri/src/tools/time.rs
git commit -m "feat(time): add date_diff and date_add actions"
```

---

### Task 4.2: 日期计算器 — 前端面板

**Files:**

- Create: `apps/desktop/src/components/DateCalcPanel.vue`
- Modify: `apps/desktop/src/tool-registry.ts`
- Modify: `apps/desktop/src/App.vue`
- Modify: `apps/desktop/src/bridge/tauri.ts`

**Step 1: 添加 IPC 通道**

```typescript
"tool:time:date-diff": { domain: "time", action: "date_diff" },
"tool:time:date-add": { domain: "time", action: "date_add" },
```

**Step 2: 创建 DateCalcPanel.vue**

UI 布局（两个功能区用 `el-card` 分隔）：

功能区 1 — 日期间隔：

- 两个 `el-date-picker`（开始日期、结束日期）+ 计算按钮
- 结果区：天数、小时、分钟、秒（grid 展示）+ 自然语言描述

功能区 2 — 日期加减：

- 一个 `el-date-picker` + 三个 `el-input-number`（天/时/分，支持负数）+ 计算按钮
- 结果区：计算后的日期

Script:

- `diffStart`, `diffEnd` ref (Date)
- `addDate` ref, `addDays`/`addHours`/`addMinutes` ref
- `calcDiff()` 调用 `tool:time:date-diff`
- `calcAdd()` 调用 `tool:time:date-add`

**Step 3: 注册组件和侧边栏**

`tool-registry.ts`:

```typescript
"date-calc": defineAsyncComponent(() => import("./components/DateCalcPanel.vue")),
```

`App.vue` 的 `id="calc"` 组 tools 数组中添加：

```typescript
{ id: "date-calc", name: "日期计算器", desc: "日期间隔计算与日期加减" },
```

**Step 4: 类型检查**

Run: `pnpm typecheck`
Expected: PASS

**Step 5: 提交**

```bash
git add apps/desktop/src/components/DateCalcPanel.vue apps/desktop/src/tool-registry.ts apps/desktop/src/App.vue apps/desktop/src/bridge/tauri.ts
git commit -m "feat(ui): add DateCalcPanel for date calculation"
```

---

### Task 4.3: Bcrypt 哈希 — Rust 后端

**Files:**

- Modify: `apps/desktop/src-tauri/Cargo.toml` (添加 `bcrypt` 依赖)
- Modify: `apps/desktop/src-tauri/src/tools/crypto.rs`

**Step 1: 添加 bcrypt 依赖**

在 `Cargo.toml` 的 `[dependencies]` 中添加：

```toml
bcrypt = "0.15"
```

**Step 2: 写测试**

在 `crypto.rs` 测试模块中添加：

```rust
#[test]
fn bcrypt_hash_generates() {
    let r = execute("bcrypt_hash", &json!({"password": "test123", "cost": 4})).unwrap();
    let hash = r["hash"].as_str().unwrap();
    assert!(hash.starts_with("$2b$04$"));
}

#[test]
fn bcrypt_verify_correct() {
    let r = execute("bcrypt_hash", &json!({"password": "test123", "cost": 4})).unwrap();
    let hash = r["hash"].as_str().unwrap();
    let v = execute("bcrypt_verify", &json!({"password": "test123", "hash": hash})).unwrap();
    assert_eq!(v["valid"], true);
}

#[test]
fn bcrypt_verify_wrong() {
    let r = execute("bcrypt_hash", &json!({"password": "test123", "cost": 4})).unwrap();
    let hash = r["hash"].as_str().unwrap();
    let v = execute("bcrypt_verify", &json!({"password": "wrong", "hash": hash})).unwrap();
    assert_eq!(v["valid"], false);
}
```

**Step 3: 运行测试确认失败**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::crypto::tests::bcrypt -- --nocapture`
Expected: FAIL

**Step 4: 实现**

在 `crypto.rs` 顶部添加 `use bcrypt::{hash, verify, DEFAULT_COST};`

添加两个函数：

- `bcrypt_hash(payload)`: 从 payload 取 password 和 cost（默认 12），调用 `bcrypt::hash(password, cost)`
- `bcrypt_verify(payload)`: 从 payload 取 password 和 hash，调用 `bcrypt::verify(password, hash)`

在 `execute` match 中添加：

```rust
"bcrypt_hash" => bcrypt_hash(payload),
"bcrypt_verify" => bcrypt_verify(payload),
```

**Step 5: 运行测试确认通过**

Run: `cd apps/desktop/src-tauri && cargo test --lib tools::crypto::tests::bcrypt -- --nocapture`
Expected: 3 tests PASS

**Step 6: 提交**

```bash
git add apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/src/tools/crypto.rs
git commit -m "feat(crypto): add bcrypt hash and verify actions"
```

---

### Task 4.4: Bcrypt 哈希 — 前端面板

**Files:**

- Create: `apps/desktop/src/components/BcryptPanel.vue`
- Modify: `apps/desktop/src/tool-registry.ts`
- Modify: `apps/desktop/src/App.vue`
- Modify: `apps/desktop/src/bridge/tauri.ts`

**Step 1: 添加 IPC 通道**

```typescript
"tool:crypto:bcrypt-hash": { domain: "crypto", action: "bcrypt_hash" },
"tool:crypto:bcrypt-verify": { domain: "crypto", action: "bcrypt_verify" },
```

**Step 2: 创建 BcryptPanel.vue**

UI 布局（两个功能区用 `el-card` 分隔）：

功能区 1 — 生成哈希：

- 密码输入框 `el-input`
- Cost 因子 `el-input-number` (min=4, max=31, default=12)
- 生成按钮
- 结果显示 `el-input` readonly + 复制按钮

功能区 2 — 验证哈希：

- 密码输入框 `el-input`
- 哈希输入框 `el-input`
- 验证按钮
- 结果显示：匹配（绿色 el-tag success）/ 不匹配（红色 el-tag danger）

Script:

- `hashPassword`, `hashCost`, `hashResult` ref
- `verifyPassword`, `verifyHash`, `verifyResult` ref
- `generateHash()` 调用 `tool:crypto:bcrypt-hash`
- `verifyBcrypt()` 调用 `tool:crypto:bcrypt-verify`

**Step 3: 注册组件和侧边栏**

`tool-registry.ts`:

```typescript
"bcrypt": defineAsyncComponent(() => import("./components/BcryptPanel.vue")),
```

`App.vue` 的 `id="crypto"` 组 tools 数组中添加：

```typescript
{ id: "bcrypt", name: "Bcrypt", desc: "Bcrypt 哈希生成与验证" },
```

**Step 4: 类型检查**

Run: `pnpm typecheck`
Expected: PASS

**Step 5: 提交**

```bash
git add apps/desktop/src/components/BcryptPanel.vue apps/desktop/src/tool-registry.ts apps/desktop/src/App.vue apps/desktop/src/bridge/tauri.ts
git commit -m "feat(ui): add BcryptPanel for bcrypt hash/verify"
```

---

### Task 4.5: 批次 4 集成验证

**Step 1:** `cd apps/desktop/src-tauri && cargo test --lib`
**Step 2:** `pnpm typecheck`
**Step 3:** `pnpm --filter @lazycat/desktop build:web`
**Step 4:** `pnpm test`

Expected: ALL PASS

---

## 最终验证

### Task 5.1: 全量回归

**Step 1:** `cd apps/desktop/src-tauri && cargo test --lib` (全部 Rust 测试)
**Step 2:** `pnpm typecheck` (TypeScript 类型检查)
**Step 3:** `pnpm --filter @lazycat/desktop build:web` (前端构建)
**Step 4:** `pnpm test` (单元测试)
**Step 5:** `pnpm test:e2e` (E2E 测试)
