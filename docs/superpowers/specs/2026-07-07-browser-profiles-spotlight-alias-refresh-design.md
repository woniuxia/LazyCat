# 浏览器身份 Spotlight 别名刷新设计

## 背景

浏览器身份工具已经支持给 Edge Profile 设置 LazyCat 别名，Spotlight provider 也会把别名加入 `searchFields`。现象是：用户在「浏览器身份」面板里把某个可见 Profile 的别名改成新值后，Spotlight 仍能用 Edge 显示名或 `Profile 2` 这类目录名搜到该身份，但结果展示和搜索字段仍是旧别名。

这说明当前问题不是搜索字段缺失，而是 Spotlight 驻留窗口中的 `browser-profiles` 预取缓存没有在浏览器身份配置变更后及时失效。

## 目标

1. 用户保存浏览器身份别名后，Spotlight 中该身份的标题和别名搜索字段立即更新。
2. 用户隐藏、恢复、修改 Edge 路径或启动 Profile 后，Spotlight 中浏览器身份结果同步刷新。
3. 刷新范围限定为 `browser-profiles` provider，不触发 Spotlight 全量重建。
4. 保持现有搜索字段规则：LazyCat 别名、Edge 显示名、Profile 目录名共用 `buildBrowserProfileSearchFields`。
5. 局部刷新失败时不破坏现有 Spotlight 可用性。

## 非目标

1. 不把浏览器身份改成 query-time provider。
2. 不修改浏览器身份后端存储模型。
3. 不新增数据库表或迁移。
4. 不改变隐藏 Profile 不出现在 Spotlight 的现有规则。
5. 不重构 Spotlight provider 注册、排序或全局搜索算法。

## 当前链路

浏览器身份面板：

1. `BrowserProfilesPanel.vue` 调用 `tool:browser-profiles:list` 拉取 Profile。
2. 用户保存别名时调用 `tool:browser-profiles:save-alias`。
3. 保存成功后面板调用 `loadProfiles()`，面板内列表可以看到新别名。

Spotlight：

1. `SpotlightPanel.vue` 启动时执行 `prefetchAll()`。
2. `browserProfilesProvider.prefetch()` 调用 `tool:browser-profiles:list`。
3. provider 把未隐藏 Profile 映射成 `SpotlightItem`，其中 `title` 和 `searchFields` 都来自当前 alias。
4. 结果保存在 `itemsByProvider`。

缺口是：主窗口保存别名后，只有主窗口面板刷新；Spotlight 窗口里的 `itemsByProvider["browser-profiles"]` 没有收到刷新信号。

## 方案

采用前端跨窗口事件通知：

```text
BrowserProfilesPanel.vue
  save alias / set hidden / set edge path / launch
    -> notifyBrowserProfilesChanged(reason)

browserProfilesProvider.defaultAction()
  successful launch
    -> notifyBrowserProfilesChanged("launch")

SpotlightPanel.vue
  listenBrowserProfilesChanged(handler)
    -> refreshBrowserProfilesProvider()
    -> replace itemsByProvider["browser-profiles"]
```

这里的通知必须使用 Tauri 跨窗口事件 API，不是 Vue 组件 `emit`。为避免事件名、payload 和 API 作用域散落在组件里，新增一个小封装模块，例如：

```text
apps/desktop/src/spotlight/browser-profiles-events.ts
```

该模块统一导出事件常量、payload 类型、通知函数和监听函数。`BrowserProfilesPanel.vue` 只调用通知函数；`SpotlightPanel.vue` 只调用监听函数。

### 事件名称

新增事件名：

```ts
const BROWSER_PROFILES_CHANGED_EVENT = "browser-profiles-changed";
```

事件 payload 保持小而稳定：

```ts
interface BrowserProfilesChangedPayload {
  reason: "alias" | "hidden" | "edge-path" | "launch";
}
```

当前功能只依赖“有变更”这一事实，不依赖具体字段。`reason` 仅用于调试和后续扩展。

事件封装示意：

```ts
export const BROWSER_PROFILES_CHANGED_EVENT = "browser-profiles-changed";

export async function notifyBrowserProfilesChanged(
  reason: BrowserProfilesChangedPayload["reason"],
): Promise<void> {
  const { emit } = await import("@tauri-apps/api/event");
  await emit(BROWSER_PROFILES_CHANGED_EVENT, { reason });
}

export async function listenBrowserProfilesChanged(
  handler: (payload: BrowserProfilesChangedPayload) => void | Promise<void>,
): Promise<UnlistenFn> {
  const { listen } = await import("@tauri-apps/api/event");
  return listen<BrowserProfilesChangedPayload>(
    BROWSER_PROFILES_CHANGED_EVENT,
    (event) => void handler(event.payload),
  );
}
```

### 变更入口

所有会改变浏览器身份 Spotlight 展示数据的前端入口，都要在后端操作成功后广播事件。

`BrowserProfilesPanel.vue`：

