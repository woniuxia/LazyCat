# Spotlight Keyword Commands:用户自定义关键字命令

> 上一版:`2026-05-17-spotlight-vNext-restructure-design.md`
> 主轴:在 vNext 的描述符化基础上,新增「关键字命令」(KeywordCommand)第三类指令体系,既提供 8-10 个开箱即用的内置项(`;ip` / `;uuid` / `;ts` / `;jwt <token>` 等),也允许用户自定义跳工具透传参数、查 Vault tag、查 Snippet tag 三类命令。本期面向「个人全栈开发者」轻量增量,不引入插件运行时与脚本执行能力。

## 概述

vNext 已经把 provider 元信息抽成描述符,并允许用户编辑 scope 别名与 quick command 开关。但当前体系下:

- Quick command 只有开发者硬编码的 2 个(`calc` / `+ `),且 ID 是 union 类型,用户无法新增
- Provider scope 别名只能改"已存在 provider 的前缀",不能定义全新的"动作"
- 没有"内联结果项"机制(`;ip` 这种不打开任何工具、直接在 Spotlight 内展示数据的能力)
- 没有"keyword + 参数透传"的语法(类似 `;jwt <token>` 直达 JWT 工具并预填)

本版引入 KeywordCommand 作为独立的第三类指令体系,解决上述空白。所有指令统一以 `;` 前缀触发,与现有 scope 别名(无前缀)和 quick command(`+ ` / `calc`)互不冲突。

## 目标 / 非目标

### 目标

1. 引入 `;keyword [args]` 语法,与现有 scope / quick command 解析互不干扰
2. 提供 8 个内置 KeywordCommand,开箱即用,默认全部启用:
   - 显示值类:`;ip` / `;uuid` / `;ts` / `;hash <text>`
   - 直达工具类:`;b64 <text>` / `;jwt <token>` / `;regex <text>` / `;color <hex>`
3. 支持用户在 SpotlightSettings 中新增、编辑、删除、启用/禁用自定义 KeywordCommand,三种类型:
   - `open-tool`:跳工具 + 透传参数(复用 `spotlight_pick`)
   - `vault-tag`:列出 Vault 中含指定 tag 的条目(复用 vault provider 的复制密码流程)
   - `snippet-tag`:列出 Snippet 中含指定 tag 的条目(复用 snippet 工具的跳转/复制能力)
4. 内置项启用态与用户自定义项写入 `user_settings.spotlight_config_v1.keywordCommands`,与 vNext 的配置结构同 key 持久化
5. 配置变更走 vNext 已有的跨窗口广播,即改即生效

### 非目标 / YAGNI

- 不支持多参数语法(`;k <a> <b>` 不解析,仅整段参数透传)
- 不支持 Snippet 模板渲染(如 `;curl-get {url}` 变量替换)二期再做
- 不支持自定义触发前缀(固定 `;`,后续可演进)
- 不支持"执行外部命令"类型(安全风险大)
- 不支持中文 keyword(避免与查询文字混淆,与 alias 校验规则一致)
- 不为内置项提供"自定义行为"的能力(只能启用/禁用)
- 不引入 KeywordCommand 的导入/导出/分享
- 不修改现有 `calc` / `+ ` 的语法或迁移到新架构

## 现状回顾

- Spotlight 解析层 `apps/desktop/src/utils/spotlight-query.ts` 当前包含两个解析函数:
  - `parseQuickCommand(raw, enabledIds)`:识别 `calc` 和 `+ `
  - `parseSpotlightQuery(raw, aliasMap)`:识别 scope 别名前缀
