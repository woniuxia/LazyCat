# Spotlight vNext 实现计划

> 配套设计文档:`2026-05-17-spotlight-vNext-restructure-design.md`
> 目标:把 6 个 milestone 落到可独立提交、可独立验证的步骤上,每步结束时仓库都处于可编译可运行状态。

## 0. 实施总览

### 0.1 提交粒度

每个 milestone 一个 commit,按顺序合入。任一步骤失败可独立回退,不影响已合入的前序步骤。

| Milestone | 主题 | 改动文件数 | 风险等级 |
|-----------|------|:----------:|:--------:|
| M1 | types.ts + registry.ts 向后兼容改造 | 2 | 低 |
| M2 | 6 个现有 provider 改为导出 descriptor | 7 | 中 |
| M3 | config-store.ts 新增 + spotlight-query.ts 参数化 | 4 | 中 |
| M4 | launcher provider 新增(新架构试金石) | 2 | 低 |
| M5 | SpotlightSettings.vue + SettingsPanel 接入 | 2 | 中 |
| M6 | 跨窗口广播 + 配置变更联动 | 3 | 中 |

### 0.2 每步验证基线

- `pnpm typecheck`
- `pnpm test`(覆盖 spotlight-query.test.ts,M3 后含 config-store.test.ts)
- `pnpm --filter @lazycat/desktop build:web`(只在 M2、M5、M6 后必须跑;其它步骤如果不动 Vue 文件可跳)

### 0.3 前置条件确认

- ✅ launcher.rs 已支持 `admin: bool`(launcher.rs:349-373)
- ✅ launcher.rs 已支持 `open_folder` action(launcher.rs:415-425)
- ✅ 前端通道 `tool:launcher:list` / `launch` / `open-folder` 已在 `bridge/tauri.ts:195-202` 映射
- ✅ `spotlight_pick(target: "launcher")` 可跳侧边栏(sidebar id 已核对)
- ✅ `spotlight-reset` 事件 SpotlightPanel.vue:542 已监听,可复用作 forceReload 触发器之一

---

## M1. types.ts + registry.ts 向后兼容改造

### 目标

引入 `ProviderDescriptor` / `QuickCommandDescriptor` / `SpotlightConfig` / `SpotlightView` / `ResolvedProvider` 类型,但保持现有 `SpotlightProvider` 接口不删除。registry 同时接受新旧两种注册形态,使 M2 可以逐 provider 迁移而不破坏构建。

### 改动文件

#### `apps/desktop/src/spotlight/types.ts`

- `SpotlightProviderId` 增加 `"launcher"`
- 新增 `QuickCommandId = "todo-create" | "calc"`
- 新增 `QuickCommandDescriptor`:`{ id, name, trigger, description, defaultEnabled }`
- 新增 `ProviderDescriptor`:在 `SpotlightProvider` 全字段基础上增加 `name / description / defaultAliases / defaultEnabled / hiddenInSettings? / quickCommands?`
- 新增 `SpotlightConfig`:`{ version: 1, providers, quickCommands }`
- 新增 `ResolvedProvider` = `ProviderDescriptor & { enabled, aliases }`
- 新增 `SpotlightView`:`{ providers, aliasMap, enabledQuickCommands }`
- 保留 `SpotlightProvider` 接口(M2 各 provider 迁移完成后再标 `@deprecated`,M2 结束时删除)

#### `apps/desktop/src/spotlight/registry.ts`

- `registerProvider` 重载签名:同时接受 `SpotlightProvider` 或 `ProviderDescriptor`
- 内部维护两个 Map:`DESCRIPTORS: Map<id, ProviderDescriptor>` + 保留 `PROVIDERS: Map<id, SpotlightProvider>`(后者作为旧 provider 的兼容栈,M2 后删除)
- 新增 `listDescriptors(): ProviderDescriptor[]`(过滤 `hiddenInSettings`)
- `listProviders()` 暂时合并两个 Map 的内容,使 SpotlightPanel.vue 既能拿到旧 provider 也能拿到新 descriptor
- `searchItems()` 暂不引入 enabled 过滤(等 M3 拿到 SpotlightView 后再加)

### 关键代码片段

