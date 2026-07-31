# 上线包成功检测与上传后命令设计

## 背景

现有上线包以构建命令退出码为第一层成功依据，并在命令结束后校验前端目录或后端文件是否存在。服务器上传已经具备 SSH 主机信任、一次性认证预检、前后端目标级并行上传、串行提交、失败回滚和上传重试能力。

本次在现有流水线内增加两类项目级可选配置：

1. 前端、后端分别配置成功日志关键字，使构建成功必须同时满足命令退出、日志信号和产物校验。
2. 前端、后端分别配置上传后命令，在本次所有远端目标提交成功后执行，并在失败时支持只重试失败命令。

## 目标

- 前端、后端可分别配置一个成功日志关键字。
- 关键字非空时，同时匹配对应构建命令的 stdout 和 stderr，使用区分大小写的包含匹配。
- 目标构建成功必须同时满足：退出码为 `0`、已启用的成功关键字命中、产物类型和路径有效。
- 前端、后端可分别配置一段上传后命令，仅对本次选中并成功上传的目标生效。
- 所有选中目标完成远端提交后，按前端、后端顺序执行各自命令；一条失败不阻止后续命令。
- 后置命令失败不回滚已经提交的远端文件，并使用独立终态表达“上传成功、命令失败”。
- 后置命令失败后允许只重试失败命令，不重新构建或上传。

## 非目标

- 不增加任意发布步骤编排器、命令模板变量、条件表达式或可视化流程编辑器。
- 不支持正则表达式、多关键字组合、忽略大小写或按日志流分别配置关键字。
- 不向远程命令自动注入 `sudo`、环境变量、工作目录或前后端路径。
- 不提供长期 SSH 会话池、跨任务连接保活或跨应用重启恢复。
- 不持久化命令重试任务、认证秘密、预检令牌或命令运行日志。
- 不让后置命令失败触发远端文件回滚，因为命令可能已经产生不可逆副作用。

## 已确认行为

### 空值即禁用

不增加额外开关，配置内容是唯一事实源：

- `frontendSuccessKeyword` 为空时，前端不启用日志成功检测。
- `backendSuccessKeyword` 为空时，后端不启用日志成功检测。
- `frontendPostUploadCommand` 为空时，前端上传完成后不执行命令。
- `backendPostUploadCommand` 为空时，后端上传完成后不执行命令。

保存时去除字段首尾空白。关键字内部字符保持原样；多行远程命令仅去除整体首尾空白，内部换行和缩进保持原样。

### 构建成功条件

某个目标的构建成功顺序为：

1. 构建命令退出码为 `0`。
2. 若成功关键字非空，stdout 或 stderr 的任意完整日志行包含该关键字。
3. 前端产物是目录，或后端产物是常规文件。

非零退出码不能被关键字命中覆盖。关键字未命中时不继续产物校验，目标直接失败并给出明确错误。未选择的目标不执行命令、不检测关键字、不校验产物。

### 上传后命令时机

服务器上传仍按目标级并行传输并串行提交。只有本次所有选中目标都完成远端提交后，才进入远程命令阶段：

```text
前端/后端并行构建
        │
        ▼
全部选中目标构建成功
        │
        ▼
目标级并行上传、校验和串行提交
        │
        ▼
全部选中目标已提交
        │
        ├─ 前端上传后命令
        │
        └─ 后端上传后命令
              │
              ▼
           聚合终态
```

命令固定按前端、后端顺序执行。前端命令失败不会阻止后端命令执行。任一构建失败或上传事务失败时，不执行任何上传后命令。

## 数据模型

在 `release_package_projects` 增加四个非空文本列：

```sql
frontend_success_keyword TEXT NOT NULL DEFAULT '',
backend_success_keyword TEXT NOT NULL DEFAULT '',
frontend_post_upload_command TEXT NOT NULL DEFAULT '',
backend_post_upload_command TEXT NOT NULL DEFAULT ''
```

使用现有幂等列迁移方式增加字段。旧项目读取为空字符串，行为与升级前一致。

前端类型增加对应 camelCase 字段：