- `SpotlightPanel.vue` 在 `parsed` / `quickCommand` computed 中按 `quickCommand → scope → 全局` 顺序消费
- 配置存储位置 `user_settings.spotlight_config_v1`,结构见 `SpotlightConfig`(`apps/desktop/src/spotlight/types.ts:102`)
- `spotlight_pick` 在 `apps/desktop/src-tauri/src/main.rs:990`,签名为 `(target, text?, source?, itemId?)`,已支持文本透传
- `useClipboardSuggestion.setPendingToolInput` 是面板端消费透传 text 的入口
- Vault tag 数据在 `vault_entry_tags` 表,`tool:vault:meta-list` 返回的 `VaultMetaEntry` 已含 `tags: string[]`
- Snippet tag 数据在 `snippet_entry_tags` 表,Snippet 工具已有按 tag 过滤的查询能力(`v2_tag_stats`)

## 数据契约

### KeywordCommand 描述符(运行时合并产物)

```ts
type KeywordCommandKind = "open-tool" | "show-value" | "vault-tag" | "snippet-tag";

interface KeywordCommandDescriptor {
  id: string;                // 内置: "ip" / "uuid" / ... ; 自定义: 由 store 生成的 nanoid
  keyword: string;           // 不含 ";" 前缀,如 "ip" / "jwt" / "myapi"
  name: string;              // 展示名,如 "本机 IP"
  description: string;       // 展示子标题
  kind: KeywordCommandKind;
  origin: "builtin" | "custom";

  // 仅 open-tool 用
  toolId?: string;
  forwardArgs?: boolean;

  // 仅 vault-tag / snippet-tag 用
  targetTag?: string;

  // 仅 show-value 用(内置只读)
  valueProducer?: KeywordValueProducerId;

  defaultEnabled: boolean;
}

type KeywordValueProducerId =
  | "local-ip"
  | "uuid-v4"
  | "timestamp-now"
  | "hash-text";
```

### 配置层扩展

`SpotlightConfig` 在 vNext 基础上追加一个字段(版本号不动,字段缺失时回落空):

```ts
interface SpotlightConfig {
  version: 1;
  providers: Partial<Record<SpotlightProviderId, SpotlightConfigProviderOverride>>;
  quickCommands: Partial<Record<QuickCommandId, SpotlightConfigQuickCommandOverride>>;
  // 新增
  keywordCommands?: {
    builtins?: Partial<Record<string, { enabled: boolean }>>;  // key 为内置 id
    custom?: Array<{
      id: string;
      keyword: string;
      name: string;
      description: string;
      kind: "open-tool" | "vault-tag" | "snippet-tag";
      toolId?: string;
      forwardArgs?: boolean;
      targetTag?: string;
      enabled: boolean;
    }>;
  };
}
```

### SpotlightView 扩展

```ts
interface SpotlightView {
  providers: ResolvedProvider[];
  aliasMap: Map<string, SpotlightProviderId>;
  enabledQuickCommands: Set<QuickCommandId>;
  quickCommands: QuickCommandDescriptor[];
  // 新增
  keywordCommands: KeywordCommandDescriptor[];   // 合并 builtins + custom,仅 enabled=true
  keywordIndex: Map<string, KeywordCommandDescriptor>;  // keyword(小写) -> descriptor
}
```

### 解析输出

```ts
interface KeywordCommandInvocation {
  kind: "keyword";
  command: KeywordCommandDescriptor;
  args: string;   // 去掉 keyword 和空格后的纯参数文本
}
```

## 触发语法与解析层

### 语法规则

- 必须以 `;` 开头(单字符)
- `;keyword` 或 `;keyword <args>` 都合法
- `;keyword` 与 `;keyword ` 等价(无参数)
- `;<空格>` 与 `;` 单独输入:视为无效输入,等价空查询(不解析为 keyword)
- keyword 大小写不敏感(存与匹配统一 `toLowerCase()`)
- args 保留用户原始大小写与空格(只去首尾)
- keyword 字符集:`[a-zA-Z0-9_-]+`,最大长度 24(与 alias 16 不同,因为 keyword 命名空间更大)

### 解析函数

新增 `parseKeywordCommand`:

```ts
export function parseKeywordCommand(
  raw: string,
  index?: Map<string, KeywordCommandDescriptor>,
): KeywordCommandInvocation | null;
```

