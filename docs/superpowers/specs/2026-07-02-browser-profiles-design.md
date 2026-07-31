# 浏览器身份启动器设计

## 概述

新增轻量工具「浏览器身份」，内部工具 ID 为 `browser-profiles`。首版只支持 Microsoft Edge Profile，目标是把测试过程中频繁打开不同 Edge 用户窗口的动作收敛为 LazyCat 内的一键启动和 Spotlight 快速启动。

本设计只解决“打开指定 Edge Profile”。不做账号密码管理、不做自动登录、不绑定固定 URL，也不读取 Edge Cookie、Token、浏览记录或收藏夹。用户仍通过 Edge 自身维护登录状态，LazyCat 只负责发现 Profile、维护别名和启动入口。

## 目标

1. 自动发现本机 Edge Profile，例如 `Default`、`Profile 1`、`Profile 2`。
2. 尽量读取 Edge `Local State` 中的 Profile 显示名，展示更接近 Edge UI 的名称。
3. 支持用户在 LazyCat 内为 Profile 设置别名，例如“管理员”“普通用户”“测试账号 A”。
4. 支持隐藏不常用 Profile，并允许恢复。
5. 支持记录打开次数和最近打开时间，默认按高频使用排序。
6. 支持从面板点击启动指定 Edge Profile。
7. 支持 Spotlight 搜索并启动常用 Profile。
8. 找不到 Edge 时允许用户手动选择 `msedge.exe` 路径。
9. 完全离线运行，不新增运行时公网依赖。

## 非目标

1. 不自动填写账号密码。
2. 不自动提交登录表单。
3. 不绑定固定 URL，也不打开指定业务系统页面。
4. 不读取或修改 Edge Cookie、Token、历史记录、收藏夹、扩展数据。
5. 不管理 Edge 账号本身，不创建、删除或重命名 Edge Profile。
6. 不支持 Chrome、Firefox 或其他浏览器；后续可按同一模型扩展。
7. 不改造 Launcher 的通用参数编辑能力。
8. 不把浏览器 Profile 与 Hosts、API 环境或 Vault 凭据绑定成项目 Profile。

## 用户流程

### 首次进入

1. 用户进入「浏览器身份」工具。
2. 后端扫描 Edge 可执行文件和 Edge 用户数据目录。
3. 面板展示发现到的 Profile 列表。
4. 用户可给常用 Profile 设置别名。
5. 用户可隐藏不常用 Profile。

### 启动 Profile

1. 用户在列表中点击某个 Profile 的「启动」。
2. 前端调用 `tool:browser-profiles:launch`。
3. 后端使用 Edge 可执行文件启动指定 Profile：

```text
msedge.exe --profile-directory=Profile 2
```

4. 启动成功后更新 `launchCount` 和 `lastLaunchedAt`。
5. 列表重新按打开次数排序。

### Spotlight 快速启动

1. 用户呼出 Spotlight。
2. 输入 LazyCat 别名、Edge 显示名或 Profile 目录名。
3. 命中「浏览器身份」结果。
4. 按 Enter 直接启动对应 Edge Profile。
5. 启动成功后关闭 Spotlight，并提示 `已打开 Edge：管理员`。

## 前端接入

### 工具入口

修改：

- `apps/desktop/src/composables/toolCatalog.ts`：新增工具定义。
- `apps/desktop/src/tool-registry.ts`：注册 `BrowserProfilesPanel.vue`。
- `apps/desktop/src/bridge/tauri.ts`：新增 `tool:browser-profiles:*` channel。

新增：

- `apps/desktop/src/components/BrowserProfilesPanel.vue`
- `apps/desktop/src/types/browser-profiles.ts`
- `apps/desktop/src/utils/browserProfiles.ts`
- `apps/desktop/src/utils/browserProfiles.test.ts`
- `apps/desktop/src/spotlight/providers/browser-profiles.ts`
- `apps/desktop/src/spotlight/providers/browser-profiles.test.ts`（按实际测试结构可调整）

### 面板结构

面板保持轻量：

1. 顶部状态区：
   - Edge 是否已发现。
   - Profile 数量。
   - 刷新按钮。
   - 找不到 Edge 时提供“选择 msedge.exe”入口。
