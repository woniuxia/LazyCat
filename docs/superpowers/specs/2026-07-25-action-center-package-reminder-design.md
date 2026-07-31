# 动作中心与打包提醒 MVP 设计

日期：2026-07-25
状态：设计已确认

## 1. 背景与决策

LazyCat 已有 Todo 的单次事项、多提醒和独立全局通知窗口，也已有上线包项目配置、启动确认、长任务运行态和终态通知。当前需求是在 Todo 到点时提供“开始打包”，复用关联上线包配置和现有确认流程，并在打包完整成功后自动完成 Todo。

该能力按通用动作中心实现，但第一版不提供独立动作中心入口，只实现 `Todo -> 上线包打包`。模型和接口需允许后续增加：

- `Todo -> 打开浏览器身份`；
- `Todo -> 开始开发环境`。

动作中心不是任意脚本执行器，也不是第一版的自动化编排平台。它只管理触发源、已注册动作、已有目标配置、执行关联和结果策略；具体工具继续拥有确认、执行、日志和领域错误。

## 2. 目标与非目标

### 2.1 目标

- 单次 Todo 最多绑定一个已注册动作和一个已有目标配置。
- 第一版注册 `release_package.run`，目标必须是已有上线包项目。
- Todo 详情可手动触发动作；配置提醒后，提醒弹窗可触发同一动作。
- “开始打包”打开上线包页面、选择关联配置并进入现有确认流程，不绕过目标选择、覆盖确认、SSH 指纹、凭据或服务器预检。
- 动作中心持久化每次派发并关联实际上线包 `runId`。
- 只有上线包终态 `succeeded` 自动完成 Todo；其他终态均保留未完成。
- 普通 Todo 的创建、提醒、完成和稍后提醒行为保持不变。
- 数据和接口不依赖上线包专有字段，后续动作通过适配器接入。

### 2.2 非目标

- 不提供独立动作中心页面或侧栏入口。
- 不支持周期事项绑定动作。
- 不支持一个 Todo 绑定多个动作。
- 不支持动作链、条件表达式、自动重试和任意命令执行。
- 不实现浏览器身份或开发环境动作。
- 不在应用未启动时自动执行动作。
- 不把上线包参数、命令、SSH 秘密或其他目标配置复制到动作中心。

## 3. 核心概念

### 3.1 动作定义

动作定义由代码静态注册，不允许用户在数据库中创建任意动作。

```ts
interface ActionDefinition {
  type: string;
  label: string;
  triggerTypes: string[];
  targetKind: string;
  targetToolId: string;
  executionMode: "open_and_confirm" | "direct" | "background";
  completionPolicy: "on_started" | "on_succeeded" | "manual";
}
```

第一版定义：

```ts
{
  type: 'release_package.run',
  label: '开始打包',
  triggerTypes: ['todo_item'],
  targetKind: 'release_package_project',
  targetToolId: 'release-package',
  executionMode: 'open_and_confirm',
  completionPolicy: 'on_succeeded'
}
```

### 3.2 动作绑定

动作绑定描述“哪个触发对象要执行哪个动作，以及目标配置是谁”。第一版触发对象是单次 Todo，动作是上线包打包，目标是上线包项目 ID。

绑定只保存引用，不复制目标配置。目标删除或失效后保留绑定并显式展示错误，用户必须重新选择。

### 3.3 动作派发

每次点击“开始打包”创建一条 dispatch。dispatch 保存触发来源、目标快照、活动状态、外部运行 ID 和原始结果码，用于防重复、跨页面关联和 Todo 完成联动。

## 4. 总体架构

采用前后端协同的轻量动作中心：

```text
Todo 编辑/提醒窗口
        |
        v
动作中心：定义、绑定、派发、状态
        |
        v
主窗口打开目标工具并传递 dispatch intent
        |
        v
上线包：选择配置、现有确认、实际执行
        |
        v
上线包 Rust 运行时报告终态
        |
        v
动作中心结束 dispatch，并按策略完成 Todo
```

职责边界：