```ts
interface ReleasePackageProjectDraft {
  frontendSuccessKeyword: string;
  backendSuccessKeyword: string;
  frontendPostUploadCommand: string;
  backendPostUploadCommand: string;
}
```

项目创建、更新和查询沿用现有 IPC，只扩展负载和返回字段。

## 前端交互

### 项目配置

- 前端“构建命令”下增加“成功日志关键字”单行输入框。
- 后端“构建命令”下增加“成功日志关键字”单行输入框。
- 提示文案说明：同时匹配 stdout/stderr、使用包含匹配、区分大小写、留空不检测。
- 服务器上传配置中的前端远程目标旁增加“前端上传后命令”多行输入框。
- 服务器上传配置中的后端远程目标旁增加“后端上传后命令”多行输入框。
- 后置命令提示说明：全部目标上传成功后执行，留空跳过，不自动注入 `sudo` 或路径变量。

本地归档类型可以保存成功关键字并参与构建检测，但不显示或执行上传后命令。后置命令字段继续随项目保存，切换回服务器上传时恢复原配置。

### 运行反馈

现有前端、后端目标状态继续表示构建结果。上传后命令增加目标级状态：

```text
skipped | pending | running | succeeded | failed | cancelled
```

界面在服务器上传运行区域分别显示前端、后端命令状态。远程命令 stdout/stderr 继续进入现有“上传日志”，系统日志和输出行使用稳定前缀区分：

```text
[前端命令] 开始执行上传后命令
[前端命令][stdout] ...
[前端命令][stderr] ...
[后端命令] ...
```

不新增第四个日志面板。命令失败终态显示“仅重试失败命令”，与现有“重试上传”入口互斥。

## 构建日志关键字检测

现有 PowerShell 命令执行器在读取 stdout/stderr 时逐行调用日志回调。扩展命令执行结果，记录目标关键字是否被任一完整日志行包含：

```rust
struct CommandOutcome {
    success_keyword_matched: bool,
}
```

关键字为空时不执行匹配，并将该条件视为满足。匹配只基于已拆分的完整日志行，避免原始读取块边界导致误判；同一行在发送日志事件前或后匹配均可，但必须使用完全相同的文本值。

错误优先级保持明确：

1. 用户取消。
2. PowerShell 启动、读取或等待失败。
3. 命令非零退出码。
4. 已配置关键字但未命中。
5. 产物不存在或类型错误。

关键字未命中的错误包含目标和关键字，例如：

```text
前端构建命令退出成功，但日志未匹配成功关键字：Build completed
```

## SSH 连接与远程命令

### 首次执行复用当前认证连接

当前 `SftpRemoteFs` 只暴露 SFTP 文件操作，部署结束后所有连接在上传函数内部销毁。实现时将远程连接所有权调整为一个同时持有已认证 `ssh2::Session` 和 `Sftp` 的小型连接对象：

- 文件部署仍通过 `RemoteFs` 边界操作 SFTP。
- 并行上传和串行提交完成后，部署层在成功结果中交还一条仍有效的已认证控制连接。
- 运行时使用该连接依次创建 SSH command channel，执行前端、后端命令。
- 命令阶段结束、失败或取消后立即释放连接并清空 socket 注册。
- 上传失败、回滚或取消时不返回连接，沿用现有清理和恢复语义。

这里复用的是同一条仍存活的 SSH/TCP 连接，不保存可跨连接复用的登录会话，也不扩大认证秘密生命周期。

### 命令执行语义

每个非空配置作为一段命令原样交给服务器 SSH 默认 Shell 执行：

- 支持多行命令。
- 不拆分命令，不改写 shell 语法。
- 不注入工作目录、环境变量、目标路径或 `sudo`。
- SSH channel 的退出码为 `0` 才成功。
- stdout/stderr 逐行发送到上传日志。
- 输出按有损 UTF-8 转换，保留替换字符；底层读取错误仍显式返回，不能静默截断后伪报成功。
- channel 创建失败、命令发送失败、输出读取失败、等待关闭失败、无法读取退出码或非零退出码均为该目标命令失败。