1. `editAlias()`：别名保存成功后。
2. `setHidden()`：隐藏或恢复成功后。
3. `chooseEdgePath()`：Edge 路径保存成功后。
4. `launchProfile()`：启动成功并更新统计后。

`spotlight/providers/browser-profiles.ts`：

1. `launchProfile()` / 默认动作：通过 Spotlight 启动 Profile 成功后，也必须广播 `notifyBrowserProfilesChanged("launch")`。
2. 该通知用于刷新 `launchCount`、`lastLaunchedAt` 和空输入高频排序。

如果未来新增其他入口调用 `tool:browser-profiles:launch` 并成功更新统计，也必须在同一入口成功后通知刷新。实现时可抽一个小的前端 launch helper 来统一调用 `tool:browser-profiles:launch` 和 `notifyBrowserProfilesChanged("launch")`，但不强制大范围重构。

广播失败不阻断主流程。用户已经完成的保存、隐藏或启动操作不能因为通知 Spotlight 失败而回滚。

面板侧调用要求：

1. 只在后端操作成功后通知。
2. 通知失败时捕获并忽略，不展示错误消息。
3. 不在保存前乐观通知，避免 Spotlight 提前拉取到旧数据。

### Spotlight 局部刷新

`SpotlightPanel.vue` 新增监听：

1. `onMounted` 时注册 `listenBrowserProfilesChanged(refreshBrowserProfilesProvider)`。
2. `onBeforeUnmount` 时取消监听。
3. `refreshBrowserProfilesProvider()` 只调用 `browserProfilesProvider.prefetch()`。
4. 成功后复制当前 `itemsByProvider`，替换 `browser-profiles` 对应数组，再整体赋回 ref。
5. 多次刷新并发时使用请求序号保证 latest-wins，旧请求不能覆盖新请求。
6. 与 `prefetchAll()` 共用 `browser-profiles` provider 写入版本，较早的全量预取不能覆盖较新的局部刷新。

示意：

```ts
let browserProfilesWriteVersion = 0;

async function refreshBrowserProfilesProvider() {
  const version = ++browserProfilesWriteVersion;
  try {
    const items = await browserProfilesProvider.prefetch();
    if (version !== browserProfilesWriteVersion) return;
    const next = new Map(itemsByProvider.value);
    next.set("browser-profiles", items);
    itemsByProvider.value = next;
    activeIndex.value = nextSpotlightActiveIndex({
      currentIndex: activeIndex.value,
      resultCount: results.value.length,
      queryChanged: false,
    });
  } catch (err) {
    if (version !== browserProfilesWriteVersion) return;
    console.warn("[Spotlight] refresh browser profiles failed:", err);
  }
}
```

不清空旧数据。这样即使后端临时扫描失败，Spotlight 仍保留上一次可用结果；下一次 `spotlight-reset` 里的现有 `prefetchAll()` 仍会兜底刷新。

局部刷新期间不清空 query，但刷新完成后必须重新约束 `activeIndex`。如果隐藏 Profile 导致结果数量减少，当前选中项不能越界；如果当前结果消失，键盘确认应落到新结果列表中的有效项或无结果状态。

### 与全量预取的竞争

`prefetchAll()` 现有行为是每个 provider 完成后渐进式写回 `itemsByProvider`。这会和局部刷新产生竞争：全量预取先发起，用户随后保存别名并触发局部刷新，若全量预取最后返回旧数据，就可能覆盖新别名。

实现时需要让 `browser-profiles` provider 的所有写回共用同一个 `browserProfilesWriteVersion`：

1. `refreshBrowserProfilesProvider()` 发起时递增 `browserProfilesWriteVersion`，写回前必须确认版本仍相同。
2. `prefetchAll()` 对 `browser-profiles` 发起请求前捕获当前版本。
3. `prefetchAll()` 写回 `browser-profiles` 前检查捕获版本是否仍等于当前版本。
4. 若版本已变化，说明期间发生了更近的浏览器身份局部刷新，全量预取结果必须丢弃，不能覆盖 `itemsByProvider["browser-profiles"]`。
5. 其他 provider 不需要受这个版本控制影响。

这样既保留 `prefetchAll()` 的渐进式加载，又保证浏览器身份变更后的最新结果不会被旧请求回滚。

### 监听生命周期

`listenBrowserProfilesChanged()` 返回的是异步 `UnlistenFn`。实现时需要处理组件在监听注册完成前已经卸载的情况，避免重复监听残留：

1. `onMounted` 中保存 `unlistenBrowserProfilesChanged: UnlistenFn | null` 和 `disposed` 标记。
2. 监听 Promise resolve 后，如果组件已卸载，立即调用返回的 `unlisten`。
3. `onBeforeUnmount` 中设置 `disposed = true`，并调用已经拿到的 `unlisten`。
4. 重复打开/关闭 Spotlight 不应留下多个监听器。

## 数据一致性

