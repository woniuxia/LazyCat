# Lazycat / 懒猫

纯离线桌面开发者工具箱，面向日常高频研发操作场景。

#  codex -m gpt-5.3-codex --sandbox workspace-write

## 技术栈

- Tauri 2 (Rust backend + WebView frontend)
- Vue 3 + Vite + Element Plus
- TypeScript
- pnpm workspace (monorepo)
- SQLite (Rust `rusqlite`)

## 快速开始

```bash
pnpm install
pnpm dev
```

说明：`pnpm dev` 会启动 `@lazycat/desktop` 的 `tauri dev`。

## 常用命令

```bash
pnpm typecheck
pnpm build
pnpm test
pnpm test:e2e
pnpm build:portable
pnpm build:win
```

## 已实现功能（当前版本）

- 编解码：Base64、URL、MD5、文本生成二维码
- 加解密：RSA、AES、DES/3DES
- 格式化：JSON/XML/HTML/Java/SQL（编辑器支持缩进辅助）
- 数据转换：JSON/XML/YAML/CSV 常见互转
- 文本处理：按行去重、按行排序
- Hosts 管理：多配置保存、列表、删除、激活切换
- 网络与系统：TCP 端口连通测试、端口占用、Node/Java 环境检测
- 文件工具：大文件切割、小文件合并
- 时间工具：时间戳与日期互转、Cron 生成与预览
- 图片工具：格式转换、缩放、裁剪、压缩
- 生成器：UUID、GUID、随机密码
- 正则工具：模板库、表达式生成、表达式测试
- 离线手册索引：Vue2 / Vue3 / Element Plus

## 目录结构

```txt
apps/desktop            Tauri 桌面端（Vue 前端 + Rust 命令）
packages/core           编解码/转换/文本/时间/正则/cron/生成器
packages/crypto         RSA/AES/DES 工具封装
packages/formatters     JSON/XML/HTML/Java/SQL 格式化
packages/network        连通性、端口、环境检测
packages/file-tools     切割/合并等文件能力
packages/image-tools    图片转换、缩放、裁剪、压缩
packages/db             sqlite 初始化与存储
packages/ipc-contracts  请求/响应契约
resources/manuals       离线手册资源
resources/regex-library 内置正则模板
```

## 打包说明

- `pnpm build:win`：构建 Windows NSIS 安装包（Tauri）
- `pnpm build:portable`：当前与 `build:win` 使用同一打包目标（后续可扩展真正便携包）
- `pnpm build:win:precheck`：先检查 Rust / VS / Windows SDK，再执行构建（推荐，含前端与 Tauri 分步构建）

## 当前约束

- Tauri 构建依赖 Rust 工具链（`cargo`、`rustc`），未安装时 `pnpm build` 会失败。
- Windows 下还需要可用的 MSVC + Windows SDK（含 `kernel32.lib`）。
- 当前加密实现依赖 `openssl` 的 vendored 构建，Windows 还需要可用的 `perl`（建议 Strawberry Perl）。

## License

MIT
