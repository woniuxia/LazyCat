# 上线包打包工具设计

## 目标

在 LazyCat 中新增“上线包打包”工具，允许用户维护多个前后端项目配置，按项目分别执行 PowerShell 构建命令，并在构建成功后把前后端产物归档到全局目录下的新文件夹中。

每个归档目录默认命名为：

```text
<最近周四 yyyyMMdd>-<项目名>
```

周四执行时取当天；其他日期取未来最近的周四。例如 2026-07-18 执行时默认目录名为 `20260723-项目名`。用户确认前可以直接修改目录名；同名目录存在时直接报错，不覆盖已有内容。

## 范围与非目标

本次范围：

- 项目配置的数据库 CRUD。
- 全局归档根目录配置。
- 前端和后端串行构建。
- 前端产物“保留文件夹”或“压缩为 ZIP”两种模式。
- 后端文件或文件夹产物复制。
- 实时日志、终止运行和明确失败状态。
- 归档临时目录和最终目录的事务式收尾。

本次不包含：

- Git 提交、推送、Tag 或 GitHub Release。
- 构建命令模板、框架自动识别或自动修复。
- 打包历史记录和持久化执行日志。
- 覆盖或清理工程目录中的旧构建产物。
- ZIP 密码或其他产物加密能力。
- 并行构建多个项目。

## 方案选择

采用 Rust 后端统一编排方案。Vue 只负责配置、确认、状态和日志展示；Rust 负责命令执行、进程终止、路径校验、复制、压缩和最终重命名。

不采用前端 Shell 插件直接执行，因为命令权限、子进程终止和文件操作会分散在组件中。不采用独立 PowerShell 发布脚本，因为实时事件、取消控制和 Tauri 状态同步会形成第二套执行入口。

## 页面结构

工具 ID 为 `release-package`，页面使用已确认的“左侧项目列表 + 右侧配置/执行工作台”布局。

顶部区域显示全局归档根目录和“选择目录”按钮。左侧提供新建、选择、编辑和删除项目。右侧工作区包含：

1. 基本信息：项目名。
2. 前端配置：工程路径、PowerShell 构建命令、产物路径、产物处理模式。
3. 后端配置：工程路径、PowerShell 构建命令、产物路径。
4. 底部执行日志：按 `[前端]`、`[后端]`、`[归档]` 区分阶段。

开始打包前显示确认框，包含项目名、默认归档目录名、完整目标路径和前端处理模式。目录名输入框可编辑。执行期间只允许查看日志和终止任务，项目配置控件和开始按钮锁定。成功后显示最终路径并提供打开归档目录操作；失败或终止后保留日志并允许重新开始。

## 持久化模型

### 项目表

在 `apps/desktop/src-tauri/src/tools/helpers.rs` 的 schema 初始化中新增：

