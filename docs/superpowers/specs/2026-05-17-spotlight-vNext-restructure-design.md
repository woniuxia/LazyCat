# Spotlight vNext:Provider 描述符化与配置面板

> 上一版:`2026-05-16-spotlight-v0.6-default-actions-design.md`
> 主轴:结构性升级。把硬编码的 provider 注册抽为「描述符 + 配置覆盖」架构,新接入 launcher provider 作为试金石,并把可见配置入口接入 SettingsPanel。

## 概述

v0.6 之前的 Spotlight 把 provider 元信息(scope 前缀、quick command 关键字、启用与否)硬编码在多处:`spotlight-query.ts` 的 `SCOPE_PREFIX_MAP` / `parseQuickCommand`、`SpotlightPanel.vue` 的 `SCOPE_LABEL`、各 provider 文件的 `scopeKeys`。本版把这些元信息收归到 provider 自身的描述符,并引入用户配置层让 provider 启用状态、scope 别名、quick command 开关在不重新发版的前提下可调。同时接入 launcher provider,作为新架构的首个非历史 provider。

## 目标 / 非目标

### 目标

1. 把 `registerProvider(provider)` 升级为 `registerProvider(descriptor)`,descriptor 含元信息 `id / name / desc / defaultAliases / defaultEnabled / quickCommands`。
2. 引入 `SpotlightConfigStore`:从 `user_settings.spotlight_config_v1` 读用户覆盖,与 descriptor 默认值合并,产出运行时 `SpotlightView`(`enabled` / `aliases` / `quickCommandEnabled`)。
3. `SettingsPanel.vue` 新增 `Spotlight` section(沿用现有 `<section class="settings-section">` 堆叠模式,不引入 tab 机制),UI 暴露三类配置:provider 启用、scope 别名、quick command 开关。
4. 新增 `launcherProvider`:prefetch 走 `tool:launcher:list`,默认动作 `tool:launcher:launch`,备选动作含管理员启动 / 打开所在目录 / 跳转 Launcher 工具。
5. 解析层 `parseSpotlightQuery` / `parseQuickCommand` 接收 alias map / enabled set 作为参数,不再写死。
6. 配置变更跨窗口广播(主窗口设置面板 ↔ Spotlight 窗口),变更后立即生效,不需重启。

### 非目标 / YAGNI

- 不做插件化运行时(动态加载第三方 provider)
- 不做权重数值调节,只暴露启用 / 禁用
- 不做输入历史、详情预览面板、结果分组(进 v0.8 候选)
- 不做 alias 导入 / 导出
- 不为 launcher provider 增加新增条目能力,写入仍走 LauncherPanel,Spotlight 只读
- 不改其它 provider 的现有动作语义
- 不引入插件 manifest 导出(C 方案被否)

## 现状回顾

- Spotlight 在独立 webview 窗口,5 个 provider:tool / vault / hosts / todo / pm,以及隐藏的 suggestion
- 作用域前缀 `t / v / h / p` + 空格,硬编码在 `spotlight-query.ts:3` 的 `SCOPE_PREFIX_MAP`
- quick command 共 2 个:`+ ` 速建 Todo、`calc ` 计算器,硬编码在 `parseQuickCommand`
- `SCOPE_LABEL` 在 `SpotlightPanel.vue:116` 重复列举 provider 信息
- launcher 工具完整存在(`src-tauri/src/tools/launcher.rs`),已有 `tool:launcher:list` / `tool:launcher:launch` 等通道,但未接入 Spotlight

## 数据契约

### ProviderDescriptor(注册时静态描述)

```ts
interface ProviderDescriptor {
  id: SpotlightProviderId;
  name: string;
  description: string;
  badgeShort: string;
  badgeTone: StatusTone;
  weight: number;
  defaultAliases: string[];
  defaultEnabled: boolean;
  hiddenInSettings?: boolean;
  quickCommands?: QuickCommandDescriptor[];
  prefetch: () => Promise<SpotlightItem[]>;
  defaultAction: (
    item: SpotlightItem,
    ctx: SpotlightExecuteContext,
  ) => Promise<SpotlightExecuteResult>;
  buildActions?: (item: SpotlightItem) => SpotlightAction[];
  executeAction?: (
    item: SpotlightItem,
    actionId: string,
    ctx: SpotlightExecuteContext,
  ) => Promise<SpotlightExecuteResult>;
}

interface QuickCommandDescriptor {
  id: "todo-create" | "calc";
  name: string;
  trigger: { type: "prefix"; value: "+ " } | { type: "keyword"; value: "calc" };
  description: string;
  defaultEnabled: boolean;
}
```