2. 常用列表：
   - 默认展示未隐藏 Profile。
   - 按 `launchCount DESC` 排序。
   - 展示别名、Edge 显示名、Profile 目录名、打开次数、最近打开。
   - 操作：启动、编辑别名、隐藏。
3. 已隐藏分组：
   - 默认折叠。
   - 展示隐藏 Profile。
   - 操作：恢复、编辑别名。

展示名优先级：

```text
LazyCat 别名 > Edge 显示名 > Profile 目录名
```

首版不提取 Profile 头像。Edge 头像来源和格式不稳定，收益低于先把启动链路做稳。

## Spotlight 接入

新增 provider：`browser-profiles`。

规则：

1. 默认启用。
2. 不新增默认 scope alias，全局搜索即可命中。
3. 只展示未隐藏 Profile。
4. 搜索字段：
   - LazyCat 别名，权重最高。
   - Edge 显示名。
   - Profile 目录名。
5. 权重：
   - 基础权重高于普通工具入口，低于精确 keyword command。
   - `launchCount` 增加结果权重，使常用 Profile 自动靠前。
6. 默认动作：启动 Profile。
7. 备选动作：
   - 启动。
   - 跳转到浏览器身份工具。

## 后端接入

新增 Rust 模块：

- `apps/desktop/src-tauri/src/tools/browser_profiles.rs`

修改：

- `apps/desktop/src-tauri/src/tools/mod.rs`：注册 `browser_profiles` domain。

不修改 `helpers.rs`，首版不新增业务表。配置读写沿用现有 `launcher` 分组配置的模式，直接通过 `db_conn()` 访问 `user_settings`。

### IPC action

| Channel                               | Action          | 说明                                 |
| ------------------------------------- | --------------- | ------------------------------------ |
| `tool:browser-profiles:list`          | `list`          | 扫描 Edge Profile 并合并用户配置     |
| `tool:browser-profiles:save-alias`    | `save_alias`    | 保存 Profile 别名                    |
| `tool:browser-profiles:set-hidden`    | `set_hidden`    | 隐藏或恢复 Profile                   |
| `tool:browser-profiles:set-edge-path` | `set_edge_path` | 保存手动选择的 Edge 可执行文件路径   |
| `tool:browser-profiles:launch`        | `launch`        | 启动指定 Edge Profile 并更新使用统计 |

`list` 每次都重新扫描本机状态，不单独设计 `refresh` action。

### IPC payload / response

#### `list`

Payload：

```json
{}
```

Response：

```ts
interface BrowserProfilesListResponse {
  edgeFound: boolean;
  edgePath: string | null;
  userDataDir: string;
  probedEdgePaths: string[];
  warnings: string[];
  profiles: BrowserProfileItem[];
}

interface BrowserProfileItem {
  browser: "edge";
  profileDir: string;
  edgeDisplayName: string;
  alias: string;
  hidden: boolean;
  launchCount: number;
  lastLaunchedAt: string | null;
}
```

#### `save_alias`

Payload：

```json
{
  "browser": "edge",
  "profileDir": "Profile 2",
  "alias": "普通用户"
}
```

Response：

```json
{ "ok": true }
```

规则：

1. `profileDir` 必须是当前扫描结果中存在的 Profile。
2. `alias` 保存 trim 后文本；空字符串表示清空别名。
3. 后端不要求 alias 全局唯一，重复别名仍允许，搜索时靠显示名和目录名区分。

#### `set_hidden`

Payload：

```json
{
  "browser": "edge",
  "profileDir": "Profile 2",
  "hidden": true
}
```

Response：

```json
{ "ok": true }
```

#### `set_edge_path`

Payload：

```json
{
  "edgePath": "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe"
}
```

Response：

```json
{ "ok": true }
```

规则：

1. 路径必须存在。
2. 文件名必须大小写不敏感等于 `msedge.exe`。
3. 保存后 `list` 仍会重新扫描 Profile。

#### `launch`

Payload：

```json
{
  "browser": "edge",
  "profileDir": "Profile 2"
}
```

Response：

```json
{
  "ok": true,
  "launchCount": 9,
  "lastLaunchedAt": "2026-07-02T10:30:00+08:00",
  "warnings": []
}
```

规则：