- 第二参数为运行时启用的 keyword index,默认空 Map(测试用)
- 不匹配时返回 null

### 解析优先级

在 `SpotlightPanel.vue` 的 `parsed` / `commandSlot` 计算链中按此顺序:

```
parseKeywordCommand(raw, view.keywordIndex)    新增
   ↓ 未命中
parseQuickCommand(raw, view.enabledQuickCommands)    现有
   ↓ 未命中
parseSpotlightQuery(raw, view.aliasMap)    现有
```

Keyword 必须前置于 quick command,否则极端情况下用户自定义的 keyword `+x` 会被误判;但当前 keyword 必须 `;` 起头,与 `+ ` 互斥,即使顺序对调也不会冲突。前置仅为防御性约定。

## 内置 KeywordCommand 集

| keyword | kind | producer/target | 行为 | 备注 |
|---------|------|-----------------|------|------|
| `;ip` | show-value | `local-ip` | 列出本机所有网卡的 IPv4/IPv6,Enter 复制单项 | 需要 Rust 端新增 command |
| `;uuid` | show-value | `uuid-v4` | 一次生成 5 个 UUID v4,Enter 复制单项 | 纯前端 `crypto.randomUUID()` |
| `;ts` | show-value | `timestamp-now` | 列出当前时间的:Unix 秒、Unix 毫秒、ISO 8601、RFC 3339、本地友好格式 | 纯前端 |
| `;hash <text>` | show-value | `hash-text` | 对参数计算 MD5/SHA-1/SHA-256,4 行展示,Enter 复制选中行 | 调现有 `tool:hash:*` 通道,无 args 时显示提示 |
| `;b64 <text>` | open-tool | `tool:base64` | 跳 Base64 工具并预填 args 到输入框 | 复用 `spotlight_pick` |
| `;jwt <token>` | open-tool | `tool:jwt` | 跳 JWT 工具并预填 Token | 同上 |
| `;regex <text>` | open-tool | `tool:regex` | 跳正则工具,在测试文本里预填 args | 同上 |
| `;color <hex>` | open-tool | `tool:color` | 跳颜色工具并预填 | 同上 |

具体 tool ID 实施时与 `App.vue` 的 sidebarItems 对齐;若 ID 不一致以 sidebar 为准。

### 内置项的执行链路

**show-value 类**:不走 provider,直接在 SpotlightPanel 渲染层产出"内联结果项"。结果项是 `SpotlightItem` 形态但 `providerId="__keyword__"`,渲染层识别此 providerId 跳过 fuzzy 搜索流程,直接展示并响应 Enter 复制。

**open-tool 类**:用 `invoke("spotlight_pick", { target: toolId, text: args, source: "keyword" })`,与 clipboard-suggestion 链路一致,目标面板的 `watchPendingInput` 自动消费。

### 内置项的"找不到目标"降级

- 工具 ID 在 sidebar 中不存在(用户自定义菜单显隐关掉了某工具):展示"目标工具已隐藏,前往菜单设置启用"提示,Enter 跳菜单设置面板
- show-value 类计算失败(如 `;hash` 无参数 / 网卡读取失败):降级展示提示,不阻塞其它候选

## 用户自定义 KeywordCommand

### 自定义命令的能力范围

| kind | 用户填写字段 | 行为 |
|------|--------------|------|
| `open-tool` | keyword / name / description / toolId(下拉选择)/ forwardArgs(布尔) | 跳目标工具,forwardArgs=true 时透传 args |
| `vault-tag` | keyword / name / description / targetTag(文本输入,匹配 vault tag) | 列出 Vault 中含该 tag 的条目,Enter 走 vault provider 标准复制密码流程 |
| `snippet-tag` | keyword / name / description / targetTag | 列出 Snippet 中含该 tag 的条目,Enter 默认动作复制 code,备选动作跳 Snippet 工具 |