### SpotlightConfig(用户覆盖)

存储位置 `user_settings.spotlight_config_v1`,JSON 单 key。

```ts
type QuickCommandId = "todo-create" | "calc";

interface SpotlightConfig {
  version: 1;
  providers: Record<
    SpotlightProviderId,
    {
      enabled?: boolean;
      aliases?: string[];
    }
  >;
  quickCommands: Record<QuickCommandId, { enabled?: boolean }>;
}
```

- 缺省字段全部回落 descriptor 默认值
- 写时整对象覆盖,避免增量 merge 的竞争

### SpotlightView(运行时合并产物)

```ts
interface SpotlightView {
  providers: ResolvedProvider[];
  aliasMap: Map<string, SpotlightProviderId>;
  enabledQuickCommands: Set<QuickCommandId>;
}

interface ResolvedProvider extends ProviderDescriptor {
  enabled: boolean;
  aliases: string[];
}
```

## 组件拆分

### 1. `src/spotlight/registry.ts`(重构)

- `registerProvider(descriptor)` 入参由 `SpotlightProvider` 改为 `ProviderDescriptor`
- 新增 `listDescriptors()`,供设置面板枚举(过滤 `hiddenInSettings`)
- `listProviders()` / `searchItems()` 保留外部签名,内部基于 `SpotlightView` 过滤 `enabled=false` 的 provider
- 新增 `getCurrentView(): SpotlightView`,Spotlight 面板与解析层从这里读

### 2. `src/spotlight/config-store.ts`(新增)

- `ensureLoaded(forceReload = false)` / `getView()` / `saveConfig(next: SpotlightConfig)` / `subscribe(cb)`
- 内部维护 `currentView`,合并 descriptor 默认值 + 用户覆盖
- 暴露 `validateAliases(input: string[], exceptId: SpotlightProviderId)`:返回冲突详情
- 模块单例,与 `useClipboardSuggestion` 同模式

### 3. `src/utils/spotlight-query.ts`(改造)

- `parseSpotlightQuery(raw, aliasMap)`:第二参数替换原硬编码
- `parseQuickCommand(raw, enabledIds)`:加 enabledIds 参数;未启用的 quick command 返回 null
- 旧硬编码 `SCOPE_PREFIX_MAP` / calc 关键字 走 descriptor 的 `defaultAliases` / `quickCommands` 数据,store 启动时灌进 alias map / enabled set
- 单测同步:测试中显式构造 alias map / enabled set

### 4. `src/spotlight/providers/launcher.ts`(新增)

- prefetch:`invokeToolByChannel("tool:launcher:list", {})`,取 `items[]`,每条映射为 `SpotlightItem`
  - title:`name`
  - subtitle:`group_name || (isDir ? "文件夹" : "应用")`
  - badge:`{ short: "启", tone: "primary" }`
  - searchFields:`[name(1.2), 拼音首字母, exe_path 文件名 stem(0.6)]`
  - weight:`1 + min(launch_count, 50) * 0.01`
  - payload:`{ exePath, arguments, isDir, name }`
- defaultAction:`invokeToolByChannel("tool:launcher:launch", { exe_path, arguments, admin: false })`,成功 closeSpotlight,toast `已启动 <name>`
- buildActions:`launch`(默认 Enter) / `launch_admin` / `open_folder` / `open_launcher`
  - `launch_admin`:`tool:launcher:launch` 传 `admin: true`(launcher.rs:349/373 已原生支持)
  - `open_folder`:`tool:launcher:open-folder` 传 `exe_path`(launcher.rs:415 已存在,前端通道名连字符与 `bridge/tauri.ts:202` 对齐)
  - `open_launcher`:`invoke("spotlight_pick", { target: "launcher" })`(sidebar id 已核对)
- 不提供导入 / 扫描入口

### 5. `src/components/SpotlightPanel.vue`(改造)