用户取消时关闭当前运行注册的 SSH socket，使阻塞读取尽快返回。正在执行的命令标记为 `cancelled`，尚未开始的后续命令也标记为 `cancelled`。远端命令是否已经产生部分副作用无法回滚，终态必须明确文件已经上传。

## 状态模型

新增整体终态：

```text
upload_succeeded_command_failed
```

整体状态语义：

| 状态                              | 语义                                                                     |
| --------------------------------- | ------------------------------------------------------------------------ |
| `succeeded`                       | 构建、文件上传和全部已配置后置命令成功，或本次没有后置命令               |
| `package_succeeded_upload_failed` | 构建成功，但文件上传事务未完成                                           |
| `upload_succeeded_command_failed` | 文件已经全部提交，但至少一条后置命令失败                                 |
| `cancelled`                       | 在交付提交前取消，或文件已提交后取消命令阶段；详情必须说明文件是否已上传 |
| `failed`                          | 没有可交付产物的构建或运行失败                                           |

后置命令失败时不得生成上传重试令牌，只生成命令重试令牌。全局通知、动作中心和 Todo 联动只有 `succeeded` 视为完整成功；`upload_succeeded_command_failed` 映射为失败，避免自动完成上游任务。

## 仅重试失败命令

### 重试快照

首次命令阶段结束后，若存在失败目标，Rust 内存中生成一次性命令重试任务：

```rust
struct CommandRetryJob {
    project_id: i64,
    endpoint_and_auth_binding: CommandAuthBinding,
    failed_commands: Vec<CommandSnapshot>,
}

struct CommandSnapshot {
    target: ReleaseTarget,
    command: String,
}
```

只保存失败命令的目标和命令快照。已成功、已跳过或未配置的命令不进入重试。项目后续编辑不会改变本次重试内容。

重试令牌只存在于当前应用进程内，只能消费一次。重试后仍有失败时生成新的令牌；全部成功后不再生成。应用重启后令牌失效。

### 重试认证

终态后的手动重试是新任务，原 SSH 连接已经释放，因此必须重新连接并认证，但不重新构建、不读取产物、不上传文件，也不执行 SFTP 目标或覆盖预检。

- Vault 密码认证从已解锁 Vault 重新读取密码，通常不需要用户再次输入。
- 无口令私钥自动认证。
- 有口令私钥由确认弹窗重新收集口令，不持久化。
- 主机探测和指纹信任仍沿用现有安全链路；指纹变化必须阻止认证。

新增命令重试专用动作，名称在实现中保持前后端契约一致：

```text
tool:release-package:command-retry-prepare
  -> release_package / command_retry_prepare

tool:release-package:command-retry-preflight
  -> release_package / command_retry_preflight

tool:release-package:command-retry-start
  -> release_package / command_retry_start
```

职责如下：

- `command_retry_prepare`：校验命令重试令牌和项目绑定，返回失败目标、服务器非敏感摘要和认证方式，不返回命令正文或秘密。
- `command_retry_preflight`：结合已信任的主机探测结果完成 SSH 认证和 command channel 可用性校验，签发短期一次性认证令牌；不执行用户命令。
- `command_retry_start`：原子消费命令重试令牌和认证令牌，关联运行槽后启动独立命令任务。

命令认证令牌绑定项目 ID、服务器、端口、用户名、认证方式、可信指纹和重试任务，短期有效且只能消费一次。准备、认证或启动失败时不能误消费另一类令牌；启动成功后立即从待重试存储移除旧令牌。

## 运行与终态聚合

服务器上传的运行阶段扩展为：

```text
building -> uploading -> running post-upload commands -> terminal
```

整体运行状态仍可复用 `running`，通过 `phase = upload` 和目标级命令状态区分上传与命令阶段，不新增仅用于展示的整体中间状态。

终态聚合规则：

1. 构建未全部成功：保持现有构建终态，不上传、不执行命令。
2. 上传未提交：`package_succeeded_upload_failed` 或 `cancelled`，不执行命令。
3. 上传已提交且所有命令成功或跳过：`succeeded`。
4. 上传已提交且任一命令失败：`upload_succeeded_command_failed`，生成仅包含失败目标的命令重试令牌。
5. 上传已提交后用户取消命令阶段：`cancelled`，错误详情明确“服务器文件已上传，上传后命令未全部完成”；取消不会生成命令重试令牌，避免把用户主动取消等同于命令执行失败。