- Todo：事项、事件时间、提醒和最终完成状态。
- 动作中心：动作定义、目标查询、绑定、dispatch、并发和完成策略。
- 上线包：项目配置、启动参数、确认、构建、归档/上传、日志和原始终态。
- 前端：动作选择、页面路由和用户确认，不保存第二份业务真值。

## 5. 数据模型

### 5.1 `action_bindings`

```text
id              INTEGER PRIMARY KEY
trigger_type    TEXT NOT NULL
trigger_id      TEXT NOT NULL
action_type     TEXT NOT NULL
target_id       TEXT NOT NULL
enabled         INTEGER NOT NULL DEFAULT 1
created_at      TEXT NOT NULL
updated_at      TEXT NOT NULL
```

约束：

- `(trigger_type, trigger_id)` 唯一，第一版保证一个 Todo 最多一个动作。
- `trigger_id` 和 `target_id` 统一存字符串，兼容不同领域的整数、UUID 或其他稳定 ID。
- Todo 删除时删除绑定。
- 不对目标配置建立跨领域外键；每次展示和派发都由动作适配器校验目标。
- 存在活动 dispatch 时禁止更新、禁用或删除绑定。

### 5.2 `action_dispatches`

```text
id                  TEXT PRIMARY KEY
binding_id          INTEGER NULL
trigger_type        TEXT NOT NULL
trigger_id          TEXT NOT NULL
trigger_event_id    TEXT NOT NULL
action_type         TEXT NOT NULL
target_id           TEXT NOT NULL
status              TEXT NOT NULL
external_run_id     TEXT NULL
result_code         TEXT NULL
error               TEXT NULL
created_at          TEXT NOT NULL
started_at          TEXT NULL
finished_at         TEXT NULL
```

`status` 只允许：

```text
pending_confirmation
running
succeeded
failed
cancelled
```

合法转换：

```text
pending_confirmation -> running
pending_confirmation -> failed | cancelled
running              -> succeeded | failed | cancelled
```

约束与保留策略：

- `binding_id` 删除时置空；dispatch 保留触发、动作和目标快照。
- 同一绑定最多存在一个 `pending_confirmation` 或 `running` dispatch，使用后端事务和部分唯一索引双重保证。
- 提醒触发使用 Todo 提醒事件 ID 作为 `trigger_event_id`；详情手动触发使用新 UUID。
- `external_run_id` 关联上线包 `runId`。
- `result_code` 保存工具原始终态，例如 `partially_succeeded` 或 `package_succeeded_upload_failed`。
- 重复终态事件幂等忽略，错误 `runId` 不得更新 dispatch 或 Todo。

## 6. 后端模块与接口

新增 Rust 模块：

```text
action_center/
├─ definitions.rs
├─ bindings.rs
├─ dispatches.rs
└─ mod.rs
```

模块职责：

- `definitions.rs`：静态动作定义和适配器注册。
- `bindings.rs`：绑定校验、事务保存和目标摘要。
- `dispatches.rs`：创建派发、状态机、活动执行互斥、外部运行关联和启动恢复。
- `mod.rs`：IPC 分发以及供 Todo、上线包调用的内部接口。

第一版 IPC：

```text
tool:action-center:definition-list
tool:action-center:target-list
tool:action-center:binding-get
tool:action-center:dispatch
tool:action-center:dispatch-cancel
tool:action-center:dispatch-latest
```

外部运行关联不依赖前端收到 `runId` 后再二次回写。上线包启动 payload 携带可选 `actionDispatchId`，上线包后端在工作线程开始前完成 `dispatchId + runId` 关联，再启动流水线，避免极短运行产生终态竞态。普通手动打包不携带该字段，行为保持不变。

统一目标选项：

```ts
interface ActionTargetOption {
  id: string;
  label: string;
  available: boolean;
  unavailableReason?: string;
}
```

动作中心通过 `release_package.run` 适配器调用上线包项目查询与校验逻辑，不在动作中心重复解析上线包配置。

## 7. Todo 保存与展示

`TodoItemUpsertPayload` 增加：

```ts
actionBinding?: {
  actionType: string
  targetId: string
} | null
```