- 启动时 `await configStore.ensureLoaded()`,从 view 取 alias map / enabled set
- `parsed` / `quickCommand` computed 接受 view 数据
- 监听 `configStore.subscribe`:配置变化时重跑 `prefetchAll()`(provider 启用集变更)
- `placeholder` 文案根据当前启用 provider 集动态生成
- 删除硬编码 `SCOPE_LABEL`,改读 descriptor

### 6. `src/components/settings/SpotlightSettings.vue`(新增)

- 三段式:Provider 列表、scope 别名编辑、quick command 开关
- Provider 行:开关 + 名称 + 描述 + 默认别名 chip + 自定义别名输入框(逗号分隔,失焦校验)
- 别名校验:重复、与系统保留前缀(`+ ` / `calc `)冲突时红框 + 行内提示
- 「恢复默认」按钮:整个 Spotlight 配置重置为 descriptor 默认
- 通过 `emit('change')` 触发 store 持久化

### 7. `src/components/SettingsPanel.vue`(改造)

- 新增 `Spotlight` section,渲染 `SpotlightSettings.vue`
- 沿用现有 `<section class="settings-section">` 堆叠模式;当前 SettingsPanel 无 tab 机制,本版不引入

### 8. `src/spotlight/types.ts`(扩展)

- `SpotlightProviderId` 增加 `"launcher"`
- 导出新接口 `ProviderDescriptor` / `QuickCommandDescriptor` / `SpotlightConfig` / `SpotlightView` / `ResolvedProvider`

### 9. 各 provider 文件(适配)

- tool / vault / hosts / todo / pm / suggestion 改为导出 `ProviderDescriptor`
- 把元信息(`scopeKeys`、当前在 SpotlightPanel 中的 `SCOPE_LABEL`、quick command 关键字)抽到 descriptor

## 数据流

### 启动序列(Spotlight 窗口打开)

1. `SpotlightPanel.onMounted` → `configStore.ensureLoaded()`
   - 读 `user_settings.spotlight_config_v1`
   - 与 6 个内置 descriptor 合并,生成 `SpotlightView`,缓存到模块内
2. 并行 `prefetchAll()`:对 `view.providers` 中 `enabled=true` 的 provider 调 `prefetch()`
3. `refreshClipboardSuggestion()`:仍受 suggestion provider 是否启用约束;disabled 时整体跳过

### 查询解析序列(每次 query 变化)

1. `parseQuickCommand(raw, view.enabledQuickCommands)`
   - 命中 → 直接产出虚拟结果项,跳过后续
2. `parseSpotlightQuery(raw, view.aliasMap)`
   - 命中 scope → 走 `searchItems(query, scope)`
   - 未命中 → 全局搜索(过滤掉 `enabled=false`)

### 配置变更序列(设置面板内编辑)

1. 用户改开关 / 编辑别名 / 提交
2. `SpotlightSettings.vue` → `configStore.saveConfig(next)`
3. store 内部:
   - 校验 alias 不冲突、不与保留前缀重叠
   - 写 `user_settings.spotlight_config_v1`(失败回滚内存状态)
   - 计算 `nextView`,对比 `currentView` 判定刷新策略:
     - enabled 集变化 → 触发 `prefetchAll()`,新启用的 provider 需要拉数据
     - 仅 alias / quick command 开关变化 → 只替换 `currentView`,不重跑 prefetch(数据不变,只影响解析)
   - 通过 Tauri `emit("spotlight-config-changed")` 广播(主窗口 + Spotlight 窗口都监听)
4. Spotlight 窗口收事件 → `ensureLoaded(forceReload=true)` → 按上述策略刷新
5. 主窗口设置面板同步显示态(避免双窗口编辑漂移)

### Launcher 接入数据流

1. prefetch:`tool:launcher:list` → 已含 `launch_count`,用于权重
2. 默认动作:`tool:launcher:launch` → 后端原子地 `launch_count += 1`,Spotlight 不需再次查询
3. 备选「跳转到 Launcher 工具」沿用现有 `spotlight_pick` 通道,target 为 `launcher`(实施前确认侧边栏 id)

### 降级路径