1. `browser` 首版只接受 `edge`，其他值直接拒绝。
2. `profileDir` 必须是当前扫描结果中存在的 Profile。
3. 只有 `spawn()` 成功后才更新使用统计。
4. 使用统计写入与启动成功后的配置更新在同一数据库连接内完成；若统计写入失败，返回 warning，但不把已经成功的进程启动伪装成失败。

## Edge 发现规则

### Edge 可执行文件

优先级：

1. 用户手动配置的 `msedge.exe` 路径。
2. `%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe`
3. `%ProgramFiles%\Microsoft\Edge\Application\msedge.exe`
4. `%LOCALAPPDATA%\Microsoft\Edge\Application\msedge.exe`

如果都不存在，返回 `edgeFound = false`，并带上探测路径供前端展示。

### Edge 用户数据目录

首版使用默认目录：

```text
%LOCALAPPDATA%\Microsoft\Edge\User Data
```

后续如需支持自定义 `--user-data-dir`，另开设计。

### Profile 扫描

扫描来源：

1. `Local State`：读取 `profile.info_cache.<profileDir>.name` 作为 Edge 显示名。
2. Profile 目录：兜底发现 `Default` 和 `Profile *`。

稳定 key：

```text
browser = "edge"
profileDir = "Default" | "Profile 1" | "Profile 2"
```

不使用 Edge 显示名作为 key，因为显示名可能被用户修改。

## 用户配置

首版不新增表，用 `user_settings` 存 JSON。建议 key：

```text
browser_profiles_config_v1
```

结构：

```json
{
  "edgePath": "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "edge": {
    "Default": {
      "alias": "管理员",
      "hidden": false,
      "launchCount": 12,
      "lastLaunchedAt": "2026-07-02T10:30:00+08:00"
    },
    "Profile 2": {
      "alias": "普通用户",
      "hidden": false,
      "launchCount": 8,
      "lastLaunchedAt": "2026-07-02T09:00:00+08:00"
    }
  }
}
```

规则：

1. 扫描结果是事实源。
2. 用户配置是覆盖层。
3. Profile 被 Edge 删除后，`list` 不展示该项，但配置保留。
4. Edge 临时不可用时，不清理用户配置。
5. `launch` 成功后递增 `launchCount` 并更新 `lastLaunchedAt`。
6. 对 `browser_profiles_config_v1` 的读改写必须在单次操作内完成，避免别名更新和启动计数互相覆盖。
7. 保存配置时保留未知字段，便于后续版本追加 `pinned`、`url` 等字段时兼容旧实现。

## 排序规则

`list` 默认排序：

1. 未隐藏优先。
2. `launchCount DESC`。
3. `lastLaunchedAt DESC`。
4. 展示名按不区分大小写排序。
5. Profile 目录名兜底排序。

首版不提供手动排序，避免与打开次数排序形成双重真源。后续如用户需要固定置顶，再单独增加 `pinned` 字段，排序规则放在打开次数之前。

## 启动策略

Rust 启动时必须把 `--profile-directory=Profile 2` 作为一个完整参数传给 `Command`：

```rust
Command::new(msedge_path)
    .arg(format!("--profile-directory={profile_dir}"))
    .spawn()
```

不能复用 Launcher 当前的 `arguments.split_whitespace()` 路径，否则 `Profile 2` 会被拆成错误参数。

启动成功的判定以 `spawn()` 成功为准。首版不检测 Edge 窗口是否真的显示，也不等待进程退出。

## 错误处理

1. 找不到 Edge：返回明确状态，前端展示“未找到 Edge”，并提供手动选择入口。
2. 手动 Edge 路径无效：后端拒绝保存，提示必须选择存在的 `msedge.exe`。
3. 找不到 User Data：返回空 Profile 列表和扫描目录。
4. `Local State` 不存在或解析失败：按目录兜底展示，并返回 warning。
5. 指定 Profile 不存在：启动前拒绝，提示 Profile 已不存在。
6. 启动失败：返回原始错误，不更新打开次数。
7. 配置 JSON 解析失败：后端按空配置降级，并返回 warning；保存配置时覆盖为新结构。

错误必须显式暴露，不做伪成功。

## 安全与隐私

