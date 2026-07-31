# Spotlight `=` 直达计算：无空格快捷触发

> 关联：`2026-05-24-spotlight-keyword-commands-design.md`、`2026-05-17-spotlight-vNext-restructure-design.md`
> 主轴：在 Spotlight 的 QuickCommand 体系中新增一个 `=` 前缀触发条目，行为对齐现有 `calc <expr>`，但**不需要空格分隔**——`=1+1` 直接命中计算器结果卡。为避免劫持开发者高频搜索词，`=` 后紧跟 `=` 或 `>`（即 `==`/`===`/`=>`）时不当作算式、回落普通搜索。最小改动：3 个源码文件 + 1 个测试文件，无后端/IPC/DB 变动。

## 概述

当前 Spotlight 的 `calc` 触发要求 `calc <expr>` 形式（必须空格分隔），对于"想快速算一下"的高频场景有键程成本。本设计以 `=` 作为额外的 QuickCommand 触发前缀，去掉空格约束，与 macOS Spotlight、Raycast、Alfred 等同类工具的 `=` 直达计算行为对齐。

`=` 触发后的渲染、Enter 行为、错误提示与现有 `calc` 一致；仅在「解析触发」一层引入新分支。唯一刻意的差异：本工具面向开发者，`==`/`===`/`=>` 这类等号/箭头操作符既非合法算式（交给 calc 只会得到错误卡），又是常见搜索输入，因此在解析层就把它们放行回普通搜索，而非送入 calc。

## 目标 / 非目标

### 目标

1. 新增 QuickCommand `calc-eq`：以 `=` 作为首字符触发，**不强制空格**。
   - `=1+1` → 命中计算器结果卡，标题 `1+1 = 2`
   - `= 1+1` → 等价于 `=1+1`（首字符仍是 `=`，剩余 trim 后给 calc）
   - `=`（仅一个等号）→ 等价于空 `calc`，进入计算器空态卡
   - `==` / `===` / `=>` → **不触发 calc**，回落普通搜索（等号/箭头操作符既非合法算式，又是开发者高频搜索词）
2. 复用现有 calc 渲染/执行链路（`SpotlightPanel.vue` 内的 `quickCommand.kind === "calc"` 分支），下游无感知。
3. 设置面板「快速命令」分组自动出现 `= 直达计算` 开关项，可独立启用/禁用。
4. 禁用 `calc-eq` 时，`=<text>` 退化为普通搜索；`calc <expr>` 仍可用。

### 非目标 / YAGNI

- 不修改现有 `calc` 解析规则与渲染输出。
- 不引入新的 calc 表达式语法、不扩展 `calculateExpression` 能力。
- 不引入 calc 历史持久化到 Spotlight（计算器历史仍只在 `CalcDraftPanel` 内维护）。
- 不为 `=` 设计独立的结果展示样式（与 `calc` 完全一致，标题里不保留 `=` 字符）。
- 不支持其他无空格前缀（`+1+1` 等不在本次范围）。
- 不做 i18n 触发字符替代（`＝` 全角等号暂不解析，需要时由 calc 内部归一化吃掉）。

## 现状回顾

- `apps/desktop/src/utils/spotlight-query.ts:51` `parseQuickCommand(raw, enabledIds)` 识别 `+ ` 与 `calc`：
  - `+ ` 前缀（含空格强制）→ `{ kind: "todo-create", text }`
  - `^calc(?:\s([\s\S]*))?$` → `{ kind: "calc", text }`（`calc` 单独，或 `calc <expr>`）
- `DEFAULT_ENABLED_QUICK_COMMANDS = new Set(["todo-create", "calc"])`
- `apps/desktop/src/spotlight/types.ts:13` `QuickCommandId = "todo-create" | "calc"`
- `apps/desktop/src/spotlight/quick-commands.ts` 维护 `QUICK_COMMAND_DESCRIPTORS` 数组，描述符含 `id / name / description / trigger / defaultEnabled`
- `apps/desktop/src/components/SpotlightPanel.vue:209` 通过 `parseQuickCommand(query.value, view.value?.enabledQuickCommands)` 计算 `quickCommand`
- 同文件 `:343` `if (quickCommand.value?.kind === "calc") { ... }` 已经处理空态/合法表达式/不完整预览/非法表达式 4 种渲染分支
- `apps/desktop/src/components/settings/SpotlightSettings.vue:43` 通过 `v-for="qc in quickCommands"` 自动渲染整个 `QUICK_COMMAND_DESCRIPTORS`，无需改模板
- `SpotlightConfig.quickCommands: Partial<Record<QuickCommandId, ...>>` 是部分映射，新增 id 后类型与持久化自动兼容，**不需要数据迁移**
- `apps/desktop/src/utils/calc.ts` 的 `calculateExpression` 已经做了中英文标点 / 空格 / `%` / `×÷` 归一化，对 `=1+1` 截取后的 `1+1` 完全兼容