字段语义：

- 字段缺失：更新时保留原绑定；创建时表示无绑定。
- 对象：新增或替换绑定。
- `null`：解除绑定。

Todo 保存事务调用动作中心绑定存储，事项和绑定必须同时成功或同时回滚。后端拒绝为周期事项保存动作绑定。

Todo 返回通用摘要：

```ts
actionBinding?: {
  id: number
  actionType: string
  actionLabel: string
  targetId: string
  targetLabel: string
  available: boolean
  unavailableReason?: string
}
```

单次 Todo 创建/编辑区增加：

```text
执行动作：[无动作 / 上线包打包]
打包配置：[已有上线包配置]
```

交互规则：

- 选择动作后必须选择可用目标才能保存。
- 没有上线包配置时显示空态并引导用户前往上线包创建配置。
- 周期事项不展示动作字段；已绑定动作的单次事项切换为周期事项前必须解除绑定。
- 动作绑定与事件时间、提醒预设相互独立；无提醒时仍可从详情手动执行。
- Todo 详情展示动作、目标、最近执行状态和“开始打包”。
- 该功能不增加新的 Todo kind 或 status。

## 8. 提醒与执行流程

### 8.1 提醒负载

Todo 提醒派发附加可选通用动作摘要：

```ts
action?: {
  bindingId: number
  actionType: string
  actionLabel: string
  targetLabel: string
  available: boolean
  unavailableReason?: string
}
```

普通提醒仍显示：

```text
完成 / 知道了 / 稍后提醒
```

绑定动作的提醒显示：

```text
开始打包 / 知道了 / 稍后提醒
```

动作不可用时禁用主按钮并展示明确原因。提醒摘要只用于展示；点击时必须重新读取并校验最新绑定和目标。

### 8.2 创建派发

点击“开始打包”：

1. 独立提醒窗口调用 `tool:action-center:dispatch`。
2. 后端校验提醒事件、绑定、动作定义、目标配置和活动执行。
3. 后端创建 `pending_confirmation` dispatch。
4. 后端唤起主窗口并发送 `action-center://dispatch-request`。
5. 命令或事件发送失败时立即把 dispatch 标记为失败，Todo 保持未完成。

主窗口事件包含：

```ts
{
  dispatchId: string;
  actionType: "release_package.run";
  targetToolId: "release-package";
  targetId: string;
}
```

前端新增独立的 `useActionDispatchIntent` 承载该请求，不复用带剪贴板语义的 `useClipboardSuggestion` 共享状态。

### 8.3 上线包确认与启动

主窗口打开上线包页面，`ReleasePackagePanel` 消费 intent：

1. 若页面有不能安全覆盖的未保存配置，拒绝切换并把 dispatch 标记失败。
2. 加载 `targetId` 对应的最新上线包配置。
3. 调用现有 `prepareStart()` 流程。
4. 继续使用现有目标选择、本地目录覆盖确认、SSH 主机指纹、Vault/私钥认证和远程预检。
5. 用户取消或关闭确认时，将 dispatch 标记为 `cancelled`。
6. 用户确认后，把 `actionDispatchId` 随上线包启动请求发送到 Rust。
7. 后端关联 `runId` 后将 dispatch 置为 `running`，再开始实际流水线。

动作中心不得直接执行构建命令，也不得生成或猜测上线包启动参数。

### 8.4 终态联动

上线包后端在形成最终结果后调用动作中心内部终态接口：

| 上线包终态                        | Dispatch    | Todo       |
| --------------------------------- | ----------- | ---------- |
| `succeeded`                       | `succeeded` | 自动完成   |
| `partially_succeeded`             | `failed`    | 保持未完成 |
| `package_succeeded_upload_failed` | `failed`    | 保持未完成 |
| `failed`                          | `failed`    | 保持未完成 |
| `cancelled`                       | `cancelled` | 保持未完成 |

Todo 自动完成必须复用 Todo 后端状态变更语义，正确写入 `completed_at` 并清理后续提醒。Todo 已被用户完成时保持幂等；失败不得把已完成 Todo 改回未完成。