1. LazyCat 不读取 Edge Cookie、Token、密码、历史记录或收藏夹。
2. LazyCat 不修改 Edge Profile 内容。
3. LazyCat 只读取 `Local State` 中 Profile 显示信息和 Profile 目录结构。
4. 用户别名、隐藏状态、打开次数存入本地 SQLite 的 `user_settings`。
5. 启动命令只调用本机 `msedge.exe`，不访问外部服务。
6. 手动配置 Edge 路径时后端必须校验文件存在且文件名为 `msedge.exe`。

## 纯函数与测试边界

前端纯函数优先放在 `utils/browserProfiles.ts`：

1. 展示名选择。
2. Profile 排序。
3. 隐藏 / 未隐藏分组。
4. Spotlight item 映射所需搜索字段和权重计算。

后端纯函数优先放在 `browser_profiles.rs`：

1. Edge 路径候选生成。
2. `Local State` 解析。
3. Profile 目录筛选。
4. 扫描结果与用户配置合并。
5. 排序 key 生成。
6. Edge 启动参数构造。

## 验证计划

### Rust 单测

覆盖：

1. 从 `Local State` 解析 Profile 显示名。
2. `Local State` 解析失败时返回 warning 并允许目录兜底。
3. 目录兜底发现 `Default`、`Profile 1`、`Profile 2`。
4. 非 Profile 目录不进入结果。
5. 用户配置合并 alias、hidden、launchCount、lastLaunchedAt。
6. Profile 删除后扫描结果不展示旧配置项。
7. 排序按打开次数和最近打开时间生效。
8. `--profile-directory=Profile 2` 作为单个参数构造。
9. 无效 Edge 路径被拒绝。

建议命令：

```powershell
cargo test browser_profiles -- --nocapture
```

### 前端单测

覆盖：

1. 展示名优先级：alias > edgeDisplayName > profileDir。
2. 隐藏 / 未隐藏分组。
3. 打开次数排序。
4. 最近打开兜底排序。
5. Spotlight item 的搜索字段包含 alias、Edge 显示名和目录名。
6. Spotlight 权重随 `launchCount` 增加。

建议命令：

```powershell
pnpm test src/utils/browserProfiles.test.ts src/spotlight/providers/browser-profiles.test.ts
```

### 集成验证

```powershell
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

## 影响文件

预计涉及：

| 文件                                                       | 类型 | 说明                 |
| ---------------------------------------------------------- | ---- | -------------------- |
| `apps/desktop/src/composables/toolCatalog.ts`              | 修改 | 新增工具入口         |
| `apps/desktop/src/tool-registry.ts`                        | 修改 | 注册面板组件         |
| `apps/desktop/src/bridge/tauri.ts`                         | 修改 | 新增 channel         |
| `apps/desktop/src/components/BrowserProfilesPanel.vue`     | 新增 | 工具 UI              |
| `apps/desktop/src/types/browser-profiles.ts`               | 新增 | 类型定义             |
| `apps/desktop/src/utils/browserProfiles.ts`                | 新增 | 前端纯函数           |
| `apps/desktop/src/utils/browserProfiles.test.ts`           | 新增 | 前端单测             |
| `apps/desktop/src/spotlight/providers/browser-profiles.ts` | 新增 | Spotlight provider   |
| `apps/desktop/src-tauri/src/tools/browser_profiles.rs`     | 新增 | 后端扫描、配置、启动 |
| `apps/desktop/src-tauri/src/tools/mod.rs`                  | 修改 | 注册 domain          |

## 风险与取舍

1. Edge `Local State` 格式可能变化，因此显示名读取必须是增强能力，不能成为发现 Profile 的唯一来源。
2. Edge Profile 显示名可能为空或重复，因此稳定 key 只能用目录名。
3. 启动成功只能代表进程已创建，不能保证用户看到的窗口聚焦在前台；首版不做窗口管理。
4. 不复用 Launcher 参数系统，避免把通用启动器改大，也避免 `Profile 2` 参数拆分问题。
5. 不做 URL 绑定，保持本轮只解决“打开指定 Edge 用户窗口”这个高频动作。

## 后续扩展

后续按真实使用需求评估：

1. 支持 Chrome Profile。
2. 支持每个 Profile 绑定常用 URL。
3. 支持固定置顶。
4. 支持全局命名快捷键直达某个 Profile。
5. 支持 Profile 与测试账号备注做弱关联，但不保存密码。
6. 支持从 Launcher 创建 Edge Profile 启动项。
