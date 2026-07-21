# 全局通知弹窗重构设计

## 目标

把现有任务提醒专用弹窗重构为统一的全局通知弹窗，并在上线包打包结束后展示结果通知。

通知必须在主窗口可见、隐藏或当前已经位于上线包打包页面时都弹出。打包成功、部分成功、失败需要通知，用户主动取消不通知。成功和部分成功可以直接打开上线包打包页面或最终归档目录；失败可以打开上线包打包页面查看日志；所有打包结果通知都提供“知道了”。

## 范围与非目标

本次范围：

- 将任务提醒专用窗口、启动参数和队列协议提升为通用全局通知能力。
- 保留任务提醒已有的完成、知道了、稍后提醒行为。
- 接收上线包打包的成功、部分成功和失败终态并生成通知。
- 支持从通知打开上线包打包页面、打开最终归档目录或仅关闭当前通知。
- 支持任务提醒与打包结果混合排队、FIFO 展示和唯一键去重。
- 将窗口生命周期、通知模型和类型专属动作拆分到清晰边界中。

本次不包含：

- 用户主动取消打包后的通知。
- 通知历史、数据库持久化、已读中心或跨进程恢复。
- 系统原生通知中心集成。
- 新增通知开关、声音或免打扰设置。
- 修改上线包的构建、归档和取消语义。
- 大范围调整应用视觉风格。

## 方案选择

采用“统一全局通知窗口”方案。现有独立任务提醒窗口升级为通用通知窗口，窗口只维护定位、置顶、聚焦、队列和关闭生命周期；任务提醒与打包结果分别提供内容和动作。

不新增打包专用窗口，因为它会复制窗口创建、定位、队列、样式和导航逻辑。不使用主窗口内 Toast，因为主窗口隐藏时无法可靠提示，也不能复用现有任务提醒队列。

## 通知模型

全局通知使用带判别字段的联合类型，至少包含以下公共字段：

```ts
type GlobalNotification = TodoReminderNotification | ReleasePackageNotification;

interface GlobalNotificationBase {
  id: string;
  kind: "todo-reminder" | "release-package";
  createdAt: string;
}
```

任务提醒通知继续携带现有 `TodoReminderDispatch` 所需的事件 ID、任务 ID、提醒 ID、优先级、标题、正文和触发时间。其唯一键为 `todo-reminder:<eventId>`。

打包结果通知携带：

```ts
interface ReleasePackageNotification extends GlobalNotificationBase {
  kind: "release-package";
  runId: string;
  projectId: number;
  projectName: string;
  status: "succeeded" | "partially_succeeded" | "failed";
  archivePath?: string;
  error?: string;
}
```

打包通知唯一键为 `release-package:<runId>`。项目名和归档路径来自任务启动时读取的项目快照，不能依赖当前面板选中项或任务结束后的可变配置。

## 后端职责与数据流

Rust 新增独立的全局通知模块，负责：

1. 定义可序列化的通知 payload。
2. 创建或复用唯一通知窗口。
3. 将初始通知注入新窗口，或向已存在窗口发送通知事件。
4. 统一窗口尺寸、右下角定位、置顶、聚焦和队列为空后的关闭。
5. 提供“显示主窗口并导航到指定工具”的通用命令。

任务调度器把 `ReminderDispatch` 转换为任务提醒通知后交给该模块，不再直接管理任务专用窗口。

上线包运行时仍先发送现有 `release-package://status` 事件并落定真实运行状态。只有 `phase == "overall"` 且状态为 `succeeded`、`partially_succeeded` 或 `failed` 时，才从同一项目快照生成一条打包结果通知。`running`、前端或后端子阶段状态以及 `cancelled` 不生成通知。

通知展示是打包结果的旁路行为。弹窗创建、通知事件发送或窗口聚焦失败不能改变打包终态、回滚归档或伪造失败。

## 前端职责与队列

通知窗口入口从任务提醒专用入口改为全局通知入口。Vue 组件负责：

- 合并初始化 payload 与运行期推送事件。
- 按通知唯一键去重并保持 FIFO 顺序。
- 仅渲染队首通知，顶部显示 `当前序号/总数`。
- 根据 `kind` 选择标题、图标、正文、状态样式和操作按钮。
- 操作成功后移除当前通知；队列仍有内容时继续展示，队列为空时关闭窗口。
- 操作失败时保留当前通知并显示明确错误，允许用户重试。

队列仅保存在通知窗口内存中，不写入数据库或本地设置。通知窗口被关闭后不恢复本次队列。

通知类型、唯一键生成、payload 归一化、队列合并、状态文案和可用动作判断提取为纯函数，Vue 组件只负责状态编排和 IPC 调用。

## 交互设计

通知窗口沿用现有右下角、置顶、不可缩放和紧凑尺寸，视觉保持当前浅色体系。窗口标题改为通用通知语义，内容区按当前通知类型变化。