- 配置读取失败 → 用纯 descriptor 默认值,设置面板顶部红条提示「配置加载失败,已使用默认值」
- 别名 map 为空 → 解析层返回 `scope: null`,所有查询走全局
- `enabled` 集为空(用户全关)→ Spotlight 显示空态「所有数据源已禁用,前往设置启用」+ 跳设置按钮(保存时不强制至少启用一个,把选择权留给用户)
- 跨窗口广播兜底 → Spotlight 窗口除监听 `spotlight-config-changed`,同时监听 `tauri://window-shown`(或现有 `spotlight-reset` 通道),窗口每次显示时调一次 `ensureLoaded(forceReload=true)` 兜底

## 错误与冲突处理

### 别名校验规则(写入前)

- alias 一律存为小写,匹配前 `toLowerCase` 归一(与 `parseSpotlightQuery` 当前 head 处理一致)
- 单 provider 内别名去重(自身重复直接合并)
- 跨 provider 别名互斥,冲突项明确提示:`别名 "t" 已被「任务」占用`
- 保留 token(不可作为 alias):`+`、`calc`、空串、含空格的字符串
- 别名最大长度 16,允许字符 `[a-zA-Z0-9_-]`,中文不允许(防止与查询文字混淆)
- tool provider 同其它 provider 一样允许用户编辑 alias(descriptor 默认 `defaultAliases: []`,用户可加自定义别名,与其它 provider 行为一致)
- 不强制至少启用一个 provider:全关时由运行时显示空态 + 引导按钮处理,把选择权留给用户

### 配置版本不兼容

- `version` 字段不匹配:整体丢弃,回落默认值,写入备份 key `spotlight_config_v1.backup`(单 key 覆盖,避免多次损坏积累备份)
- 字段缺失:每字段独立回落 descriptor 默认,不整体放弃
- JSON 解析失败:同上整体回落 + 单 key 备份

### 运行时降级

- `tool:launcher:list` 失败:launcher provider 返回空列表,SpotlightPanel 不报错;prefetch 失败一向静默,与现有 todo / pm 行为一致
- 配置广播事件丢失(理论不会):双窗口可能短暂不一致,下次呼出 Spotlight 时 `ensureLoaded` 重新拉取兜底
- 用户用 alias 触发已禁用 provider 的 scope:解析时如果 `enabledIds` 不含该 provider,视为非作用域查询(原文进 query)

### 并发写

- store 内 `saveConfig` 加 `inFlight` 标记,期间二次调用排队
- 失败回滚内存态,不调用 emit;UI 通过 await 的拒绝路径感知失败,行内提示

### 回归保护

- 现有所有 provider 默认动作不变,descriptor 默认 enabled=true、defaultAliases 与当前硬编码一致 → 用户不动配置,行为与 v0.6 完全一致
- suggestion provider `hiddenInSettings=true`,UI 不展示,无法被用户误关

## 改动文件清单