```typescript
// registry.ts
function isDescriptor(p: SpotlightProvider | ProviderDescriptor): p is ProviderDescriptor {
  return "defaultAliases" in p && "defaultEnabled" in p;
}

export function registerProvider(p: SpotlightProvider | ProviderDescriptor): void {
  if (isDescriptor(p)) {
    DESCRIPTORS.set(p.id, p);
  } else {
    PROVIDERS.set(p.id, p);
  }
}
```

### 验证

- `pnpm typecheck` 通过
- `pnpm test` 通过(现有 spotlight-query.test.ts 不动)
- `pnpm dev` 启动后 Spotlight 行为与 main 完全一致

### 可独立提交

✅ 是。新增类型不影响现有 provider;registry 双 Map 兼容。

---

## M2. 6 个现有 provider 改为导出 descriptor

### 目标

把 `tool / vault / hosts / todo / pm / suggestion` 从导出 `SpotlightProvider` 改为导出 `ProviderDescriptor`。`SpotlightPanel.vue` 中硬编码的 `SCOPE_LABEL` 改读 descriptor。M2 结束时,`SpotlightProvider` 接口可以彻底删除。

### 改动文件

#### 6 个 provider 文件

每个 provider 加入新字段,值与现状等价:

| Provider | name | description | defaultAliases | defaultEnabled | hiddenInSettings |
|----------|------|-------------|----------------|----------------|------------------|
| tool | 工具 | 在所有内置工具中检索 | `[]` | true | false |
| vault | 凭据 | 密码库快速复制 | `["v","vault"]` | true | false |
| hosts | Hosts | 切换 hosts profile | `["h","hosts"]` | true | false |
| todo | 任务 | 任务清单与速建 | `["t","todo"]` | true | false |
| pm | 项目 | 项目工作项检索 | `["p","pm"]` | true | false |
| suggestion | 剪贴板建议 | 剪贴板内容智能匹配 | `[]` | true | **true** |

每个文件保留所有现有函数(prefetch / defaultAction / buildActions / executeAction),只换最末导出对象的类型与字段。

#### `apps/desktop/src/components/SpotlightPanel.vue`

- 删除 `SCOPE_LABEL` 常量(行 116-122)
- `scopeLabel` 改为从 `registry.getProvider(scope)` 读 `descriptor.name`
- `placeholder` 改为根据所有启用 provider 的 name 动态生成(本步骤先用全部启用,M3 接入 SpotlightView 后改为按 view.providers)

#### `apps/desktop/src/spotlight/registry.ts`

- 删除 `PROVIDERS` 兼容 Map,只保留 `DESCRIPTORS`
- 删除 `registerProvider` 的 union 入参,只接受 `ProviderDescriptor`
- 删除 `isDescriptor` 类型守卫

#### `apps/desktop/src/spotlight/types.ts`

- 删除 `SpotlightProvider` 接口

### 验证

- `pnpm typecheck` 通过(关键检查点:6 个 provider 文件不再 import `SpotlightProvider`)
- `pnpm test` 通过
- `pnpm --filter @lazycat/desktop build:web` 通过
- `pnpm dev` 手测:
  - 所有 5 个 scope 前缀(t/v/h/p)正常工作
  - quick command (`+ ` / `calc `) 正常工作
  - 5 个 provider 的默认动作正常

### 可独立提交

✅ 是。M2 结束时 Spotlight 行为与 v0.6 完全一致,只是元信息存储位置变了。

---

## M3. config-store.ts 新增 + spotlight-query.ts 参数化

### 目标

引入用户配置层和运行时 SpotlightView。`parseSpotlightQuery` / `parseQuickCommand` 接收 alias map / enabled set 参数,不再写死。

### 改动文件

#### `apps/desktop/src/spotlight/config-store.ts`(新增)

API 设计:

```typescript
let currentView: SpotlightView | null = null;
let cachedConfig: SpotlightConfig | null = null;
const subscribers = new Set<(view: SpotlightView) => void>();
let inFlightSave: Promise<void> | null = null;

export async function ensureLoaded(forceReload = false): Promise<SpotlightView>;
export function getView(): SpotlightView;  // 必须先 ensureLoaded
export async function saveConfig(next: SpotlightConfig): Promise<void>;
export function subscribe(cb: (view: SpotlightView) => void): () => void;
export function validateAliases(
  input: string[],
  exceptId: SpotlightProviderId,
): { ok: true } | { ok: false; conflicts: { alias: string; reason: string }[] };

// 内部
function mergeView(descriptors: ProviderDescriptor[], config: SpotlightConfig): SpotlightView;
function readFromUserSettings(): Promise<SpotlightConfig | null>;
function writeToUserSettings(config: SpotlightConfig): Promise<void>;
function buildDefaultConfig(descriptors: ProviderDescriptor[]): SpotlightConfig;
```