解析优先级（保持不变）：

1. `parseKeywordCommand`（`;` 前缀）
2. `parseQuickCommand`（`+ ` / `calc` / **新增 `=`**）
3. `parseSpotlightQuery`（scope 别名）
4. 全局搜索

## 解析规则

在 `parseQuickCommand` 中，于现有 `+ ` 与 `calc` 分支之间追加 `=` 分支（顺序：`;` keyword 已在外层先消费，所以 `+ ` / `=` / `calc` 三者之间互斥的输入不会有歧义；为可读性放在 `+ ` 之后、`calc` 之前）：

```ts
if (trimmedLeft.startsWith("=")) {
  if (!enabled.has("calc-eq")) return null;
  const rest = trimmedLeft.slice(1).trim();
  // 放行 == / === / => 等开发者搜索词：= 后紧跟 "=" 或 ">" 不视为算式，回落搜索。
  // 这些操作符本就不是合法 calc 输入（送进去只会得到错误卡），分流到搜索更有用。
  if (rest.startsWith("=") || rest.startsWith(">")) return null;
  return { kind: "calc", text: rest };
}
```

行为表：

| 输入                     | rest（slice(1).trim()） | 返回值                              | 渲染结果                                              |
| ------------------------ | ----------------------- | ----------------------------------- | ----------------------------------------------------- |
| `=`                      | `""`                    | `{ kind: "calc", text: "" }`        | 计算器空态卡（标题"计算器"）                          |
| `=1+1`                   | `"1+1"`                 | `{ kind: "calc", text: "1+1" }`     | `1+1 = 2` 结果卡                                      |
| `= 1+1`                  | `"1+1"`                 | `{ kind: "calc", text: "1+1" }`     | 同上                                                  |
| `=(2+3)*4`               | `"(2+3)*4"`             | `{ kind: "calc", text: "(2+3)*4" }` | `(2+3)*4 = 20` 结果卡                                 |
| `=1+`                    | `"1+"`                  | `{ kind: "calc", text: "1+" }`      | `1+ ≈ 1` 预览卡（getCalcPreview 兜底）                |
| `=abc`                   | `"abc"`                 | `{ kind: "calc", text: "abc" }`     | 错误卡（"仅支持数字和 + - \* / ( ) 运算符"）          |
| `==`                     | `"="`                   | `null`                              | 普通搜索（守卫放行）                                  |
| `===`                    | `"=="`                  | `null`                              | 普通搜索（守卫放行）                                  |
| `=>`                     | `">"`                   | `null`                              | 普通搜索（守卫放行）                                  |
| `  =1+1`（前导空格）     | `"1+1"`                 | `{ kind: "calc", text: "1+1" }`     | 同 `=1+1`，因 `trimmedLeft = raw.replace(/^\s+/, "")` |
| 禁用 `calc-eq` 时 `=1+1` | —                       | `null`                              | 普通搜索                                              |

**关键设计点**：

- 返回的 `kind` 仍是 `"calc"`，而非新建 `"calc-eq"` kind——只有"触发来源"不同，下游不需要分支。
- enabled 判断使用 `"calc-eq"` id，所以 `=` 触发可独立禁用，与 `calc` 开关解耦。
- `=` 后的内容统一 `trim()`，与 calc 自身归一化协作（calc 内部还会再吃掉空格）。
- **blocklist 守卫只拦 `=`/`>` 两个后继字符**：选择 blocklist（"仅排除"）而非 allowlist（"仅放行数字/括号"）是为了最小化与 `calc` 的行为分歧——除 `==`/`===`/`=>` 外，`=<任意>` 与 `calc <任意>` 行为完全一致（含 `=abc` 仍走 calc 错误卡）。守卫只针对实测高频且对 calc 无意义的操作符，符合 KISS / YAGNI。

## 数据契约变更

### 类型扩展

`apps/desktop/src/spotlight/types.ts`：

```ts
export type QuickCommandId = "todo-create" | "calc" | "calc-eq";
```

### 描述符追加

`apps/desktop/src/spotlight/quick-commands.ts`：

```ts
{
  id: "calc-eq",
  name: "= 直达计算",
  description: '以 "=" 前缀直接计算表达式，无需空格分隔，例如 =1+1',
  trigger: { type: "prefix", value: "=" },
  defaultEnabled: true,
}
```

### 默认启用集

`apps/desktop/src/utils/spotlight-query.ts`：

```ts
const DEFAULT_ENABLED_QUICK_COMMANDS = new Set<QuickCommandId>(["todo-create", "calc", "calc-eq"]);
```

### 配置持久化