| 文件                                                         | 改动类型 | 说明                                                                                                                                                     |
| ------------------------------------------------------------ | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/desktop/src/spotlight/types.ts`                        | 修改     | 新增 `launcher` 到 `SpotlightProviderId`;导出 `ProviderDescriptor` / `QuickCommandDescriptor` / `SpotlightConfig` / `SpotlightView` / `ResolvedProvider` |
| `apps/desktop/src/spotlight/registry.ts`                     | 修改     | `registerProvider` 接受 descriptor;新增 `listDescriptors` / `getCurrentView`;`searchItems` 过滤 disabled                                                 |
| `apps/desktop/src/spotlight/config-store.ts`                 | 新增     | 配置持久化 + 合并 + 广播 + 校验                                                                                                                          |
| `apps/desktop/src/spotlight/providers/tool.ts`               | 修改     | 改为导出 descriptor,内置元信息                                                                                                                           |
| `apps/desktop/src/spotlight/providers/vault.ts`              | 修改     | 同上                                                                                                                                                     |
| `apps/desktop/src/spotlight/providers/hosts.ts`              | 修改     | 同上                                                                                                                                                     |
| `apps/desktop/src/spotlight/providers/todo.ts`               | 修改     | 同上;`calc` 不属于 todo,quick command 拆到独立 descriptor 集                                                                                             |
| `apps/desktop/src/spotlight/providers/pm.ts`                 | 修改     | 同上                                                                                                                                                     |
| `apps/desktop/src/spotlight/providers/suggestion.ts`         | 修改     | descriptor 标记 `hiddenInSettings: true`                                                                                                                 |
| `apps/desktop/src/spotlight/providers/launcher.ts`           | 新增     | launcher provider 全套实现                                                                                                                               |
| `apps/desktop/src/spotlight/quick-commands.ts`               | 新增     | quick command descriptor 集中注册(`+ ` / `calc`)                                                                                                         |
| `apps/desktop/src/utils/spotlight-query.ts`                  | 修改     | 解析函数接收 alias map / enabled set 参数                                                                                                                |
| `apps/desktop/src/utils/spotlight-query.test.ts`             | 修改     | 单测对齐新签名 + 新增 alias / 禁用 quick command 用例                                                                                                    |
| `apps/desktop/src/components/SpotlightPanel.vue`             | 修改     | 启动 ensureLoaded;`SCOPE_LABEL` 改读 descriptor;监听配置变更                                                                                             |
| `apps/desktop/src/components/settings/SpotlightSettings.vue` | 新增     | Spotlight 设置子页                                                                                                                                       |
| `apps/desktop/src/components/SettingsPanel.vue`              | 修改     | 新增 Spotlight 子页签                                                                                                                                    |
| `apps/desktop/src/spotlight/config-store.test.ts`            | 新增     | 配置 store 单测                                                                                                                                          |

## 验证

### 自动化

- `pnpm typecheck`
- `pnpm test`(覆盖 `spotlight-query.test.ts` + 新增 `config-store.test.ts`)
- `pnpm --filter @lazycat/desktop build:web`

### 手测清单

1. 默认行为不变回归:全新数据库 / 老数据库,不动设置 → 全部默认动作、scope 前缀、quick command 与 v0.6 一致
2. Provider 禁用:关掉 hosts → 全局查询不再出现 hosts、`h xxx` 视为普通查询
3. 别名自定义:把 todo 别名改为 `q`,`q 周报` 正确命中;`t xxx` 不再触发 scope
4. 别名冲突:尝试给 vault 加 `t` → UI 拒绝并提示「已被任务占用」
5. 保留 token:尝试加 `+` 或 `calc` → 拒绝
6. 全部禁用:关闭所有 provider → 空态正确,跳设置按钮可用
7. 快速命令开关:关 calc → `calc 1+2` 走普通查询;关 `+ ` → `+ 写周报` 走普通查询
8. Launcher 接入:已有 launcher 条目 → Spotlight 全局可搜;Enter 启动;Tab 备选含管理员/打开目录/跳转 Launcher
9. 配置广播:主窗口设置面板改完,Spotlight 已开着 → 下次输入立即生效(不需重启)
10. 多窗口一致性:主窗口设置面板和 Spotlight 窗口同时打开,在主窗口改配置 → Spotlight 监听到广播自动刷新;手动 hide 再 show Spotlight 也能拿到最新配置(window-shown 兜底路径)
11. 配置损坏恢复:手动改 `spotlight_config_v1` 为非法 JSON → 重启 Spotlight 不崩溃、回落默认、生成单 key 备份(连续两次损坏只留最近一次)

## 关键风险与对策

| 风险                                        | 对策                                                                                      |
| ------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Provider 接口变更影响范围大,易破坏现有动作  | descriptor 兼容当前 `SpotlightProvider` 全字段,默认值与现状等价;增加回归手测项            |
| 跨窗口 emit 不可达(主窗口 ↔ Spotlight 窗口) | 失败时下次呼出 Spotlight `ensureLoaded` 兜底;若仍不可达,在 Rust 端加桥接命令              |
| 用户改 alias 后忘了原默认前缀,行为困惑      | UI 始终展示「默认 / 自定义」两行,提供恢复默认按钮                                         |
| 中文 alias 与查询文字混淆                   | 校验拒绝中文;只允许 `[a-zA-Z0-9_-]`                                                       |
| Launcher 数据量大(数百条)拖慢 prefetch      | launcher 已在后端按 `launch_count DESC` 排序,前端不做额外处理;若实测瓶颈,后续追加上限参数 |

## 后续可演进(不在本版范围)

- 插件化运行时:descriptor 已具备,可在后续版本支持外部加载
- 权重数值调节:descriptor 已暴露 weight,可在设置面板补 UI
- 输入历史 / 详情预览面板 / 结果分组
- alias 导入 / 导出 / 团队分享
- launcher 在 Spotlight 内新增条目