合并规则:
- alias:`provider.aliases ?? descriptor.defaultAliases`,统一 `toLowerCase().trim()`,去除空串
- enabled:`provider.enabled ?? descriptor.defaultEnabled`
- quickCommand:同上
- 计算 `aliasMap`:遍历所有启用 provider 的 aliases,后注册的覆盖先注册的(理论上 saveConfig 已校验冲突)

保留 token 校验:
```typescript
const RESERVED_TOKENS = new Set(["+", "calc"]);
const ALIAS_PATTERN = /^[a-zA-Z0-9_-]{1,16}$/;
```

备份策略:JSON 解析失败时,先把原始字符串写到 `spotlight_config_v1.backup`(单 key 覆盖),再回落默认值。

#### `apps/desktop/src/utils/spotlight-query.ts`(改造)

```typescript
export function parseSpotlightQuery(
  raw: string,
  aliasMap: Map<string, SpotlightProviderId>,
): ScopeParseResult;

export function parseQuickCommand(
  raw: string,
  enabledIds: Set<QuickCommandId>,
): QuickCommand | null;

export function dropScopePrefix(
  raw: string,
  aliasMap: Map<string, SpotlightProviderId>,
): string;
```

删除文件内 `SCOPE_PREFIX_MAP` 常量。calc / `+ ` 触发逻辑保持,但加 `enabledIds.has(...)` 守卫。

#### `apps/desktop/src/utils/spotlight-query.test.ts`(改造)

- 所有调用点显式构造 `aliasMap`(沿用现有 `t/todo/v/vault/h/hosts/p/pm` 默认映射)
- 所有 quick command 用例显式传 `enabledIds = new Set(["todo-create", "calc"])`
- 新增用例:
  - alias 大小写:`parseSpotlightQuery("T 客户", aliasMap)` 命中(map 内 alias 已小写,函数内 head 转小写)
  - 自定义 alias:`new Map([["q", "todo"]])` 可命中 `q 周报`
  - 禁用 quick command:`parseQuickCommand("calc 1+2", new Set(["todo-create"]))` 返回 null

#### `apps/desktop/src/spotlight/config-store.test.ts`(新增)

覆盖用例:
- 空配置回落默认(所有 enabled=true,alias 与现状一致)
- 用户禁用 hosts 后 view.providers 的 hosts.enabled=false
- 用户改 todo alias 为 `["q"]`,aliasMap 含 q→todo,不含 t→todo
- alias 冲突校验:跨 provider 重复返回 conflicts
- 保留 token 冲突:alias 为 `"+"` 或 `"calc"` 返回 conflicts
- alias 大小写归一:输入 `["T","Todo"]` 经校验后存为 `["t","todo"]`
- JSON 损坏:`readFromUserSettings` 抛错时回落默认 + 写入 backup key
- saveConfig 并发:同时调两次,第二次排队,内存态在第二次完成后才更新

#### `apps/desktop/src/components/SpotlightPanel.vue`

- `onMounted` 增加 `await ensureLoaded()`
- `parsed`、`quickCommand` 接受 view 参数:
  ```typescript
  const view = ref<SpotlightView | null>(null);
  const parsed = computed(() =>
    view.value ? parseSpotlightQuery(query.value, view.value.aliasMap) : { scope: null, query: query.value },
  );
  const quickCommand = computed(() =>
    view.value ? parseQuickCommand(query.value, view.value.enabledQuickCommands) : null,
  );
  ```
- `prefetchAll` 改为只对 `view.providers.filter(p => p.enabled)` 拉数据
- `searchItems` 调用点传入 enabled 过滤(在 registry.searchItems 内部按 view.providers 过滤)
- `scopeLabel` 改为从 view.providers 查 name
- `placeholder` 用 view.providers 中 enabled 的 name 拼接

### 验证

- `pnpm typecheck` 通过
- `pnpm test` 通过(spotlight-query.test.ts + config-store.test.ts)
- `pnpm dev` 手测:
  - 默认行为与 v0.6 完全一致(用户未动配置)
  - 在 devtools 内手动 `localStorage` 没用,要直接改 user_settings 验证用户覆盖生效
  - 第一次启动 Spotlight,backup key 不存在

