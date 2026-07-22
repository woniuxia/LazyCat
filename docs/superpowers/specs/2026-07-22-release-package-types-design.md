# 上线包打包类型重构设计

## 背景

当前上线包工具把“仅打包”和“打包并上传”表达为同一次运行的两种启动模式。上传链路仍强制要求项目配置归档根目录，先创建并提交本地归档，再从工程产物上传服务器。这使服务器上传无端依赖本地归档配置，也让运行时、终态和重试逻辑都围绕 `archivePath` 耦合。

本设计将本地归档与服务器上传重构为互斥的两种项目打包类型。它取代 [上线包 Linux 上传设计](./2026-07-22-release-package-linux-upload-design.md) 中“构建和本地归档成功后再上传”的流程；SSH 信任、预检、远端完整替换和取消安全等既有设计继续有效。

## 目标

1. 提供 `local_archive`（本地归档）和 `server_upload`（上传服务器）两种明确的打包类型。
2. 上传服务器不需要归档根目录，不校验、不创建本地归档目录。
3. 两种类型复用项目构建、目标选择、并行执行、日志、取消和产物校验逻辑。
4. 上传服务器直接使用本次构建生成的产物目录或文件，不创建本地临时副本。
5. 保留 SSH/SFTP 预检、主机信任、远端事务替换和失败后的单独重试上传。
6. 将现有项目无歧义地迁移到新打包类型。

## 非目标

- 不增加“本地归档后再上传”的第三种类型。
- 不改变构建命令的 PowerShell 执行语义。
- 不改变前端本地归档的 `copy_directory` / `zip_directory` 处理方式。
- 不增加内容哈希、增量上传、远程命令、发布脚本或版本回滚。
- 不创建上传产物的隐藏持久副本或系统临时副本。
- 不修改服务器端 temp -> backup -> commit 的完整替换事务。

## 已确认行为

- 两种打包类型互斥，项目保存一个确定类型，单次启动时不再切换成另一种类型。
- 现有 `upload_enabled = 1` 项目迁移为 `server_upload`；其他项目迁移为 `local_archive`。
- `server_upload` 完全跳过本地归档；`output_root` 可以为空且不参与上传校验。
- 前端上传复制产物目录内的内容，不额外复制产物目录本身。

例如：

```text
前端产物目录：D:\project\dist
服务器目录：  /srv/portal/web

上传结果：
/srv/portal/web/index.html
/srv/portal/web/assets/...

不会生成：
/srv/portal/web/dist/...
```

- 前端配置为 `zip_directory` 只影响本地归档。服务器上传仍读取原始前端产物目录并递归上传其内容。
- 后端继续把选中的产物文件上传为配置的远程文件路径。
- 上传失败后的重试读取原构建产物；若文件列表、类型或大小与失败任务保存的清单不一致，明确拒绝重试并要求重新打包。

## 数据模型与迁移

在 `release_package_projects` 增加：

```sql
package_type TEXT NOT NULL DEFAULT 'local_archive'
  CHECK (package_type IN ('local_archive', 'server_upload'))
```

幂等迁移加入字段后，将尚未迁移的现有记录按以下规则写入：

```text
upload_enabled = 1 -> server_upload
upload_enabled = 0 -> local_archive
```

`package_type` 成为唯一行为真值。旧 `upload_enabled` 列因 SQLite 删除列兼容性和迁移风险暂不物理删除，但业务读写、前端类型和启动协议不再使用它，避免双重真值。

`output_root` 保留现有非空数据库列以避免重建表；`server_upload` 允许保存空字符串，`local_archive` 必须保存有效目录。服务器、认证方式和远程目标字段继续复用。

项目配置解析按类型校验：

- `local_archive`：要求归档根目录；不要求服务器字段完整。
- `server_upload`：要求服务器、认证方式和远程目标；不要求归档根目录。
- 两种类型都要求项目目录、构建命令和产物路径。

## 前端交互

项目基础配置增加打包类型分段控件：

```text
[ 本地归档 ] [ 上传服务器 ]
```

选择 `local_archive` 时显示归档根目录和本地归档处理配置；选择 `server_upload` 时隐藏归档根目录，展开服务器配置。前端产物处理方式仍可保留在工程配置中，但上传页面明确说明它只影响本地归档，避免用户误以为服务器会收到 ZIP。

启动确认框不再提供“仅打包 / 打包并上传”切换：

- 本地归档：显示归档目录名、最终归档路径、目标选择和本地覆盖确认。
- 上传服务器：显示服务器、远程目标、凭据输入、目标选择、预检结果和远端覆盖确认；不显示归档目录名、归档路径或本地覆盖步骤。
- 上传重试只进入上传确认流程，不重新执行构建命令。

运行日志继续使用前端、后端、上传三个 lane。`local_archive` 的上传 lane 保持空态；`server_upload` 不显示打开归档目录入口，只有真实 `archivePath` 的本地归档成功任务显示该入口。

## IPC 契约

项目创建、更新和列表使用：

```ts
type ReleasePackageType = "local_archive" | "server_upload";

interface ReleasePackageProjectConfig {
  packageType: ReleasePackageType;
  outputRoot: string;
  // 其他现有构建和服务器字段
}
```

`prepare` 根据项目类型返回判别联合：

```ts
type ReleasePackagePrepareResult =
  | {
      packageType: "local_archive";
      defaultFolderName: string;
      outputRoot: string;
      archivePath: string;
    }
  | {
      packageType: "server_upload";
    };
```

`start` 不再接收可与项目配置冲突的 `mode`。后端重新读取项目并按 `packageType` 解析类型专属参数：

