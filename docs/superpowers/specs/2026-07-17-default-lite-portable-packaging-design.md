# Windows 默认 Lite Portable 打包设计

## 目标

降低日常 Windows 打包成本：默认只构建不内置 WebView2 的 lite portable zip，同时保留当前一次生成 lite/full 安装包和 portable 包的完整发布能力。

## 命令接口

- `scripts/release-all-win.ps1` 新增 `-AllPackages` switch。
- 直接调用脚本且不传 `-AllPackages` 时，仅生成 `${baseName}_portable-lite.zip` 和 `SHA256SUMS.txt`。
- 传入 `-AllPackages` 时，继续生成当前四个产物：lite/full setup 与 lite/full portable。
- 根 `package.json` 新增 `release:win`，作为默认 lite portable 入口。
- 现有 `release:all:win` 保留，并固定向脚本传入 `-AllPackages`，维持命令名与行为一致。
- `-Tag`、`-Repo`、`-SkipBuild`、`-SkipUpload` 的既有语义不变。

## 构建流程

默认模式执行 Web 渲染层构建，然后通过 Tauri `--no-bundle` 只生成运行文件，复制现有 portable 必需文件并压缩为 lite zip。默认模式不检查固定版 WebView2，不构建 NSIS，也不创建 full portable stage。

全量模式沿用现有流程：检查固定版 WebView2，构建 fixedRuntime NSIS，从完整 NSIS 脚本派生 lite 安装包，再生成 lite/full portable zip。现有辅助函数和产物命名保持不变。

## 产物与发布

脚本按模式构造唯一的 `$artifacts` 集合，缺失检查、SHA256 生成和 GitHub Release 上传均以该集合为准。`SHA256SUMS.txt` 只记录本次模式应有的产物，避免默认模式错误要求其他三个包已存在，也避免上传输出目录中遗留的旧文件。

`-SkipBuild` 继续表示复用对应模式的已有产物：默认模式只要求 lite portable 已存在，全量模式要求四个产物都存在。

## 错误处理与清理

保持现有 fail-fast 行为。构建、压缩、哈希或上传任一阶段失败时显式报错；`finally` 继续清理临时 stage 和临时 Tauri 配置。默认模式不触碰 full stage 和离线配置。

## 文档与验证

同步更新 `README.md`、`AGENTS.md`、`CLAUDE.md` 的命令和打包说明，并保持两份代理规范一致。实现后至少执行 PowerShell 语法解析、命令/文档引用检查，以及无需实际重编译的参数分支静态验证；完整打包耗时较长，不在未明确要求时启动。