### 可独立提交

✅ 是。此时 UI 还没有设置面板,但运行时已支持配置覆盖。可以通过手改数据库验证。

---

## M4. launcher provider 新增

### 目标

引入第一个非历史 provider,验证新架构可扩展性。

### 改动文件

#### `apps/desktop/src/spotlight/providers/launcher.ts`(新增)

```typescript
import { invoke } from "@tauri-apps/api/core";
import { invokeToolByChannel } from "../../bridge/tauri";
import { toPinyinInitials } from "../../utils/fuzzy-match";
import { registerProvider } from "../registry";
import type {
  ProviderDescriptor,
  SpotlightExecuteContext,
  SpotlightExecuteResult,
  SpotlightItem,
} from "../types";

interface LauncherEntry {
  id: number;
  name: string;
  exe_path: string;
  arguments?: string;
  group_name?: string;
  launch_count?: number;
}

function makeField(text: string, weight: number) {
  const cleaned = text.trim();
  return { text: cleaned, initials: toPinyinInitials(cleaned), weight };
}

function isDirPath(p: string): boolean {
  // 后端 launch_app 用 Path::is_dir,前端做粗略推断:无扩展名且不以 .exe / .bat 结尾
  // 实际由后端最终判断,前端只用于 subtitle 文案
  return !/\.[a-zA-Z0-9]{1,5}$/.test(p);
}

async function prefetchLauncher(): Promise<SpotlightItem[]> {
  let list: LauncherEntry[] = [];
  try {
    const raw = (await invokeToolByChannel("tool:launcher:list", {})) as
      | { items?: LauncherEntry[] }
      | LauncherEntry[]
      | null;
    if (Array.isArray(raw)) list = raw;
    else if (raw && Array.isArray(raw.items)) list = raw.items;
  } catch {
    return [];
  }

  return list.map<SpotlightItem>((e) => {
    const isDir = isDirPath(e.exe_path);
    const count = e.launch_count ?? 0;
    const stem = e.exe_path.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? "";
    return {
      providerId: "launcher",
      itemId: String(e.id),
      title: e.name,
      subtitle: e.group_name || (isDir ? "文件夹" : "应用"),
      badge: { short: "启", tone: "primary" },
      searchFields: [
        makeField(e.name, 1.2),
        makeField(stem, 0.6),
      ],
      weight: 1 + Math.min(count, 50) * 0.01,
      payload: {
        exePath: e.exe_path,
        arguments: e.arguments ?? "",
        isDir,
        name: e.name,
      },
    };
  });
}

async function launchEntry(item: SpotlightItem, admin: boolean): Promise<SpotlightExecuteResult> {
  try {
    await invokeToolByChannel("tool:launcher:launch", {
      exe_path: item.payload?.exePath,
      arguments: item.payload?.arguments ?? "",
      admin,
    });
    return {
      closeSpotlight: true,
      toast: { message: `已启动 ${item.payload?.name}`, type: "success" },
    };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { errorMessage: msg };
  }
}

async function openFolder(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  try {
    await invokeToolByChannel("tool:launcher:open-folder", {
      exe_path: item.payload?.exePath,
    });
    return { closeSpotlight: true };
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err);
    return { errorMessage: msg };
  }
}

async function openLauncher(): Promise<SpotlightExecuteResult> {
  await invoke("spotlight_pick", { target: "launcher" });
  return { closeSpotlight: true };
}

async function defaultAction(item: SpotlightItem): Promise<SpotlightExecuteResult> {
  return launchEntry(item, false);
}

function buildActions() {
  return [
    { id: "launch", label: "启动", icon: "play", shortcut: "Enter" },
    { id: "launch_admin", label: "以管理员身份启动", icon: "shield" },
    { id: "open_folder", label: "打开所在目录", icon: "folder" },
    { id: "open_launcher", label: "跳转到快捷启动", icon: "external" },
  ];
}

async function executeAction(
  item: SpotlightItem,
  actionId: string,
): Promise<SpotlightExecuteResult> {
  if (actionId === "launch") return launchEntry(item, false);
  if (actionId === "launch_admin") return launchEntry(item, true);
  if (actionId === "open_folder") return openFolder(item);
  if (actionId === "open_launcher") return openLauncher();
  return { errorMessage: `未知动作 ${actionId}` };
}

export const launcherDescriptor: ProviderDescriptor = {
  id: "launcher",
  name: "快捷启动",
  description: "通过 Spotlight 启动已注册的应用与文件夹",
  badgeShort: "启",
  badgeTone: "primary",
  weight: 0.95,
  defaultAliases: [],
  defaultEnabled: true,
  prefetch: prefetchLauncher,
  defaultAction,
  buildActions,
  executeAction,
};

registerProvider(launcherDescriptor);
```

