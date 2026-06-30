# 接口调试左侧导航管理设计

## 概述

本次完善「接口调试」工具左侧集合和接口导航栏，让已有的集合、文件夹、接口模型真正可管理。现有后端已经具备集合、文件夹、接口的基础 CRUD，并且数据表已包含 `parent_id`、`folder_id`、`sort_order`。前端当前只展示集合和当前集合下的扁平接口列表，缺少文件夹树、右键菜单、移动和排序入口。

设计目标是在不改变请求发送、环境变量、历史和 Markdown 导出语义的前提下，补齐左侧导航的管理能力。用户选择方案 B：在完整树管理基础上同步补后端移动和排序 action，为后续拖拽排序预留稳定接口，但本次不实现拖拽。

## 目标

1. 左侧展示集合列表和当前集合下的多级接口树。
2. 支持多级文件夹，文件夹可以包含子文件夹和接口。
3. 支持右键菜单管理集合、文件夹、接口和空白区域。
4. 支持新建、重命名、删除集合。
5. 支持新建、重命名、删除文件夹。
6. 支持打开、重命名、删除接口。
7. 支持接口保存到指定文件夹或未分组。
8. 支持文件夹和接口移动到其他位置。
9. 支持文件夹和接口在同级内上移、下移排序。
10. 删除文件夹时保留内部接口，把接口移到未分组。
11. 后端显式校验跨集合归属、移动目标合法性和排序列表完整性。

## 非目标

1. 不做拖拽排序。
2. 不做接口复制。
3. 不做批量操作。
4. 不做导航搜索过滤。
5. 不新增数据库表。
6. 不改变请求发送、环境变量、历史记录或 Markdown 导出规则。

## 交互设计

### 左侧布局

左侧仍分为两段：

1. 集合区：显示「接口集合」标题、新建集合按钮和集合列表。
2. 接口树区：显示当前集合下的未分组接口、根文件夹和文件夹内接口。

集合行显示集合名称和接口数量。当前选中集合高亮。接口行显示 Method badge 和接口名称。文件夹行显示展开箭头、文件夹图标、名称和子项数量。

### 树形结构

当前集合的 `folders` 和 `requests` 由前端纯函数组装为树：

1. `parentId: null` 的文件夹作为根文件夹。
2. `folderId: null` 的接口进入「未分组」节点。
3. 子文件夹挂到对应父文件夹下。
4. 文件夹内接口挂到对应文件夹下。
5. 同级文件夹和接口分别按 `sortOrder ASC, id ASC` 保持稳定顺序。

如果集合没有文件夹和接口，接口树区显示空态。空态提示用户可以通过右键或保存接口开始组织。

### 展开和选中

展开态保存在前端内存中，key 使用 `collectionId + folderId`。切换集合时只显示该集合的展开状态。

新建文件夹后自动展开父级。打开接口时自动展开该接口所在文件夹及其祖先。当前打开的接口高亮；只选中集合但未打开接口时，不高亮接口。

### 右键菜单

右键菜单使用 Teleport 渲染到 `body`，定位复用 `clampContextMenuPosition`，避免超出视口。菜单支持点击外部关闭、Escape 关闭、危险项样式。

空白区菜单：

- 新建集合
- 新建根文件夹（存在当前集合时显示）

集合菜单：

- 选择集合
- 新建文件夹
- 重命名
- 导出 Markdown
- 删除

文件夹菜单：

- 新建子文件夹
- 重命名
- 移动到
- 上移
- 下移
- 删除

接口菜单：

- 打开
- 重命名
- 移动到
- 上移
- 下移
- 删除

### 移动目标

移动接口时，目标列表包含「未分组」和当前集合内所有文件夹。

移动文件夹时，目标列表包含「根级」和当前集合内合法文件夹。合法文件夹必须满足：

1. 不等于当前文件夹。
2. 不是当前文件夹的后代。
3. 属于同一集合。

第一版移动入口使用轻量弹窗选择目标，不做拖拽。

### 删除确认

删除集合必须二次确认，提示集合名和影响范围：接口、文件夹、环境会删除，历史引用按后端外键置空或清理。

删除文件夹必须二次确认，提示文件夹会删除，内部接口会移到未分组。子文件夹结构会删除，子文件夹内接口同样移到未分组。

删除接口必须二次确认，提示接口名。删除接口不删除历史记录。

## 前端架构

### `ApiWorkbenchPanel.vue`

保留页面总编排和现有核心流程：

1. 加载集合、环境和历史。
2. 选择集合和环境。
3. 请求编辑、发送、保存。
4. 响应展示。
5. 环境变量保存。
6. Markdown 导出触发。

面板把左侧管理能力委托给新的 sidebar 组件。管理动作成功后调用 `loadAll()`，以 SQLite 返回结果作为单一真源。

### `ApiWorkbenchSidebar.vue`

新增侧边栏组件，负责：

1. 渲染集合列表。
2. 渲染当前集合接口树。
3. 管理展开和选中展示。
4. 捕获右键目标并打开菜单。
5. 向父组件发出选择集合、打开接口、创建、重命名、删除、移动、排序等事件。

组件不直接维护业务真源，只接收 `collections`、`selectedCollectionId`、`selectedRequestId` 等状态，并通过事件请求父组件执行 IPC。

### `ApiWorkbenchContextMenu.vue`

新增通用右键菜单组件，输入菜单项数组：

```ts
interface ApiWorkbenchMenuItem {
  key: string;
  label: string;
  danger?: boolean;
  disabled?: boolean;
  divider?: boolean;
}
```

组件只负责展示和选择，不包含业务逻辑。

### `utils/apiWorkbenchTree.ts`

