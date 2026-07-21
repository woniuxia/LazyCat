# Windows 默认打包防呆设计

## 目标

为日常 Windows 本地打包提供唯一、无歧义的命令入口，确保用户只说“打包”或“本地打包”时默认生成 lite portable zip，而不会误生成 NSIS 安装包或触发 GitHub 上传。

## 命令接口

- 新增根命令 `pnpm package:win`，作为日常本地打包的唯一默认入口。
- `package:win` 自动读取根 `package.json` 的当前版本，构造 `v<version>` tag，并调用现有 `release-all-win.ps1`。
- 包装脚本固定传入 `-SkipUpload`，不推送分支、不创建 tag、不上传 GitHub Release。
- 包装脚本不传 `-AllPackages`，因此只生成 lite portable zip 和对应 `SHA256SUMS.txt`。
- 保留 `build:win`、`build:portable`、`build:win:precheck`、`release:win` 和 `release:all:win` 的现有行为，避免破坏已有自动化或使用习惯。

## 双重防呆

1. 命令入口防呆：`package:win` 隐藏 tag 和上传参数，调用者不需要理解发布脚本细节。
2. 错误入口提示：`build-tauri-win.ps1` 在开始完整构建前输出明确提示，说明该脚本会生成 NSIS 安装包，默认 lite portable 应使用 `pnpm package:win`。

提示只增强可见性，不中断或改变旧命令行为。

## 文档规则

- `AGENTS.md` 与 `CLAUDE.md` 同步增加硬规则：用户未指定产物类型、只说“打包”或“本地打包”时，必须执行 `pnpm package:win`。
- 只有用户明确要求安装包时才使用 `pnpm build:win`；明确要求发布 GitHub Release 时才使用 `pnpm release:win`；明确要求四包时才使用 `pnpm release:all:win`。
- `README.md` 的命令表加入 `package:win`，区分本地打包与正式发布。
- 在 `process.md` 记录本次误操作根因和命令选择规则。

## 实现结构

- 新增 `scripts/package-lite-win.ps1`：读取版本并调用现有发布脚本，保持自身逻辑最小。
- 修改根 `package.json`：注册 `package:win`。
- 修改 `scripts/build-tauri-win.ps1`：增加 NSIS 行为提示。
- 新增打包命令静态测试：验证入口存在、自动版本读取、固定 `-SkipUpload`、不传 `-AllPackages`，以及误入口提示存在。
- 同步修改 `AGENTS.md`、`CLAUDE.md`、`README.md` 和 `process.md`。

## 错误处理

- 根版本缺失或为空时，包装脚本立即报错，不调用发布脚本。
- `release-all-win.ps1` 的构建、压缩和哈希错误继续原样向上抛出，不做静默兜底。
- 包装脚本不复制发布逻辑，避免两套实现分叉。

## 验证

1. 先增加静态测试并确认因缺少 `package:win` 和包装脚本而失败。
2. 实现后运行针对性测试和完整 `pnpm test`。
3. 使用 PowerShell AST 解析新旧脚本，确认语法有效。
4. 运行 `pnpm typecheck`。
5. 不在本次防呆改动中自动执行耗时打包；实际 `package:win` 构建由用户明确要求时执行。