命令重试运行不改变已经完成的上传事实。全部重试成功时返回 `succeeded`；仍有失败时返回 `upload_succeeded_command_failed` 并签发新令牌；用户取消时返回 `cancelled` 且不签发新令牌。

## 错误与安全

- 构建日志匹配不记录额外敏感内容，只复用已经展示的构建日志。
- Vault 密码、私钥口令、认证令牌和重试令牌不得进入日志、通知、数据库或错误详情。
- 远程命令正文保存在项目配置和内存重试快照中，但默认不在全局通知中完整展示，避免命令中可能存在的业务参数泄漏。
- 后置命令失败提示必须说明远端文件已提交，不得使用“上传失败”文案。
- 非零退出码错误包含目标和退出码；stderr 已在上传日志中展示，不把全部输出复制到终态错误。
- 关闭确认弹窗、取消、启动成功和异常路径都清理一次性私钥口令与认证令牌。
- 运行槽、socket 注册、命令重试任务和动作中心关联都必须在启动线程前建立或原子消费，避免快速失败留下永久运行态。

## 测试设计

### Rust 定向测试

构建关键字：

- 空关键字不改变现有成功行为。
- stdout 命中成功。
- stderr 命中成功。
- 区分大小写，大小写不同不命中。
- 退出码非零即使命中也失败。
- 关键字未命中时不通过目标构建。
- 关键字命中但产物缺失或类型错误仍失败。
- 未选择目标不执行检测。

SSH 命令与运行时：

- 全部目标提交后才开始命令。
- 上传失败或取消时不执行任何命令。
- 前端命令先于后端命令。
- 前端命令失败不阻止后端命令。
- stdout/stderr 均进入带目标标识的上传日志。
- 非零退出码、channel 错误、读取错误和连接中断正确归类。
- 有损 UTF-8 输出可见且不会伪造读取成功。
- 用户取消关闭连接并标记未完成命令。
- 命令失败使用 `upload_succeeded_command_failed`，不生成上传重试令牌。
- 命令重试只执行明确失败的目标，不执行已成功、取消或尚未执行的目标。
- 二次失败生成新的一次性令牌，旧令牌失效。
- 命令重试认证不执行 SFTP 写入和远端覆盖检查。
- 项目配置变更不影响已签发重试快照。
- 应用退出清理命令重试任务和认证秘密。

### 前端定向测试

- 空项目、项目回填、保存和 dirty 判断覆盖四个新字段。
- 成功关键字和上传后命令的首尾空白规范化正确。
- 本地归档显示成功关键字但隐藏上传后命令。
- 服务器上传显示前后端独立命令输入。
- 目标命令状态、日志前缀和错误文案正确。
- `upload_succeeded_command_failed` 显示“仅重试失败命令”，不显示“重试上传”。
- 重试准备、主机信任、认证和启动 IPC 参数正确。
- Vault 锁定、凭据失效和私钥口令清理行为正确。

### 最低验证命令

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
pnpm --filter @lazycat/desktop test -- src/components/ReleasePackagePanel.test.ts src/utils/releasePackage.test.ts src/composables/useReleasePackageRuntime.test.ts src/composables/useReleasePackageUploadPreflight.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
git diff --check
```

## 影响范围

预计涉及：

- `apps/desktop/src/types/release-package.ts`
- `apps/desktop/src/utils/releasePackage.ts`
- `apps/desktop/src/components/ReleasePackagePanel.vue`
- `apps/desktop/src/composables/useReleasePackageRuntime.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src-tauri/src/tools/release_package.rs`
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- `apps/desktop/src-tauri/src/tools/release_package_remote.rs`
- `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`
- `apps/desktop/src-tauri/src/global_notification.rs`
- `apps/desktop/src-tauri/src/tools/action_center/dispatches.rs`
- 对应前端与 Rust 测试

不改变本地归档事务、远端完整替换规则、目标级上传并发上限、Vault 秘密存储边界和主机指纹信任模型。