### vault-tag / snippet-tag 的内联渲染

- 解析为 keyword 后,直接调用对应 prefetch 函数(过滤 tag),把结果包成临时 `SpotlightItem[]` 显示在结果列表
- Vault tag 命中条目:复用 `vault.ts` 的 `copyPasswordFlow`(走 `ctx.requestMasterPassword`)
- Snippet tag 命中条目:新增 `;`-mode 的 snippet 内联渲染,默认动作 `invokeToolByChannel("tool:snippets:copy", { id })`(实施时校验通道名),备选跳 Snippet 工具
- 命中 0 条:展示"未找到含 tag X 的条目",Enter 跳目标工具

### 用户输入的参数语法

- `;keyword` 无 args:vault-tag / snippet-tag 列全部命中
- `;keyword <text>`:vault-tag / snippet-tag 把 text 作为二次模糊筛选(在 title / subtitle 上跑 fuzzy match)
- `open-tool` 类:无 args 仍跳工具,args 透传为空字符串(目标面板自行处理)

## 组件拆分

### 1. `src/spotlight/keyword-commands.ts`(新增)

- 导出内置 `BUILTIN_KEYWORD_COMMANDS: KeywordCommandDescriptor[]`(8 个)
- 导出 `resolveKeywordView(builtinOverrides, customList): { commands, index }`
- 导出 `validateCustomKeyword(input, existingKeywords): { ok: boolean; error?: string }`,校验:
  - 字符集 `[a-zA-Z0-9_-]+`、长度 1-24
  - 不与已启用 keyword 重复
  - 不与内置 keyword 重复
- 导出 value producer 实现(`resolveLocalIp` / `resolveUuid` / `resolveTimestamp` / `resolveHash`),返回 `SpotlightItem[]`

### 2. `src/spotlight/config-store.ts`(改造)

- 合并 `keywordCommands` 字段进 `currentView`,产出 `view.keywordCommands` / `view.keywordIndex`
- `saveConfig` 在校验阶段调用 `validateCustomKeyword`,失败抛错由 UI 显示行内提示
- subscribe 通知不变,UI 重读新 view

### 3. `src/utils/spotlight-query.ts`(扩展)

- 新增 `parseKeywordCommand(raw, index?)`,纯函数,与现有解析函数同模式
- 增加单测覆盖:`;ip` / `;jwt token` / `;` 单字符 / 中文 keyword 拒绝 / 大小写不敏感

### 4. `src/spotlight/keyword-resolver.ts`(新增)

- 入口 `resolveKeywordInvocation(invocation, ctx): Promise<{ items: SpotlightItem[] } | { errorMessage: string }>`
- show-value:调对应 producer
- open-tool:产出一条"建议项",providerId=`__keyword__`,payload 含 `toolId / args`
- vault-tag:调 `vault provider` 的 prefetch,按 tag 过滤
- snippet-tag:新增 `tool:snippets:list-by-tag` 通道(若不存在则用现有 list + 前端过滤)

### 5. `src-tauri/src/tools/system.rs` 或新增 `src-tauri/src/tools/network_info.rs`

- 新增通道 `tool:system:local-ips`,返回 `{ interfaces: Array<{ name, ipv4: string[], ipv6: string[] }> }`
- 用 `local-ip-address` crate 或 `std::net` + 系统调用实现;Windows 优先 IPv4
- 走 `CHANNEL_MAP` + `execute_tool` 标准分发

### 6. `src/components/SpotlightPanel.vue`(改造)

- `parsed` 计算链加 `parseKeywordCommand`,放在第一位
- `results` 计算在 keyword 命中时:
  - show-value / open-tool:直接渲染 keyword-resolver 产出的 items
  - vault-tag / snippet-tag:渲染 resolver 异步产出的 items(加 loading 态)
- Enter 执行:open-tool 走 `spotlight_pick`,show-value 走"复制单行",vault-tag / snippet-tag 走对应 provider 的 defaultAction
- 在 placeholder 文案中追加一段"试试 `;ip` `;uuid` `;jwt <token>`"提示