新增纯函数模块，职责包括：

1. 构造接口树。
2. 计算文件夹祖先链。
3. 生成移动目标列表。
4. 过滤文件夹非法移动目标。
5. 计算上移、下移后的完整 id 顺序。
6. 构造 reorder payload。

这些逻辑配套单测，避免树结构和排序行为堆在 Vue 组件里。

### 类型

在 `types/api-workbench.ts` 补充树节点、菜单 target、移动和排序 payload 类型。类型只描述前后端边界和组件事件，不引入运行时复杂抽象。

## 后端接口设计

继续使用 `api_workbench` domain，新增以下 channel/action：

| Channel | Action | 说明 |
|---|---|---|
| `tool:api-workbench:folder-move` | `folder_move` | 移动文件夹到根级或目标父文件夹 |
| `tool:api-workbench:request-move` | `request_move` | 移动接口到未分组或目标文件夹 |
| `tool:api-workbench:folder-reorder` | `folder_reorder` | 重排同一父级下的文件夹 |
| `tool:api-workbench:request-reorder` | `request_reorder` | 重排同一文件夹下的接口 |

### `folder_move`

Payload：

```json
{
  "id": 1,
  "targetParentId": 2
}
```

`targetParentId` 可以为 `null`，表示移动到根级。

后端规则：

1. 文件夹必须存在。
2. 目标父文件夹为非空时必须存在。
3. 目标父文件夹必须与当前文件夹属于同一集合。
4. 不能移动到自己。
5. 不能移动到自己的后代。
6. 移动后写入目标父级末尾 `sort_order`。

### `request_move`

Payload：

```json
{
  "id": 10,
  "targetFolderId": null
}
```

`targetFolderId` 可以为 `null`，表示移动到未分组。

后端规则：

1. 接口必须存在。
2. 目标文件夹为非空时必须存在。
3. 目标文件夹必须与接口属于同一集合。
4. 移动后写入目标文件夹或未分组末尾 `sort_order`。

### `folder_reorder`

Payload：

```json
{
  "collectionId": 1,
  "parentId": null,
  "orderedIds": [3, 2, 5]
}
```

后端规则：

1. `orderedIds` 必须是同一 `collectionId + parentId` 下完整文件夹 id 集合。
2. 不能遗漏、重复或夹带其他父级/集合的 id。
3. 事务内按数组顺序写入 gapless `sort_order`。

### `request_reorder`

Payload：

```json
{
  "collectionId": 1,
  "folderId": null,
  "orderedIds": [11, 12, 10]
}
```

后端规则：

1. `orderedIds` 必须是同一 `collectionId + folderId` 下完整接口 id 集合。
2. 不能遗漏、重复或夹带其他文件夹/集合的 id。
3. 事务内按数组顺序写入 gapless `sort_order`。

### `folder_delete` 调整

当前 `folder_delete` 直接删除文件夹。新规则要求删除文件夹前先保留接口：

1. 找出目标文件夹及其所有后代文件夹 id。
2. 把这些文件夹内的接口 `folder_id` 更新为 `NULL`。
3. 删除目标文件夹，子文件夹按外键级联删除。

整个过程在事务内完成。删除完成后接口出现在「未分组」。

## 错误处理

后端错误必须显式返回，前端直接展示，不做静默兜底。

典型错误：

1. `文件夹不存在`
2. `接口不存在`
3. `目标文件夹不存在`
4. `目标文件夹不属于当前集合`
5. `不能移动到自己的子文件夹`
6. `排序列表不完整`
7. `排序列表包含重复项`
8. `排序列表包含其他集合的数据`

前端动作执行期间禁用对应菜单入口或按钮，避免重复提交。失败后保留当前 UI 状态，并提示错误。

## 测试计划

### 前端纯函数测试

新增 `src/utils/apiWorkbenchTree.test.ts`，覆盖：

1. 多级文件夹树构造。
2. 未分组接口节点。
3. 同级排序稳定性。
4. 祖先链计算。
5. 移动目标列表生成。
6. 文件夹移动目标过滤自己和后代。
7. 上移、下移生成完整排序 id。

### 前端组件验证

若当前测试环境适合，补 `ApiWorkbenchSidebar` 组件测试，覆盖：

1. 右键集合、文件夹、接口触发不同 target。
2. 点击接口发出打开事件。
3. 空态和未分组节点展示。

如果组件测试成本过高，优先用纯函数测试、`pnpm typecheck` 和 `build:web` 覆盖主要风险。

### Rust 单测

在 `api_workbench.rs` 测试中新增：

1. `folder_move` 可以移动到根级和目标父文件夹。
2. `folder_move` 禁止移动到自己和后代。
3. `request_move` 可以移动到文件夹和未分组。
4. `folder_reorder` 事务内写入 gapless 顺序。
5. `request_reorder` 事务内写入 gapless 顺序。
6. reorder 遇到遗漏、重复、跨集合 id 时返回错误。
7. 删除文件夹后内部接口保留并进入未分组。

### 验证命令

```powershell
cargo test api_workbench -- --nocapture
pnpm test src/utils/apiWorkbench.test.ts src/utils/apiWorkbenchTree.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

## 风险与取舍

1. 本次选择补移动和排序后端 action，改动面大于纯前端导航，但能避免后续拖拽排序时再改后端协议。
2. 暂不做拖拽，降低交互复杂度；上移、下移和移动弹窗先覆盖可管理性。
3. 删除文件夹保留接口，降低误删风险；代价是删除后未分组可能变多，但结果更安全。
4. 展开态先保存在内存，不写 `user_settings`；重启后恢复默认展开，避免为低频偏好扩持久化模型。