```sql
CREATE TABLE IF NOT EXISTS release_package_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    frontend_project_path TEXT NOT NULL,
    frontend_build_command TEXT NOT NULL,
    frontend_artifact_path TEXT NOT NULL,
    frontend_artifact_mode TEXT NOT NULL
        CHECK (frontend_artifact_mode IN ('copy_directory', 'zip_directory')),
    backend_project_path TEXT NOT NULL,
    backend_build_command TEXT NOT NULL,
    backend_artifact_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

`release_package_projects` 只维护项目配置，不保存归档目录、构建日志或最近运行状态。项目名不自动从工程目录推导，必须由用户填写；项目名和运行时可编辑的目录名都必须符合 Windows 单级目录名规则。

### 全局设置

全局归档根目录保存到现有 `user_settings`：

```text
key = release_package.output_root
value = <用户选择的目录>
```

该设置不属于任何项目。删除项目不会删除工程目录、构建产物或已有归档目录。

## 工具接口

在 `apps/desktop/src/bridge/tauri.ts` 增加以下映射：

```text
tool:release-package:project-list    -> release_package / project_list
tool:release-package:project-create  -> release_package / project_create
tool:release-package:project-update   -> release_package / project_update
tool:release-package:project-delete   -> release_package / project_delete
tool:release-package:prepare          -> release_package / prepare
tool:release-package:start            -> release_package / start
tool:release-package:cancel           -> release_package / cancel
```

项目 CRUD 使用明确的 camelCase JSON 字段映射到 Rust 的 snake_case 数据库列。`prepare` 接收 `{ projectId }`，返回当前全局归档根目录、按本地日期计算的默认目录名、默认完整路径和前端产物模式。`start` 只接收 `{ projectId, folderName }`；Rust 从数据库重新读取项目和全局设置，避免使用过期或被前端篡改的配置。`cancel` 接收 `{ runId }`。

Rust 模块在 `tools/mod.rs` 注册 `release_package`，并通过 `execute_tool_with_app` 获取 `tauri::AppHandle`，用于后台线程发送事件。

## 执行流程

1. 前端保存项目配置和全局归档根目录。
2. 用户点击开始，前端调用 `prepare`，展示默认目标目录。
3. 用户确认或修改单级目录名。
4. Rust 重新读取数据库配置，校验归档根目录、工程目录、命令和目录名。
5. 检查最终目标目录是否已存在；存在则立即失败，不启动构建。
6. 创建唯一 `runId`，登记当前唯一运行任务，并在后台线程中执行。
7. 在前端工程目录执行：

   ```text
   powershell.exe -NoProfile -NonInteractive -Command <frontend_build_command>
   ```

8. 前端退出码为 0 后，在后端工程目录执行相同形式的 PowerShell 命令。
9. 后端退出码为 0 后检查产物：前端必须是目录，后端可以是文件或目录。
10. 在全局归档根目录下创建本次运行专用临时目录，目录名包含 `runId`，保证与最终目录同卷。
11. 前端为 `copy_directory` 时，将产物目录本身复制到临时目录根部；为 `zip_directory` 时生成同名 ZIP，ZIP 内保留产物目录这一层。
12. 后端文件或目录按源名称复制到临时目录根部。
13. 复制前检查前后端顶层名称冲突；冲突直接失败，禁止覆盖。
14. 临时目录全部完成后，再次确认最终目录不存在，并将临时目录重命名为最终目录。
15. 成功发送最终归档路径；失败或终止时删除本次临时目录，不创建最终目录。

构建命令的失败由非零退出码判定。构建命令自身负责清理旧产物，LazyCat 不删除或修改工程目录内容。

## 进程与事件

`start` 只负责登记任务并启动后台线程，立即返回 `runId`，不阻塞 IPC。当前只允许一个运行任务；运行期间再次开始返回明确错误。

后台线程分别读取 stdout 和 stderr，按行发送：

```ts
type ReleasePackageLogEvent = {
  runId: string;
  projectId: number;
  phase: "frontend" | "backend" | "archive";
  stream: "stdout" | "stderr" | "system";
  line: string;
};
```

发送状态事件：

```ts
type ReleasePackageStatusEvent = {
  runId: string;
  status: "running" | "succeeded" | "failed" | "cancelled";
  phase: "frontend" | "backend" | "archive";
  archivePath?: string;
  error?: string;
};
```

事件必须携带 `runId`。前端只接受当前运行的事件，旧任务的迟到事件不能覆盖当前日志或状态。

终止通过共享取消标记和 Windows `taskkill /PID <pid> /T /F` 结束当前 PowerShell 及其子进程树。终止后不执行后续阶段，不复制产物，并发送 `cancelled` 状态。应用退出时清理仍登记的运行任务。

日志解码先尝试 UTF-8，失败后使用现有 `encoding_rs` 的 GBK 解码，兼容 Windows 中文工具输出。

## 校验与错误处理

- 保存项目时要求项目名、工程路径、构建命令和产物路径非空；前端处理模式只能是 `copy_directory` 或 `zip_directory`。
- 运行前要求全局归档根目录、前端工程目录和后端工程目录存在且为目录。
- 产物路径允许运行前不存在，因为它由构建命令生成；每个构建成功后再检查产物类型。
- 目录名禁止路径分隔符、`.`、`..`、Windows 非法字符、保留设备名以及尾部空格或句点；校验失败显式提示，不静默替换。
- 最终目录已存在、前后端顶层名称冲突、磁盘 IO 失败、ZIP 失败或重命名失败都属于归档失败。
- 所有失败返回明确阶段和错误信息，保留已收集日志，不伪造成功或静默降级。
- 临时目录只清理本次运行创建且仍由本次任务持有的路径，绝不清理用户已有目录。

## 文件落点

- 创建：`apps/desktop/src/components/ReleasePackagePanel.vue`
- 创建：`apps/desktop/src/types/release-package.ts`
- 创建：`apps/desktop/src/utils/releasePackage.ts` 及对应 Vitest 测试
- 创建：`apps/desktop/src-tauri/src/tools/release_package.rs`
- 修改：`apps/desktop/src/composables/toolCatalog.ts`
- 修改：`apps/desktop/src/tool-registry.ts`
- 修改：`apps/desktop/src/bridge/tauri.ts`
- 修改：`apps/desktop/src-tauri/src/tools/mod.rs`
- 修改：`apps/desktop/src-tauri/src/tools/helpers.rs`
- 修改：`apps/desktop/src-tauri/src/main.rs` 或事件注册所需的 Tauri 入口文件
- 测试：新增 Rust 模块测试、前端纯函数测试和组件状态测试

## 验证要求

实现后按影响面执行：

1. Rust：`cargo test release_package -- --nocapture`，覆盖 CRUD、日期、目录校验、复制、ZIP、失败清理和命令退出码。
2. 前端：运行上线包相关 Vitest 文件，覆盖表单、确认目录名、事件 runId 过滤和状态转换。
3. 工作区：`pnpm typecheck`。
4. 渲染层：`pnpm --filter @lazycat/desktop build:web`。
5. 最小冒烟：使用临时前端/后端目录执行成功、非零失败和终止三条路径，确认成功目录内容和失败清理结果。

不在实现阶段自动启动正式产品 dev server；可视化伴侣仅用于本设计阶段的布局确认。
