# 上线包测试/生产环境设计

## 背景

当前上线包以一条 `release_package_projects` 记录同时保存项目名称、工程目录、构建命令、产物路径、本地归档配置和服务器上传配置。一个项目若同时发布测试与生产环境，只能复制成两个项目配置，导致工程目录重复、项目关系丢失，也容易在修改或动作绑定时选错目标。

本设计将上线包拆成“项目公共配置 + 固定环境配置”两层。同一项目在列表中只出现一次，内部固定提供测试、生产两个环境。测试环境默认且保持普通操作流程；生产环境在配置、确认、运行和通知阶段持续醒目标识，并要求额外确认。

## 目标

1. 每个上线包项目固定拥有 `test` 和 `production` 两个环境。
2. 项目名称和前后端工程目录只保存一次；构建与交付配置按环境独立保存。
3. 配置、预检、运行、重试、日志、通知及动作中心统一使用稳定的环境配置 ID。
4. 默认进入测试环境，生产发布必须经过不可绕过的显式确认链。
5. 现有项目和 Todo 动作绑定无歧义迁移到生产环境。
6. 保持现有本地归档、服务器上传、Vault、SSH 信任、远端事务、取消和重试语义不变。

## 非目标

- 不支持新增、删除、重命名或排序环境。
- 不增加开发、预发布等第三种环境。
- 不允许测试环境从生产环境隐式继承配置。
- 不增加测试与生产并发；继续保持全局同一时间一个上线包任务。
- 不增加环境变量编辑器、配置模板、批量复制或配置导入导出。
- 不改变本地归档与服务器上传互斥的打包类型。
- 不改变上传后命令失败、上传失败重试和远端完整替换事务的既有边界。

## 方案选择

采用“项目主表 + 环境配置表”。未采用以下方案：

- 单表增加测试/生产两套字段：字段、SQL 和校验分支接近翻倍，维护成本持续增长。
- 将测试与生产保存为两个独立项目：工程目录重复，无法稳定表达归属关系，动作绑定也更容易误选。

环境表增加自增 `id` 作为主键，同时保留 `(project_id, environment)` 唯一约束。`id` 用于 IPC、运行态、令牌和动作绑定的稳定引用；`environment` 保留业务语义和生产保护判断，不能由 ID 推断环境类型。

## 数据模型

### 项目公共配置

```sql
CREATE TABLE release_package_projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    frontend_project_path TEXT NOT NULL,
    backend_project_path TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

项目公共配置只保存：

- 项目名称；
- 前端工程目录；
- 后端工程目录；
- 创建、更新时间。

### 环境配置

```sql
CREATE TABLE release_package_environments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    environment TEXT NOT NULL
        CHECK (environment IN ('test', 'production')),
    output_root TEXT NOT NULL DEFAULT '',
    package_type TEXT NOT NULL DEFAULT 'local_archive'
        CHECK (package_type IN ('local_archive', 'server_upload')),
    frontend_build_command TEXT NOT NULL DEFAULT '',
    frontend_success_keyword TEXT NOT NULL DEFAULT '',
    frontend_post_upload_command TEXT NOT NULL DEFAULT '',
    frontend_artifact_path TEXT NOT NULL DEFAULT '',
    frontend_artifact_mode TEXT NOT NULL DEFAULT 'copy_directory'
        CHECK (frontend_artifact_mode IN ('copy_directory', 'zip_directory')),
    backend_build_command TEXT NOT NULL DEFAULT '',
    backend_success_keyword TEXT NOT NULL DEFAULT '',
    backend_post_upload_command TEXT NOT NULL DEFAULT '',
    backend_artifact_path TEXT NOT NULL DEFAULT '',
    ssh_host TEXT NOT NULL DEFAULT '',
    ssh_port INTEGER NOT NULL DEFAULT 22,
    ssh_username TEXT NOT NULL DEFAULT '',
    ssh_auth_type TEXT NOT NULL DEFAULT 'password'
        CHECK (ssh_auth_type IN ('password', 'private_key')),
    vault_entry_id INTEGER NULL,
    ssh_private_key_path TEXT NOT NULL DEFAULT '',
    frontend_remote_dir TEXT NOT NULL DEFAULT '',
    backend_remote_path TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (project_id) REFERENCES release_package_projects(id) ON DELETE CASCADE,
    UNIQUE (project_id, environment)
);
```

环境配置独立保存：

- 构建命令、成功日志关键字和产物路径；
- 打包类型、归档根目录和前端归档方式；
- SSH 认证方式、Vault 凭据引用和私钥配置；
- 前后端远程路径与上传后命令。

空字符串组成的待配置环境是合法持久化状态，但不是可运行配置。是否可运行由后端统一校验得出，不额外保存 `configured` 布尔值，避免第二事实源。

### 固定环境不变量

- 每个项目必须恰好有一条 `test` 和一条 `production` 环境记录。
- 创建项目时在同一事务内创建项目及两条环境记录。
- 删除项目依赖外键级联删除两条环境记录。
- 不提供环境创建、删除或修改类型的 IPC action。
- 更新公共配置和当前环境配置在同一事务内提交。

## 迁移设计

现有记录按以下规则迁移：

1. 保留原 `release_package_projects.id`、项目名称、前端工程目录、后端工程目录和时间字段。
2. 将原记录的所有环境专属字段复制到新建的 `production` 环境。
3. 为同一项目创建一条使用安全空值和类型默认值的 `test` 环境，不复制生产服务器、凭据、远程路径或命令。
4. 把 `action_bindings` 中 `action_type = 'release_package.run'` 的 `target_id` 从旧项目 ID 改为对应生产环境 ID。
5. 把尚处于 `pending_confirmation` 或 `running` 的同类型 `action_dispatches.target_id` 同步改为生产环境 ID；现有启动恢复逻辑仍负责将应用重启前中断的 `running` 派发收口为失败。
6. 已完成的动作派发仅是历史快照，不重写其目标 ID。
7. 验证每个旧项目都已产生一条生产环境、一条测试环境，并验证所有活动绑定都能解析后，才移除旧表中的环境专属字段。

迁移使用单个 `IMMEDIATE` SQLite 事务，通过新表复制和表替换完成。任一步失败均回滚到旧结构，不允许保留只有项目主记录或只有一个环境的半迁移状态。迁移函数必须幂等：检测到新结构和完整固定环境后不得重复插入或覆盖用户数据。

## 前端类型与 IPC 契约

### 核心类型

```ts
type ReleasePackageEnvironmentKind = "test" | "production";

