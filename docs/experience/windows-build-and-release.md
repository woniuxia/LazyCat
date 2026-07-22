# Windows 构建与发布经验

适用范围：本地打包、portable、NSIS、WebView2、版本、tag、哈希和 GitHub Release。

关键词：`package:win`、`release:win`、`release:all:win`、`WebView2`、`SkipBuild`

## 命令决策

- 用户只说“打包”或“本地打包”：`pnpm package:win`，生成 lite portable zip 与 SHA256，不上传。
- 正式默认 Release：`pnpm release:win -- -Tag vX.Y.Z`，发布 lite portable。
- 完整四包：`pnpm release:all:win -- -Tag vX.Y.Z`。
- 明确要求 NSIS 安装包：`pnpm build:win`。

`build:portable` 是历史命名，当前底层仍走 NSIS；不要把它当作默认 zip 入口。桌面交付必须通过 Tauri build 嵌入前端资源，不用裸 `cargo build --release` 作为可运行产物。

## 正式发版前置条件

统一根 `package.json`、桌面 `package.json`、`Cargo.toml`、`tauri.conf.json` 四处版本；tag 固定为 `v<version>`。先查本地和远端 tag，再从 `main` 干净工作区执行。版本变更与脚本修复先提交并推送 `origin/main`。

## 默认 lite 与完整四包

`release-all-win.ps1` 默认 `tauri build --no-bundle` 并只处理 lite portable；`-AllPackages` 才构建 lite/full 安装包和绿色包。产物集合驱动缺失检查、SHA256 与上传，不能扫描目录把历史文件误传。

## 中断恢复

构建已成功但哈希/上传中断时使用原命令追加 `-SkipBuild`；只本地出包追加 `-SkipUpload`。恢复模式仍校验当前模式需要的产物与版本一致性。

## Windows 特有问题

- Git `usr/bin/link.exe` 可能遮蔽 MSVC linker；在 PowerShell 层清理 PATH，不依赖 cmd 字符串替换。
- 自定义 manifest 通过 `tauri_build::WindowsAttributes::app_manifest` 注入，不额外用 `embed-resource` 生成第二份 MANIFEST。
- 运行中的 `.exe` 持有文件锁，重建前先结束进程。
- PowerShell 缺少 `Get-FileHash` 时使用 .NET SHA256 fallback。
- 便携包 DLL 同时兼容 `target/release/` 与 `target/release/deps/` 产物位置。

**使用次数**：0