### 任务提醒

保留现有行为：

- “完成”：完成对应任务并移除当前通知。
- “知道了”：标记提醒已读并移除当前通知。
- “稍后提醒”：设置延后时间并移除当前通知。

### 打包成功

展示项目名、成功状态、归档完成说明和最终目录。操作包括：

- “打开打包页面”
- “打开目标目录”
- “知道了”

### 打包部分成功

展示项目名、部分成功状态、精简错误摘要和最终目录。由于存在可用归档，操作与成功通知相同。

### 打包失败

展示项目名、失败状态和精简错误摘要。因为没有最终归档目录，不显示“打开目标目录”。操作包括：

- “打开打包页面”
- “知道了”

### 关闭与导航

“知道了”和右上角关闭按钮只移除当前通知，不一次清空整个队列。打开页面或目录成功后也移除当前通知。

“打开打包页面”通过通用主窗口导航入口显示并聚焦主窗口，再发送现有工具导航事件打开 `release-package` 标签页。该动作只承诺打开功能页面，不额外引入项目选中状态或运行历史持久化。

“打开目标目录”复用现有 `tool:system:open-local-path` 能力打开 `archivePath` 指向的最终目录，不新增文件系统权限或路径绕过逻辑。

## 错误与边界处理

- 未知通知类型或结构不完整的 payload 必须显式拒绝，不能渲染成伪成功通知。
- 重复的任务事件 ID 或打包 `runId` 不得重复入队。
- 打包错误摘要在弹窗中限制展示长度，完整错误和日志保留在上线包打包页面。
- 打开页面或目录失败时显示错误并保留当前通知。
- `archivePath` 为空时不显示目录操作；部分成功若缺少 `archivePath` 也按无目录能力处理，不构造路径。
- 窗口创建或事件发送失败只记录通知失败，不改变任务提醒调度或上线包运行结果。
- 任务提醒现有 IPC 命令和数据库已读、完成、稍后提醒语义保持不变。

## 文件边界

涉及以下职责边界：

- 新增 `apps/desktop/src-tauri/src/global_notification.rs`：通知 payload、窗口管理、打包终态映射和主窗口导航命令。
- 修改 `apps/desktop/src-tauri/src/main.rs`：注册全局通知模块和命令，任务调度改为发送通用通知。
- 修改 `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`：仅在符合条件的 overall 终态生成打包通知。
- 修改 `apps/desktop/src-tauri/src/events.rs` 与 `apps/desktop/src/bridge/events.ts`：将任务专用推送事件提升为通用通知推送事件。
- 修改 `apps/desktop/src-tauri/capabilities/default.json`：窗口 label 改为 `global-notification` 后仍能调用所需 IPC。
- 将 `apps/desktop/src/ReminderPopupApp.ts` 重命名为 `apps/desktop/src/GlobalNotificationApp.ts`。
- 将 `apps/desktop/src/components/ReminderPopup.vue` 重命名为 `apps/desktop/src/components/GlobalNotificationPopup.vue`，挂载并渲染通用通知窗口。
- 新增 `apps/desktop/src/types/global-notification.ts`：集中定义判别联合类型。
- 新增 `apps/desktop/src/utils/globalNotification.ts` 与 `apps/desktop/src/utils/globalNotification.test.ts`：通知归一化、队列去重、文案和动作纯函数。
- 新增 `apps/desktop/src/components/GlobalNotificationPopup.test.ts`：锁定各通知类型的操作和关闭行为。
- 修改 `apps/desktop/src/main.ts`：识别通用通知窗口视图。
- 修改 `process.md`：记录统一全局通知队列与长任务终态通知的稳定经验。

不修改数据库 schema，不新增用户设置，不改上线包页面布局。

## 验证要求

按 TDD 顺序实现并运行以下验证：

1. Rust 纯逻辑测试：仅 overall 的成功、部分成功、失败生成通知；取消、运行中和子阶段不生成；payload 包含项目快照、错误和归档路径。
2. 前端纯函数测试：初始化归一化、混合队列 FIFO、唯一键去重、三种打包结果的可用动作。
3. 通知组件测试：任务提醒行为保持，打包通知按钮差异，“知道了”和右上角关闭只移除当前项，操作失败保留当前项。
4. 上线包运行态回归：现有状态事件、日志隔离和最终归档路径不受通知旁路影响。
5. 执行 `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`。
6. 执行相关 Vitest 文件。
7. 执行 `pnpm typecheck`。
8. 执行 `pnpm --filter @lazycat/desktop build:web`。
9. 执行 `git diff --check`。

实现阶段不自动启动产品 dev server。若需要运行时视觉检查，使用仓库现有本地预览机制并覆盖任务提醒、打包成功、部分成功、失败和混合队列状态。