### 7. `src/components/settings/SpotlightSettings.vue`(改造)

- 新增第 4 个 section「关键字命令」(KeywordCommands)
- 顶部展示内置项列表,每行可启用/禁用,不可编辑/删除
- 中部为自定义项列表,行内编辑、删除、启用/禁用
- 底部「+ 添加」按钮,弹出 KeywordCommandEditor 对话框
- 「恢复默认」复用 vNext 已有按钮,扩展为同时重置 keywordCommands 字段

### 8. `src/components/settings/KeywordCommandEditor.vue`(新增)

- 对话框形态,字段:keyword / name / description / kind / toolId(open-tool) / forwardArgs(open-tool) / targetTag(vault-tag/snippet-tag)
- kind 切换时显示不同字段集
- 实时校验 keyword 合法性,失焦时显示具体错误
- 保存调 `configStore.saveConfig(next)`

### 9. `src/spotlight/types.ts`(扩展)

- 导出 `KeywordCommandKind` / `KeywordCommandDescriptor` / `KeywordValueProducerId` / `KeywordCommandInvocation`
- `SpotlightView` 加 `keywordCommands` / `keywordIndex`
- `SpotlightConfig` 加可选 `keywordCommands` 字段

## 数据流

### 启动序列

1. `configStore.ensureLoaded()` 读 `spotlight_config_v1`
2. 合并 builtins 默认值 + 用户 `keywordCommands.builtins` 覆盖 + `keywordCommands.custom` 列表
3. 校验自定义项(过滤掉非法或空 keyword 的项)
4. 产出 `view.keywordCommands` 和 `view.keywordIndex`

### 查询解析序列

1. 用户输入 `;ip`
2. `parseKeywordCommand("ip", index)` → 命中 builtin `ip` descriptor
3. `keyword-resolver.resolveKeywordInvocation({ command, args: "" })` → 调 `resolveLocalIp()`
4. `tool:system:local-ips` 返回 5 个网卡 IP → 包成 5 条 `SpotlightItem`
5. SpotlightPanel 渲染 5 行,Enter 复制选中行 IP,toast「已复制 192.168.1.42」

### 自定义项添加序列

1. SpotlightSettings → 「+ 添加」 → KeywordCommandEditor 弹出
2. 用户填写 keyword="wifi" / kind="vault-tag" / targetTag="wifi"
3. 失焦校验:keyword 合法、不重名
4. 保存 → `configStore.saveConfig(next)` → 写库 → 广播 `spotlight-config-changed`
5. Spotlight 窗口监听到事件 → `ensureLoaded(forceReload=true)` → keywordIndex 更新
6. 用户输入 `;wifi` 立即生效

## 错误与冲突处理

### Keyword 冲突

- 自定义 keyword 与内置 keyword 同名:拒绝,提示「'ip' 是内置命令,不能覆盖」
- 自定义 keyword 互相同名(同时启用):后保存的拒绝,提示「'wifi' 已被另一个命令占用」
- 内置 keyword 与用户禁用过的自定义同名:允许,但禁用的项不参与冲突检测(只检测启用项)

### Keyword 与 scope alias / quick command 的命名空间

- KeywordCommand 必须 `;` 前缀,scope alias 必须"裸 + 空格",quick command 是 `+ ` 或 `calc <expr>`
- 三者前缀互不重叠,无需额外冲突校验
- 但 keyword 字符集与 alias 一致(`[a-zA-Z0-9_-]+`),不允许中文 keyword

### 配置版本不兼容

- 沿用 vNext 已有的整体回落策略
- `keywordCommands` 字段缺失时回落空对象 → 启用 builtins 全集 / 无自定义
- 自定义项中单条非法(keyword 不合法 / 缺字段)→ 跳过该条,其它项不受影响

### 运行时降级