interface ReleasePackageProjectBase {
  id: number;
  name: string;
  frontendProjectPath: string;
  backendProjectPath: string;
  createdAt: string;
  updatedAt: string;
}

interface ReleasePackageEnvironmentConfig extends ReleasePackageUploadConfig {
  id: number;
  projectId: number;
  environment: ReleasePackageEnvironmentKind;
  packageType: ReleasePackageType;
  outputRoot: string;
  frontendBuildCommand: string;
  frontendSuccessKeyword: string;
  frontendPostUploadCommand: string;
  frontendArtifactPath: string;
  frontendArtifactMode: ReleasePackageArtifactMode;
  backendBuildCommand: string;
  backendSuccessKeyword: string;
  backendPostUploadCommand: string;
  backendArtifactPath: string;
  configured: boolean; // 后端根据完整校验计算，只读
  createdAt: string;
  updatedAt: string;
}

interface ReleasePackageProject extends ReleasePackageProjectBase {
  environments: ReleasePackageEnvironmentConfig[];
}
```

列表返回项目聚合结构，每个项目固定包含测试、生产两个环境。若数据库不变量被破坏，后端返回明确错误，不在前端临时补造缺失环境。

### 配置保存

- `project_create` 接收项目公共草稿、当前环境类型及当前环境草稿；默认当前环境是 `test`。
- 后端在一个事务中创建项目、写入当前环境并创建另一条空白环境。
- `project_update` 接收项目 ID、环境 ID、公共草稿和当前环境草稿，在同一事务中校验并保存。
- 环境 ID 必须属于提交的项目 ID；不匹配直接拒绝。
- 正常表单保存继续要求当前环境配置完整。迁移生成的空白测试环境和创建时未编辑的另一环境由内部初始化路径写入，不经普通保存接口伪装成有效配置。
- `project_delete` 仍按项目 ID 删除整个项目及两个环境。

### 运行相关 action

以下 action 以 `environmentId` 作为环境配置唯一引用：

- `prepare`
- `target_check`
- `remote_probe`
- `host_trust`
- `remote_preflight`
- `start`
- `upload_retry`
- `command_retry_prepare`
- `command_retry_preflight`
- `command_retry_start`

取消仍按当前运行 ID 处理，但取消结果、状态事件和日志事件必须返回 `environmentId`、`projectId`、`environment`。前端不得同时提交可互相矛盾的 `projectId + environment` 来选择配置。

生产环境 `start` 必须额外携带显式 `productionConfirmed: true`。后端加载环境后发现类型为 `production` 且标记缺失或不是严格布尔值 `true` 时拒绝启动。该标记用于阻止调用链遗漏确认步骤，不作为权限或身份认证机制。

## 配置界面

### 项目与环境选择

- 左侧项目列表仍只显示一次项目，不展开为两个伪项目。
- 编辑区标题旁显示固定分段控件：`测试环境`、`生产环境`。
- 首次打开工具、刷新列表或切换项目后始终选中测试环境，不持久化生产环境选择。
- 测试环境使用普通蓝色标识；生产环境使用红色标识。
- 空白测试环境显示“待配置”，完整校验通过后显示“已配置”。
- 项目公共配置与当前环境配置在同一表单中编辑，但通过分区标题明确字段归属。

### 未保存修改

dirty 判断同时覆盖公共草稿和当前环境草稿。存在未保存修改时，切换项目、切换环境、新建项目或响应外部动作请求都必须复用现有阻止流程，不得静默覆盖草稿。

保存只提交当前展示的环境和公共配置。切换到另一环境后从已加载的项目聚合数据恢复对应环境草稿，不复用上一个环境的命令、服务器或路径。

### 启动确认

测试环境沿用现有本地归档或服务器上传确认流程。

生产环境在现有类型专属确认之后增加最终确认区，持续展示：

- 项目名称和红色“生产环境”标识；
- 本次选择的前端、后端目标；
- 打包类型；
- 本地归档最终路径，或服务器地址、端口及前后端远程路径；
- “确认生产发布”危险操作按钮。

只有用户点击该按钮才提交 `productionConfirmed: true`。关闭弹窗、返回上一步、预检失败或配置变化都清除确认状态。Todo / 动作中心触发的生产任务必须进入相同确认链，不能由外部动作直接启动。

## 运行时与状态所有权

### 配置快照

`environmentId` 是启动入口，后端一次加载环境及所属项目，并构造不可变运行配置快照。构建线程、归档、部署和上传后命令只使用该快照，不在运行中重新拼接前端草稿或读取“当前选中环境”。

保持现有全局运行槽，同一时间只允许一个上线包任务。环境模型不引入测试/生产并发、排队或后台恢复。

### 运行态键

前端项目运行态从仅按 `projectId` 索引改为按 `environmentId` 索引。运行态仍保存 `projectId` 和 `environment` 供项目归类与生产标识使用。

所有日志、状态、上传进度和命令状态事件至少包含：

```ts
interface ReleasePackageEventIdentity {
  runId: string;
  environmentId: number;
  projectId: number;
  environment: ReleasePackageEnvironmentKind;
}
```

事件接收继续先校验 `runId`，再校验 `environmentId`。测试与生产的历史面板状态不能互相覆盖，生产任务运行期间切回测试配置也不能把生产日志显示为测试日志。

### 预检与重试

- 主机探测、SSH/SFTP 预检令牌绑定环境 ID、目标、最终端点和远程路径。
- 上传失败重试令牌绑定环境 ID、运行配置中的端点与远程路径快照、选中目标和产物清单。
- 上传后命令重试令牌同样绑定环境 ID 和失败命令快照。
- 重试前重新认证，但必须核对当前环境仍存在且安全相关配置与失败任务快照一致。
- 环境、服务器端点、远程路径或目标配置发生变化时拒绝旧令牌，要求重新打包；不把旧产物部署到新环境。
- 令牌仍保持短期、一次性和仅内存语义，不新增持久化恢复。

## Todo 与动作中心集成

`release_package.run` 的动作定义保持 `open_and_confirm`，但目标含义从项目 ID 改为环境 ID：

- `ActionTargetOption.id` 返回环境 ID 字符串；
- 标签显示为“项目名 · 测试环境”或“项目名 · 生产环境”；
- 未配置完整的环境仍可返回，但 `available = false` 并给出“环境配置不完整”；
- Todo 绑定和派发的 `target_id` 保存环境 ID；
- 派发与运行关联从比较项目 ID 改为比较环境 ID；
- 运行终态仍通过 run ID 统一收口 Todo。

既有 Todo 绑定迁移到生产环境 ID。上线包动作当前不支持动作组合，因此不修改 `action_combination_steps` 或组合执行器。

生产环境动作仍必须打开上线包工具并完成人工确认。测试环境也继续遵循既有 `open_and_confirm` 行为，本设计不把它改为后台自动执行。

## 数据流

```text
选择项目
  -> 默认选择测试环境
  -> 编辑公共配置 + 当前环境配置
  -> 事务保存
  -> prepare(environmentId)
  -> 后端加载项目与环境并校验
  -> 本地目标检查或 SSH 预检
  -> 测试环境：现有确认
     生产环境：现有确认 + 生产最终确认
  -> start(environmentId, 类型专属参数, productionConfirmed?)
  -> 构建前端/后端
  -> 本地归档或服务器上传
  -> 上传后命令
  -> 携带环境身份的状态、日志与全局通知