#### `apps/desktop/src/components/SpotlightPanel.vue`

- 顶部 import 列表新增 `import "../spotlight/providers/launcher";`

### 验证

- `pnpm typecheck` 通过
- `pnpm --filter @lazycat/desktop build:web` 通过
- `pnpm dev` 手测:
  - 先在「快捷启动」工具里添加 1-2 个 exe / 文件夹
  - Spotlight 输入应用名或拼音首字母可命中
  - Enter 启动应用
  - Tab 备选含「以管理员身份启动」「打开所在目录」「跳转到快捷启动」三项
  - 启动后 launch_count 自增(下次 prefetch 权重提高)
  - launcher 数据库为空时 Spotlight 不报错

### 可独立提交

✅ 是。launcher provider 是纯新增,不影响其它 provider。

---

## M5. SpotlightSettings.vue + SettingsPanel 接入

### 目标

把配置入口暴露给用户。在 SettingsPanel 新增 Spotlight section。

### 改动文件

#### `apps/desktop/src/components/settings/SpotlightSettings.vue`(新增)

UI 结构:

```vue
<template>
  <div class="spotlight-settings">
    <div v-if="loadError" class="spotlight-settings-error">
      配置加载失败,已使用默认值。详情:{{ loadError }}
    </div>

    <!-- 数据源 -->
    <div class="spotlight-settings-group">
      <div class="group-title">数据源</div>
      <div
        v-for="p in editableProviders"
        :key="p.id"
        class="provider-row"
      >
        <el-switch
          :model-value="config.providers[p.id]?.enabled ?? p.defaultEnabled"
          @update:model-value="(v) => onToggle(p.id, v)"
        />
        <div class="provider-meta">
          <div class="provider-name">{{ p.name }}</div>
          <div class="provider-desc">{{ p.description }}</div>
        </div>
        <div class="provider-aliases">
          <div class="alias-label">scope 别名(逗号分隔)</div>
          <el-input
            :model-value="aliasInputs[p.id]"
            placeholder="例如:t, todo"
            @update:model-value="(v) => aliasInputs[p.id] = v"
            @blur="commitAliases(p.id)"
          />
          <div v-if="aliasErrors[p.id]" class="alias-error">
            {{ aliasErrors[p.id] }}
          </div>
          <div class="alias-default-hint">
            默认:{{ p.defaultAliases.join(", ") || "(无)" }}
          </div>
        </div>
      </div>
    </div>

    <!-- 快速命令 -->
    <div class="spotlight-settings-group">
      <div class="group-title">快速命令</div>
      <div
        v-for="qc in allQuickCommands"
        :key="qc.id"
        class="quick-command-row"
      >
        <el-switch
          :model-value="config.quickCommands[qc.id]?.enabled ?? qc.defaultEnabled"
          @update:model-value="(v) => onToggleQuickCommand(qc.id, v)"
        />
        <div class="quick-command-meta">
          <div class="quick-command-name">{{ qc.name }}</div>
          <div class="quick-command-desc">{{ qc.description }}</div>
        </div>
      </div>
    </div>

    <!-- 恢复默认 -->
    <div class="spotlight-settings-actions">
      <el-button @click="resetToDefault">恢复默认</el-button>
    </div>
  </div>
</template>
```

逻辑:
- `onMounted` 调 `configStore.ensureLoaded()`,把 `getView()` 拍成本地 `config` ref
- `aliasInputs` 独立维护,用户输入时不立即写 store,blur 时校验并提交
- `commitAliases(id)`:解析逗号、去空、`toLowerCase`,调 `configStore.validateAliases`,有冲突显示行内错误,无冲突调 `saveConfig`
- `onToggle` / `onToggleQuickCommand`:立即 `saveConfig`
- `resetToDefault`:把当前 config 重置为所有字段都不覆盖的版本(空 providers / 空 quickCommands),`saveConfig`
- 不强制至少启用一个 provider(运行时空态由 SpotlightPanel 处理)