## 9. 并发、异常与恢复

### 9.1 重复触发

- 同一绑定有活动 dispatch 时，详情按钮禁用。
- 后续提醒显示“打包待确认”或“打包进行中”，不得再次创建 dispatch。
- 上线包自身的全局单任务互斥继续生效；若其他项目正在打包，本次 dispatch 失败，Todo 不完成。

### 9.2 配置变化

- 每次派发重新校验目标存在性和可用性。
- 进入上线包页面时加载最新配置。
- 配置删除后绑定显示失效，不静默解绑或回退到其他配置。
- 活动 dispatch 期间禁止修改或解除绑定。
- 上线包页面的未保存编辑不得被动作请求覆盖。

### 9.3 应用中断

应用启动时对账 dispatch：

- 遗留 `pending_confirmation` 标记为 `failed`，`result_code = interrupted`。
- 遗留 `running` 若无对应活动运行，同样标记为 `failed`。
- 不自动重试，不自动完成 Todo。

### 9.4 删除与手动完成

- 删除 Todo 时删除绑定，dispatch 通过快照保留历史。
- 删除正在执行的 Todo 不取消上线包；终态只结束 dispatch，不再完成不存在的 Todo。
- 打包期间手动完成 Todo 不取消打包；失败也不得重新打开 Todo。

## 10. 后续扩展边界

### 10.1 打开浏览器身份

```text
Todo -> browser_profile.launch -> profileId
进程启动成功 -> dispatch succeeded -> 按 on_started 完成 Todo
```

浏览器身份适配器提供目标列表、目标校验和启动结果。Todo 与动作中心无需增加浏览器专有字段。

### 10.2 开始开发环境

```text
Todo -> development_environment.start -> environmentConfigId
适配器启动多个服务
全部达到预期运行态 -> dispatch succeeded
任一步骤失败 -> dispatch failed
```

多服务的顺序、回滚、日志和健康判断归开发环境适配器负责。动作中心只观察一个整体 dispatch，不提前建设通用步骤编排器。

## 11. 测试策略

### 11.1 Rust 定向测试

- 动作定义和目标列表。
- 绑定新增、更新、解除和事务回滚。
- 周期 Todo 拒绝动作绑定。
- 目标失效拒绝派发。
- 活动 dispatch 唯一性与重复点击幂等。
- 合法状态转换和非法转换拒绝。
- 上线包启动前完成 `dispatchId + runId` 关联。
- 错误 `runId` 和旧终态不能完成 Todo。
- 只有 `succeeded` 完成 Todo。
- 删除 Todo 后保留 dispatch。
- 应用启动时处理中断状态。

### 11.2 前端定向测试

- 动作类型和目标配置联动。
- 未选择目标不能保存。
- 无配置空态和失效配置展示。
- 普通提醒继续显示“完成”。
- 动作提醒显示“开始打包”。
- Todo 详情手动执行。
- 主窗口 intent 路由到正确工具和配置。
- 未保存上线包配置不会被覆盖。
- 取消、失败和重复触发的 UI 状态。
- 周期事项不提供动作配置。

### 11.3 最低验证

```text
cargo test action_center
cargo test release_package
Todo、提醒、动作中心、上线包相关前端定向测试
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
git diff --check
```

## 12. 验收标准

1. 用户能为单次 Todo 选择“上线包打包”和一个已有配置。
2. 没有有效配置时无法保存绑定。
3. 到点提醒显示“开始打包”，普通提醒行为不变。
4. 点击后打开正确的上线包配置和现有确认流程。
5. 所有覆盖、SSH 和远程预检确认均未被绕过。
6. 取消确认不会启动打包或完成 Todo。
7. 上线包完整成功后自动完成 Todo。
8. 部分成功、上传失败、完全失败、取消和中断均不完成 Todo。
9. 同一绑定不能并发或重复启动。
10. 配置删除、页面未保存内容和应用中断均显式报错。
11. Todo 详情能查看最近结果并重新执行。
12. 浏览器身份和开发环境可通过新增适配器接入，无需给 Todo 增加领域专有字段。