`SpotlightConfig.quickCommands` 类型为 `Partial<Record<QuickCommandId, SpotlightConfigQuickCommandOverride>>`，新增 id 自动兼容。已持久化的旧配置（仅有 `todo-create` / `calc` key）读取后 `calc-eq` 未出现 → fallback 到 `defaultEnabled: true`。

**不需要 schema 迁移、不需要 default config 修改、不需要 store 升级**。

## 渲染链路

`SpotlightPanel.vue` 现有 `quickCommand.value?.kind === "calc"` 分支（约 `:343`）覆盖：

- `text === ""` → 空态卡（`itemId: "calc:empty"`）
- `text` 合法 → 结果卡（`itemId: "calc:<text>"`，标题 `{text} = {displayValue}`）
- `text` 不完整但 `getCalcPreview` 有兜底 → 预览卡（`itemId: "calc:<text>:preview"`）
- `text` 非法 → 错误卡（`itemId: "calc:<text>:error"`）

**`itemId` 以 `text` 为后缀，不带 `=` 前缀**，因此 `calc 1+1` 与 `=1+1` 命中同一 `itemId`，列表不会跳动。这是复用现有渲染的副作用收益。

执行链路（`commitDefault` 中 `payload.quickCommand === "calc"` 分支）同样无需改动。

## 设置面板

`SpotlightSettings.vue` 中"快速命令"分组的模板：

```vue
<div v-for="qc in quickCommands" :key="qc.id" class="quick-command-row">
  <el-switch
    :model-value="resolveQuickEnabled(qc.id, qc.defaultEnabled)"
    @update:model-value="(v: boolean) => onToggleQuickCommand(qc.id, v)"
  />
  <!-- name / description 自动从描述符读取 -->
</div>
```

`quickCommands` 直接来自 `QUICK_COMMAND_DESCRIPTORS`，所以新增条目后**模板零改动**，会在 `+ 新建任务` 和 `calc 计算器` 之后追加 `= 直达计算` 一行。

开关切换通过既有 `onToggleQuickCommand` 写入 `config.quickCommands["calc-eq"] = { enabled }`，跨窗口广播复用 vNext 的 `subscribeConfigBroadcast` 通路，即改即生效。

## 边界与权衡

1. **`=` 与作用域前缀的冲突**：`parseSpotlightQuery` 要求"前缀 + 空格"，因此 `=` 不会落入 scope 解析；`parseKeywordCommand` 要求 `;` 起头，也不冲突。三个解析函数顺序固定，互斥。
2. **`=` 作为普通搜索字符**：极少有搜索词以 `=` 开头（路径、英文短语、URL 等都不会）。守卫已放行 `==`/`===`/`=>`；剩余以字母开头的 `=word`（如 `=COUNT()` 类 Excel 函数）仍会进 calc 并显示错误卡——此类在开发者工具箱中较罕见，可接受，必要时关闭 `calc-eq` 开关。
3. **全角 `＝`（U+FF1D）**：本期不支持，保持 ASCII 优先。注意：解析层 `startsWith("=")` 只认半角 `=`，`＝1+1` 根本不会进 calc 分支，因此"靠 calc 内部归一化吃掉全角等号"**无效**——calc 分支压根不会被触发。若未来要支持，必须在**解析层**补 `startsWith("=") || startsWith("＝")`（并对全角做半角归一化后再喂 calc），而非依赖 calc 内部。
4. **`==` / `===` / `=>` 多符号开头**：由 blocklist 守卫在解析层放行，回落普通搜索（不再进 calc 错误卡）。这是有意为之：本工具面向开发者，相等/箭头操作符是高频搜索输入，且对 calc 无意义。`=<` / `=!` 等更冷门的操作符前缀未单独拦截，会进 calc 走错误卡，可接受。
5. **与 `+ ` 的对称性**：`+ ` 强制空格是因为 `+1` 可能是合法搜索/算式，且 todo 文本通常较长；`=` 后 100% 是算式，无空格更符合手感。两者差异有意保留。
6. **`calc-eq` 命名**：内部 id，不展示给用户；用户可见的是 description 中的 `"=" 前缀`。命名比 `calc-equal` / `eq-calc` 更紧凑，与既有 `todo-create` 风格一致。
7. **不动 `RESERVED_TOKENS`**：`config-store.ts` 的 `RESERVED_TOKENS = {"+", "calc"}` 用于拦截 scope 别名冲突。无需加入 `=`——别名校验先过 `ALIAS_PATTERN = /^[a-zA-Z0-9_-]{1,16}$/`，`=` 本就无法成为合法别名（与 `+` 同理，`+` 留在集合里仅为更友好的报错文案）。`=` 不展示在别名输入也不会与之冲突，加它只是冗余。

## 测试计划

`apps/desktop/src/utils/spotlight-query.test.ts` 新增用例（与 `parseQuickCommand` 现有测试同一 describe）：