```

运行链路中的环境身份只从后端加载的环境记录派生。前端传入的确认标记不能改变环境类型，测试环境也不能通过提交 `production` 字符串切换到另一配置。

## 错误处理

- 环境 ID 不存在、项目缺失或固定环境不变量损坏：明确失败，不回退到另一环境。
- 环境配置不完整：保存或启动时返回具体缺失字段，不使用生产配置补齐测试配置。
- 环境 ID 与项目 ID 不匹配：配置更新拒绝并保持原数据。
- 生产确认缺失：后端拒绝占用运行槽和启动构建线程。
- 预检后配置变化：旧令牌失效，重新预检。
- 重试时安全相关配置变化：旧重试令牌失效，要求重新打包。
- 切换环境时存在 dirty 草稿：阻止切换，等待用户保存、放弃或取消。
- 迁移失败：整个数据库事务回滚，应用明确报告 schema 初始化失败。
- 删除项目后旧动作绑定：外键级联只负责环境数据；动作目标解析返回不可用，不静默改绑其他项目。

## 通知与文案

面板状态、确认框、实时日志和全局通知必须使用同一运行快照中的项目名和环境类型。示例：

- `商城项目 · 测试环境：本地归档完成`
- `商城项目 · 生产环境：服务器文件已上传，后置命令失败`

生产环境使用危险色，但错误、警告、成功状态仍保留自身语义；不能把所有生产状态都渲染为错误。通知中的服务器密码、私钥口令和 Vault 秘密继续禁止输出。

## 测试设计

### Rust schema 与配置

- 全新数据库创建项目表和环境表，固定环境唯一约束有效。
- 旧项目保留 ID 和公共字段，旧配置完整迁移到生产环境。
- 测试环境为空且不复制服务器、Vault 引用、远程路径或命令。
- 每个项目迁移后恰好两条环境；重复执行迁移不重复插入、不覆盖配置。
- 项目删除级联删除环境。
- 创建项目和更新公共配置/当前环境保持事务原子性。
- 环境 ID 不存在或不属于指定项目时拒绝更新。
- 待配置环境可读取但不能运行。

### Rust 运行与远程链路

- 所有类型专属 action 按环境 ID 加载正确配置。
- 测试和生产相同项目使用各自命令、产物和交付目标。
- 生产启动缺少严格 `true` 确认标记时在线程启动前失败并释放运行槽。
- 日志、状态、通知包含正确的环境身份。
- 预检令牌不能跨环境使用。
- 上传失败和命令失败重试令牌不能跨环境使用。
- 配置变化后旧令牌失效；配置不变时保留现有重试行为。
- 本地归档、服务器上传、取消、远端回滚和后置命令既有测试继续通过。

### 动作中心与 Todo

- 目标列表按环境 ID 返回“项目名 · 环境”标签。
- 空白测试环境不可绑定，完整生产环境可绑定。
- 既有绑定和未完成派发迁移到生产环境 ID。
- 派发只能与相同环境 ID 的 run 关联。
- Todo 触发生产环境时打开正确环境并要求生产确认。
- 环境或项目删除后绑定明确显示不可用。

### TypeScript 与 Vue

- 聚合项目与两个固定环境的类型转换和草稿恢复。
- 默认选择测试环境，不记忆生产选择。
- 环境切换、项目切换及外部动作处理的 dirty 拦截。
- 测试/生产字段相互隔离，测试空配置显示“待配置”。
- 运行态按环境 ID 隔离并拒绝错误 run 的事件。
- 生产环境红色标识、摘要和“确认生产发布”按钮。
- 取消、返回、预检失败和配置变化清除生产确认。
- Todo 动作打开绑定的环境而不是界面默认环境。

### 最低验证

```text
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/utils/releasePackage.test.ts src/composables/useReleasePackageRuntime.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
git diff --check
```

真实 SSH 环境可用时补测试、生产各一次最小上传冒烟，并使用不同的非生产远端目录验证环境隔离；不可用时明确记录未执行，不用模拟测试冒充真实上传验证。

## 预计影响范围

主要涉及：

- `apps/desktop/src/types/release-package.ts`
- `apps/desktop/src/utils/releasePackage.ts` 及测试
- `apps/desktop/src/components/ReleasePackagePanel.vue` 及测试
- `apps/desktop/src/composables/useReleasePackageRuntime.ts` 及测试
- `apps/desktop/src/composables/useReleasePackageUploadPreflight.ts` 及测试
- `apps/desktop/src/composables/useReleasePackageCommandRetry.ts` 及测试
- `apps/desktop/src-tauri/src/tools/release_package.rs`
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`
- `apps/desktop/src-tauri/src/tools/action_center/bindings.rs`
- `apps/desktop/src-tauri/src/tools/action_center/dispatches.rs`
- 全局通知和 Todo 动作绑定的相关类型与测试
- `docs/experience/release-package.md`

不新增外部依赖，不修改 SSH/SFTP 底层协议，不改变其他工具的动作目标 ID 语义。