#### `apps/desktop/src/components/SettingsPanel.vue`

在合适位置(建议在「系统集成」前)新增一个 `<section class="settings-section">`:

```vue
<section class="settings-section">
  <div class="section-header">
    <div class="section-icon">🔍</div>
    <div class="section-title">
      <h3>Spotlight</h3>
      <p>配置全局搜索面板的数据源与快速命令</p>
    </div>
  </div>
  <div class="section-content">
    <SpotlightSettings />
  </div>
</section>
```

新增 import `SpotlightSettings`。

### 验证

- `pnpm typecheck` 通过
- `pnpm --filter @lazycat/desktop build:web` 通过
- `pnpm dev` 手测(对照设计 §07 手测清单的 1-7 项):
  - 默认行为不变回归
  - 关 hosts → Spotlight 全局查询不再出现 hosts
  - todo alias 改为 `q` → `q 周报` 命中,`t xxx` 不再触发 scope
  - 试加 `t` 给 vault → UI 拒绝
  - 试加 `+` / `calc` → UI 拒绝
  - 关所有 provider → Spotlight 显示空态 + 跳设置按钮
  - 关 calc → `calc 1+2` 走普通查询

### 可独立提交

✅ 是。M5 结束时所有 UI 入口就位,但跨窗口广播还没接(主窗口改完 Spotlight 不会立即生效,需要重新呼出)。

---

## M6. 跨窗口广播 + 配置变更联动

### 目标

主窗口设置面板改完,Spotlight 已开着时立即生效,不需重新呼出。

### 改动文件

#### `apps/desktop/src/spotlight/config-store.ts`(增量)

`saveConfig` 末尾:
```typescript
import { emit } from "@tauri-apps/api/event";

// 在写完 user_settings 并更新 currentView 后:
try {
  await emit("spotlight-config-changed", { version: 1 });
} catch {
  // 跨窗口广播失败不阻塞主流程
}
```

新增:监听同事件,触发自身 reload:
```typescript
import { listen } from "@tauri-apps/api/event";

let unlistenConfigChanged: (() => void) | null = null;

async function startListening() {
  if (unlistenConfigChanged) return;
  unlistenConfigChanged = await listen("spotlight-config-changed", async () => {
    const prevEnabled = new Set(
      currentView?.providers.filter((p) => p.enabled).map((p) => p.id) ?? [],
    );
    await ensureLoaded(true);
    const nextEnabled = new Set(
      currentView?.providers.filter((p) => p.enabled).map((p) => p.id) ?? [],
    );
    const enabledChanged =
      prevEnabled.size !== nextEnabled.size ||
      [...prevEnabled].some((id) => !nextEnabled.has(id));
    for (const cb of subscribers) cb(currentView!);
    if (enabledChanged) {
      // 通过 subscribe 让 SpotlightPanel 决定是否重跑 prefetchAll
      // 这里只传 view,enabled 比较由订阅者按需做
    }
  });
}

// 在 ensureLoaded 第一次执行时调
```

#### `apps/desktop/src/components/SpotlightPanel.vue`(增量)

```typescript
import { subscribe as subscribeConfig } from "../spotlight/config-store";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

let unsubConfig: (() => void) | null = null;
let unlistenWindowShown: UnlistenFn | null = null;

onMounted(async () => {
  // ... 现有代码
  await configStore.ensureLoaded();
  view.value = configStore.getView();

  unsubConfig = subscribeConfig(async (nextView) => {
    const prevEnabledIds = new Set(
      view.value?.providers.filter((p) => p.enabled).map((p) => p.id) ?? [],
    );
    const nextEnabledIds = new Set(nextView.providers.filter((p) => p.enabled).map((p) => p.id));
    view.value = nextView;
    const enabledChanged =
      prevEnabledIds.size !== nextEnabledIds.size ||
      [...prevEnabledIds].some((id) => !nextEnabledIds.has(id)) ||
      [...nextEnabledIds].some((id) => !prevEnabledIds.has(id));
    if (enabledChanged) {
      await prefetchAll();
    }
    // alias / quick command 变化只刷 view,不重跑 prefetch
  });

  // 窗口显示兜底:复用现有 spotlight-reset 事件已经会重跑 prefetchAll,
  // 此处仅在事件回调里追加一次 ensureLoaded 兜底
  // (在原 listen("spotlight-reset") 回调内加 await configStore.ensureLoaded(true);)
});

onBeforeUnmount(() => {
  // ... 现有代码
  unsubConfig?.();
  unlistenWindowShown?.();
});
```