- `tool:system:local-ips` 失败:`;ip` 显示「网卡信息读取失败」单项,Enter 无动作
- `tool:hash:*` 失败:`;hash` 显示「哈希计算失败」单项
- `;b64 <text>` 跳工具失败:沿用 `spotlight_pick` 现有错误路径
- vault-tag 命中但 Vault 未解锁:复用 vault provider 的 `requestMasterPassword` 弹窗
- vault-tag / snippet-tag 命中 0 条:展示"未找到含 tag X 的条目"单项,Enter 跳目标工具

### 并发与重入

- show-value 类的 producer 在用户连续输入时可能产生竞态(`;ts` → `;tsx`):
  - resolver 内部用最新 invocation 的 nonce,过期结果丢弃
- 自定义项的 prefetch(vault-tag / snippet-tag)用同样的 nonce 守卫

## 改动文件清单

| 文件 | 改动类型 | 说明 |
|------|----------|------|
| `apps/desktop/src/spotlight/types.ts` | 修改 | 新增 KeywordCommand 相关类型,扩展 `SpotlightView` / `SpotlightConfig` |
| `apps/desktop/src/spotlight/keyword-commands.ts` | 新增 | 内置 KeywordCommand 集 + value producers + 校验函数 |
| `apps/desktop/src/spotlight/keyword-resolver.ts` | 新增 | 把 invocation 解析为 SpotlightItem 列表 |
| `apps/desktop/src/spotlight/config-store.ts` | 修改 | 合并 keywordCommands 字段进 view,校验自定义项 |
| `apps/desktop/src/utils/spotlight-query.ts` | 修改 | 新增 `parseKeywordCommand` |
| `apps/desktop/src/utils/spotlight-query.test.ts` | 修改 | 新增 keyword 解析单测 |
| `apps/desktop/src/spotlight/keyword-resolver.test.ts` | 新增 | resolver 与 producer 单测(IP / UUID / hash / vault-tag mock) |
| `apps/desktop/src/components/SpotlightPanel.vue` | 修改 | 解析链加 keyword;results 渲染 keyword items;placeholder 提示 |
| `apps/desktop/src/components/settings/SpotlightSettings.vue` | 修改 | 新增「关键字命令」section |
| `apps/desktop/src/components/settings/KeywordCommandEditor.vue` | 新增 | 自定义命令编辑对话框 |
| `apps/desktop/src/bridge/tauri.ts` | 修改 | 新增 `tool:system:local-ips` 通道映射 |
| `apps/desktop/src-tauri/src/tools/system.rs` | 新增或扩展 | `local-ips` action,返回网卡列表;若已有 system 域则追加 action |
| `apps/desktop/src-tauri/src/tools/mod.rs` | 修改 | 注册 system 模块新 action |
| `apps/desktop/src-tauri/Cargo.toml` | 修改 | 引入 `local-ip-address` crate(或纯 std 实现) |

## 验证

### 自动化

- `pnpm typecheck`
- `pnpm test`(覆盖 `spotlight-query.test.ts` + 新增 `keyword-resolver.test.ts` + `keyword-commands.test.ts`)
- `pnpm --filter @lazycat/desktop build:web`

### 手测清单

1. **内置 show-value 类**
   - `;ip` → 列出当前所有网卡 IPv4/IPv6,Enter 复制选中,toast 提示
   - `;uuid` → 列 5 个 UUID,Enter 复制
   - `;ts` → 列 Unix 秒/毫秒/ISO/RFC3339/本地友好,Enter 复制
   - `;hash hello` → MD5/SHA-1/SHA-256 三行,Enter 复制选中
   - `;hash`(无 args)→ 提示行,Enter 无动作

2. **内置 open-tool 类**
   - `;b64 SGVsbG8=` → 跳 Base64 工具,输入框已预填 `SGVsbG8=`
   - `;jwt eyJhbGc...` → 跳 JWT 工具,Token 已预填
   - `;regex \\d+` → 跳正则工具,正则输入已预填
   - `;color #ff5722` → 跳颜色工具,颜色值已预填