- `local_archive` 要求 `folderName`，允许 `overwriteExisting`，禁止预检令牌和远端覆盖参数。
- `server_upload` 要求一次性 `preflightToken`，允许 `overwriteRemoteTargets`，忽略之外应拒绝归档目录名和本地覆盖参数。

`target-check` 仅允许 `local_archive`。`remote-probe`、`host-trust`、`remote-preflight` 和 `upload-retry` 仅允许 `server_upload`，避免配置类型与动作不一致。

## 运行时设计

运行时拆成“共用构建”和“类型专属交付”两层，不复制构建编排：

```text
读取项目与类型专属校验
            |
            v
前端 / 后端并行执行构建命令
            |
            v
解析并验证本次选中的产物
            |
        +---+-------------------+
        |                       |
        v                       v
 local_archive             server_upload
复制/压缩到归档 stage       生成部署产物清单
提交最终归档目录             上传远端临时目标
返回 archivePath             校验并提交远端目标
```

共用构建结果用目标描述符表达，至少包含目标类型、源路径和前端产物处理方式。构建函数只负责命令执行和产物存在性/类型验证，不感知归档目录、SSH 或远端路径。

本地归档分支继续使用 `ArchiveSession` 的 stage、commit、backup 和取消清理机制。只有该分支调用 `archive_frontend_artifact` / `archive_backend_artifact`，并产生 `archivePath`。

服务器上传分支从构建结果的源路径生成 `ArtifactManifest`：前端以产物目录为清单根，后端以产物文件为清单源。部署模块继续把清单相对路径写到远端目标目录下，因此不会额外增加源目录名。上传分支不创建 `ArchiveSession`，也不调用本地归档函数。

## 上传一致性与重试

直接读取生成物目录存在构建后被外部修改的风险。沿用并强化现有显式校验，不用静默快照掩盖问题：

1. 构建完成后立即生成 `ArtifactManifest`，记录源路径、相对文件路径、文件数、每个文件大小和总字节数。
2. 每个目标开始上传前调用 `verify_source`，发现清单变化立即失败。
3. 上传失败时，重试描述符只保存本次 `ArtifactManifest`、目标类型和项目 ID，不保存旧预检中的远端存在状态，也不再保存 `archivePath` / `ArchivedTarget`。
4. 重试重新执行 SSH 预检，使用新预检的远端路径和存在状态重建 `DeploymentTarget`，并再次调用 `verify_source`；产物变化时返回“部署产物在打包后发生变化，请重新打包”。
5. 重试令牌继续一次性消费并绑定项目，应用退出时清理内存任务。

首版清单仍按路径、类型和大小校验，不计算内容哈希。同大小内容被替换无法识别，这是已知边界；引入哈希会对大型前端目录增加一次完整读取，超出本次解耦范围。

## 状态、通知与错误

运行终态按打包类型表达：

- `local_archive` 成功：`succeeded` 或 `partially_succeeded`，返回 `archivePath`，允许打开归档目录。
- `server_upload` 成功：`succeeded`，不返回 `archivePath`，通知明确为服务器上传完成。
- 构建失败：`failed` 或既有目标级失败状态，不开始上传。
- 上传失败：保留 `package_succeeded_upload_failed` 以兼容现有前端状态，但文案改为“构建成功、上传失败”，并返回重试令牌。
- 上传取消：保持原线上版本；远端安全清理或回滚失败时显式返回恢复路径。

完全上传只在所有选中目标构建成功后开始，不部署部分成功产物。本地归档继续允许既有的部分成功归档行为。

任何类型不匹配、缺失专属参数或携带另一类型参数都直接报错，不做默认回退。服务器上传不允许因为 `outputRoot` 非法而失败，本地归档也不触发 SSH 预检。

## 测试与验证

按 TDD 增加或调整以下覆盖：

### Rust 配置与契约

- 新字段迁移以及 `upload_enabled` 到 `package_type` 的映射。
- 本地归档要求 `outputRoot`，服务器上传允许空 `outputRoot`。
- 服务器上传要求完整服务器配置，本地归档不要求。
- `prepare` 判别结果和 `start` 类型专属参数拒绝规则。

### Rust 运行时

- 共用构建结果能分别进入本地归档和服务器上传分支。
- `server_upload` 不创建、校验或返回归档目录。
- 前端清单根为产物目录，远端相对路径不包含产物目录名。
- 上传失败重试保存源清单，不依赖归档路径。
- 重试前产物未变化可继续，发生变化时明确拒绝。
- 本地归档的覆盖、部分成功、取消和提交事务保持通过。

### 前端

- 打包类型切换及类型专属字段显示。
- 保存校验按类型执行。
- 上传启动不显示或校验归档目录，不调用 `target-check`。
- 本地归档启动不执行上传预检。
- 上传成功不显示归档目录入口，失败可进入重试流程。
- 现有凭据清理、主机信任、远端覆盖确认和日志 lane 行为保持通过。

最低验证：

```text
cargo test release_package -- --nocapture
pnpm test -- <上线包相关测试>
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
git diff --check
```

真实 SSH 环境可用时补充最小上传冒烟；环境不可用时明确记录未执行，不用 mock 结果替代真实协议验证。

## 预计影响范围

主要修改：

- `apps/desktop/src/types/release-package.ts`
- `apps/desktop/src/utils/releasePackage.ts`
- `apps/desktop/src/components/ReleasePackagePanel.vue`
- `apps/desktop/src/composables/useReleasePackageRuntime.ts`
- `apps/desktop/src-tauri/src/tools/release_package.rs`
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- `apps/desktop/src-tauri/src/tools/release_package_deploy.rs`
- 对应前端与 Rust 测试
- `docs/experience/release-package.md`

不新增 IPC channel，不修改 SSH/SFTP 底层连接模块，不改变其他工具。