#### `apps/desktop/src/components/settings/SpotlightSettings.vue`(增量)

让设置面板也订阅配置变更(防止双窗口编辑漂移):
```typescript
let unsub: (() => void) | null = null;
onMounted(async () => {
  await configStore.ensureLoaded();
  syncFromView();
  unsub = configStore.subscribe(() => syncFromView());
});
onBeforeUnmount(() => unsub?.());
```

`syncFromView()`:从 `configStore.getView()` 读取当前状态,刷新本地 ref。

### 验证

- `pnpm typecheck` 通过
- `pnpm --filter @lazycat/desktop build:web` 通过
- `pnpm dev` 手测(对照设计 §07 手测清单的 8-11 项):
  - launcher 接入完整
  - 配置广播:主窗口改完 Spotlight 立即生效(可观察:Spotlight 已开着的情况下,在主窗口改 todo alias,Spotlight 立即按新 alias 解析)
  - 多窗口一致性:主窗口和 Spotlight 同开,主窗口改 → Spotlight 自动同步
  - 配置损坏恢复:手动改 user_settings 表中 `spotlight_config_v1` 为非法 JSON,重启 → 回落默认 + backup key 存在

### 可独立提交

✅ 是。M6 结束时整个 vNext 落地完成。

---

## 风险与回退

### 主要风险

| 风险 | 影响 | 缓解 |
|------|------|------|
| M2 改 6 个 provider,某个 provider 类型未对齐导致编译失败 | 中 | 一次只改一个 provider,改完立即 typecheck |
| M3 spotlight-query.ts 签名变化,SpotlightPanel.vue 多处调用未对齐 | 中 | 改完 spotlight-query.ts 立即 typecheck,SpotlightPanel.vue 编辑前用 Grep 找全所有调用点 |
| M3 user_settings 读写竞争 | 低 | 设计的 `inFlightSave` 排队机制覆盖 |
| M5 SettingsPanel 现有 section 顺序影响视觉,Spotlight 放错位置 | 低 | 设计文档已固化为「系统集成」之前;改完 dev 截图对比 |
| M6 跨窗口 emit 在 Tauri 2 上可能需要显式指定 target | 中 | 第一次实施时先用全局 emit,若 Spotlight 窗口收不到,改为 `app.emit_to(SPOTLIGHT_LABEL, ...)` |

### 回退策略

每个 milestone 独立 commit。若线上发现某 milestone 引入问题:
- M6 出问题 → revert M6,跨窗口同步失效但功能整体可用,用户需重启 Spotlight 才能让配置生效
- M5 出问题 → revert M5,设置 UI 不可见,但 store 仍可通过手改数据库验证
- M3/M4 出问题 → revert 至 M2,行为退化到 v0.6 等价(provider 元信息变了存储位置,但运行时一致)
- M1/M2 出问题 → revert 至 main,无影响

---

## 接入顺序与停顿点

按推荐流程,**每个 milestone 完成后停下来等用户 review**,确认无误再进入下一步。

- M1 完成后:仓库依然可构建可运行,无行为变化
- M2 完成后:行为应与 v0.6 完全一致,如有差异立即排查
- M3 完成后:行为仍与 v0.6 一致(用户未动配置),但可通过手改数据库验证用户覆盖生效
- M4 完成后:launcher 可用,其它行为不变
- M5 完成后:UI 可用,但跨窗口需手动重新呼出
- M6 完成后:全功能就位

## 不在本计划内

- `process.md` 经验沉淀:M6 完成后再评估是否要把"descriptor + 配置覆盖"模式记入 `process.md`(若使用次数 ≥ 3 次)
- 单测覆盖率提升:M3 的 config-store.test.ts 已覆盖主要路径,SpotlightSettings.vue 的 UI 单测留给后续
- E2E:`pnpm test:e2e` 不在每个 milestone 强制跑,只在 M6 完成、准备出版本时统一跑一次