3. **冲突与降级**
   - `;`(单字符)→ 不匹配 keyword,视为空查询
   - `;notexist` → 不命中任何 keyword,Spotlight 显示「未匹配的关键字 ;notexist」并提供「在工具中搜索」备选
   - 内置 `;ip` 启用、读网卡失败 → 显示「网卡读取失败」单项,不崩溃
   - `;wifi`(自定义 vault-tag)、Vault 未解锁 → 弹主密码框,通过后展示条目列表

4. **用户自定义**
   - 添加 `;myapi` kind=snippet-tag targetTag=api → 输入 `;myapi` 列出 Snippet 中 tag=api 的条目,Enter 复制 code
   - 添加 `;myb64` kind=open-tool toolId=base64 forwardArgs=true → `;myb64 hello` 跳 Base64 + 预填
   - 添加 `;ip`(冲突)→ 编辑器拒绝,提示「'ip' 是内置命令」
   - 添加重复自定义 `;wifi` → 拒绝
   - 添加 `;中文` → 拒绝(字符集校验)
   - 添加 `;a-b_c1` → 通过(合法字符集)
   - 禁用内置 `;uuid` → 输入 `;uuid` 不再命中,走默认未匹配提示

5. **配置广播**
   - 主窗口设置面板添加 `;myapi` → Spotlight 窗口已打开 → 监听到事件自动刷新 → 立即可用
   - 主窗口编辑某内置启用态 → Spotlight 同步生效
   - 关闭再打开 Spotlight 窗口 → 通过 window-shown 兜底拉到最新

6. **回归保护**
   - 全新数据库 / 老数据库不动设置 → 8 个内置默认启用,行为与本期一致
   - 现有 `calc 1+2` / `+ 写周报` 不受影响
   - 现有 scope alias(`t xxx` / `v xxx` 等)不受影响

## 关键风险与对策

| 风险 | 对策 |
|------|------|
| 内置 keyword 与未来 alias 自定义冲突(用户把 alias 改成 `ip`) | alias 必须"裸 + 空格",keyword 必须 `;` 起头,前缀互斥,不会冲突 |
| `tool:system:local-ips` 在不同 Windows 网卡配置下行为差异 | 实现时手测主流场景:有线、WiFi、虚拟网卡(WSL/Docker)、VPN;失败静默降级 |
| `local-ip-address` crate 引入新依赖增加包体积 | crate 体积极小(< 50KB),可接受;若反对则用纯 `std::net` + winapi 实现,工作量略增 |
| Snippet 工具没有"按 tag 列条目"的现成通道 | 实施前先 grep 确认;无则用 `tool:snippets:list` 拉全集 + 前端过滤,数据量可承受 |
| 用户自定义太多 keyword 导致设置面板冗长 | 单行紧凑展示 + 启用态置顶分组;数量过多时(20+)启用搜索框 |
| show-value 类的"复制选中行"交互与现有 Spotlight 模式冲突(原 Enter 为执行 default action) | keyword 模式下,每条 item 的 defaultAction 就是"复制本行值",语义自洽 |
| `;ts` 等动态值的展示在用户停留时秒级过期 | 解析时计算一次,不实时刷新;用户复制时拿到的是当时值;接受秒级误差 |

## 后续可演进(不在本版范围)

- 自定义触发前缀(允许用户从 `;` 改为 `:` / `!` / `\\`)
- Snippet 模板变量替换(`;curl-get {url}` → 渲染后复制)
- 多参数语法(`;k <a> <b>`)
- KeywordCommand 的导入/导出/分享(JSON 格式)
- 内置项允许"行为微调"(如 `;ip` 只显示 IPv4)
- 接入 launcher provider 直接执行外部命令(需要严格安全沙箱)
- KeywordCommand 与 Snippet 双向引用(Snippet 里 `{{keyword:ip}}` 占位)