```ts
it("= 前缀直达计算，不需要空格", () => {
  expect(parseQuickCommand("=1+1")).toEqual({ kind: "calc", text: "1+1" });
});

it("= 后允许空格，与 =<expr> 等价", () => {
  expect(parseQuickCommand("= 1+1")).toEqual({ kind: "calc", text: "1+1" });
});

it("单独 = 等价于空 calc", () => {
  expect(parseQuickCommand("=")).toEqual({ kind: "calc", text: "" });
});

it("= 前缀容忍前导空格", () => {
  expect(parseQuickCommand("  =1+1")).toEqual({ kind: "calc", text: "1+1" });
});

it("= 非法表达式仍走 calc 分支，由渲染层报错", () => {
  expect(parseQuickCommand("=abc")).toEqual({ kind: "calc", text: "abc" });
});

it("= 后接括号/小数等仍直达计算", () => {
  expect(parseQuickCommand("=(2+3)*4")).toEqual({ kind: "calc", text: "(2+3)*4" });
});

it("== / === / => 不当作算式，回落搜索", () => {
  expect(parseQuickCommand("==")).toBeNull();
  expect(parseQuickCommand("===")).toBeNull();
  expect(parseQuickCommand("=>")).toBeNull();
  expect(parseQuickCommand("=> item")).toBeNull();
});

it("禁用 calc-eq 时 = 不解析", () => {
  const enabled = new Set<QuickCommandId>(["todo-create", "calc"]);
  expect(parseQuickCommand("=1+1", enabled)).toBeNull();
});

it("启用 calc-eq 但禁用 calc，= 仍生效", () => {
  const enabled = new Set<QuickCommandId>(["calc-eq"]);
  expect(parseQuickCommand("=1+1", enabled)).toEqual({ kind: "calc", text: "1+1" });
  expect(parseQuickCommand("calc 1+1", enabled)).toBeNull();
});
```

`apps/desktop/src/spotlight/config-store.test.ts`：现有 `mergeView` 默认值用例（`uses descriptor defaults when config is empty`）追加一行断言，锁定默认启用契约：

```ts
expect(view.enabledQuickCommands.has("calc-eq")).toBe(true);
```

回归：现有 `parseQuickCommand` 测试（`+ ` / `calc 1+1` / `calc` 单独 / `calcXXX` 拒绝）全部应继续通过。`config-store.test.ts` 现有断言均用 `.has()`（非精确集合/长度比较），新增 `calc-eq` 不会破坏既有用例。

## 影响面

| 文件                                              | 改动                                                                                                |
| ------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| `apps/desktop/src/spotlight/types.ts`             | `QuickCommandId` 联合类型加 `"calc-eq"`                                                             |
| `apps/desktop/src/spotlight/quick-commands.ts`    | `QUICK_COMMAND_DESCRIPTORS` 追加 `calc-eq` 描述符                                                   |
| `apps/desktop/src/utils/spotlight-query.ts`       | `DEFAULT_ENABLED_QUICK_COMMANDS` 加 `"calc-eq"`；`parseQuickCommand` 加带 blocklist 守卫的 `=` 分支 |
| `apps/desktop/src/utils/spotlight-query.test.ts`  | 新增 9 条 `=` 解析测试（含 `==`/`===`/`=>` 放行）                                                   |
| `apps/desktop/src/spotlight/config-store.test.ts` | 默认值用例追加 1 行 `calc-eq` 默认启用断言                                                          |

**不动**：`SpotlightPanel.vue`（渲染/执行/Enter 链路无变化）、`SpotlightSettings.vue`（模板已 `v-for` 自动覆盖）、`config-store.ts`（结构兼容，`mergeView` 已按描述符 + `defaultEnabled` 解析）、`calc.ts`（计算逻辑不变）、任何 Rust 端代码。

## 验证

1. `pnpm test`（含新增 9 条解析测试 + config-store 默认启用断言）
2. `pnpm typecheck`
3. `pnpm --filter @lazycat/desktop build:web`
4. 人工验证（启动 dev）：
   - `=1+1` 显示结果卡，Enter 复制 `2`
   - `=` 单独显示计算器空态卡
   - `= 1+1` 等价 `=1+1`
   - `==` / `=>` 不显示计算卡，回落普通搜索
   - 设置面板「快速命令」分组出现 `= 直达计算` 开关
   - 关闭 `= 直达计算`，`=1+1` 退化为普通搜索；`calc 1+1` 仍可用
   - 关闭 `calc` 但开 `= 直达计算`：`=1+1` 仍生效

## 风险与回滚

- **风险等级**：低。改动局限于解析层与类型，无数据迁移、无后端、无 IPC、无 UI 模板改动。
- **回滚**：删除 4 个文件的对应改动即可。`SpotlightConfig.quickCommands` 中若已写入 `calc-eq` key，旧版读到会忽略未知 id（`Partial<Record>` 容忍），不会崩。