同一个浏览器身份的 `itemId` 保持为：

```text
edge:<profileDir>
```

局部刷新时直接替换该 provider 的结果数组，而不是合并新旧数组。原因：

1. 别名、隐藏状态和启动次数都是同一个 Profile 的可变展示/排序数据。
2. 合并会保留旧 `SpotlightItem`，可能继续暴露旧别名。
3. provider 级替换和现有 `prefetchAll()` 的语义一致，最容易验证。

Spotlight item 的启动 payload 只保留 `browser`、`profileDir` 和展示名，不缓存 Edge 可执行文件路径。默认动作执行时仍调用：

```text
tool:browser-profiles:launch
```

后端 `launch` 根据当前 `browser_profiles_config_v1` 读取最新 Edge 路径和 Profile 配置。因此修改 Edge 路径后，事件刷新只负责让 Spotlight 缓存同步；启动动作本身必须以执行时后端配置为事实源。

## 错误处理

1. 面板侧事件广播失败：静默忽略，保存/隐藏/启动流程照常成功。
2. Spotlight 侧刷新失败：保留旧 `itemsByProvider`，写 `console.warn`。
3. Spotlight 窗口尚未创建：事件无人监听，不需要补偿；下一次窗口创建或唤起时已有 `prefetchAll()` 兜底。
4. Spotlight 刷新期间用户正在输入：不清空 query；结果重新计算后重新 clamp `activeIndex`。
5. 多个刷新请求乱序完成：只有最新请求允许写回 `itemsByProvider` 和 `activeIndex`。
6. 全量预取和局部刷新乱序完成：全量预取只能在 `browserProfilesWriteVersion` 未变化时写回 `browser-profiles`。

## 测试策略

优先补前端单测：

1. `browser-profiles` provider 现有测试已覆盖 alias 会进入 `searchFields`，保留。
2. 新增可测 helper 或纯函数，验证用新 provider items 替换 `itemsByProvider["browser-profiles"]` 后，旧别名结果不再保留。
3. 如抽出事件常量，测试和组件共用同一常量，避免事件名拼写漂移。
4. 测试 `notifyBrowserProfilesChanged()` 使用 Tauri 事件 API 发送 `browser-profiles-changed`，而不是 Vue 组件事件。
5. 测试监听封装能把 Tauri event payload 转交给 handler，并返回可调用的 `UnlistenFn`。
6. 测试连续两次局部刷新乱序返回时，较早请求不能覆盖较晚请求。
7. 测试局部刷新后 `activeIndex` 会随新结果数量收敛到有效范围。
8. 测试 `prefetchAll()` 先发起、局部刷新后完成、`prefetchAll()` 最后返回时，旧的 `browser-profiles` 结果不能覆盖新别名。
9. 测试浏览器身份 Spotlight 默认动作只通过 `profileDir` 调用后端 `launch`，不依赖 item 中缓存的 Edge 路径。
10. 测试 Spotlight 默认动作启动成功后会调用 `notifyBrowserProfilesChanged("launch")`，从而触发浏览器身份 provider 局部刷新。
11. 如组件测试成本可控，覆盖面板保存别名、隐藏、恢复、路径变更、启动成功后会调用通知函数。

建议验证命令：

```text
pnpm test src/utils/browserProfiles.test.ts src/spotlight/providers/browser-profiles.test.ts src/spotlight/search.test.ts
pnpm test src/spotlight/browser-profiles-events.test.ts
pnpm typecheck
```

如改动触及 `SpotlightPanel.vue` 或面板事件逻辑，必要时补：

```text
pnpm --filter @lazycat/desktop build:web
```

## 验收标准

1. 打开浏览器身份面板，把可见 Profile 的别名从旧值改成新值。
2. 不重启应用，不依赖关闭 Spotlight 窗口。
3. 呼出 Spotlight，用新别名可以搜到该 Profile。
4. Spotlight 结果标题展示新别名。
5. 用旧别名不再命中该 Profile。
6. 用 Edge 显示名或 Profile 目录名仍可命中。
7. 隐藏 Profile 后 Spotlight 不再展示；恢复后重新可搜。
8. 连续快速修改别名时，Spotlight 最终展示最后一次保存的别名。
9. 启动 Profile 后，Spotlight 的空输入高频结果按最新 `launchCount` / `lastLaunchedAt` 重新排序。
10. 修改 Edge 路径后，不重启应用即可通过 Spotlight 启动浏览器身份，启动动作使用后端最新配置。
11. 反复打开和关闭 Spotlight 后，一次浏览器身份变更只触发一次局部刷新。

## 风险与取舍

1. 事件通知是前端级一致性，不改变后端事实源；如果事件丢失，下一次 Spotlight reset 仍会刷新。
2. 只刷新单 provider，避免因一个别名变更触发所有 provider 的 IPC 请求。
3. 不改成 query-time provider，因为浏览器身份数量有限，预取模型仍更适合空输入高频结果展示。
