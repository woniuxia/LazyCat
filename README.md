# Lazycat / 懒猫

纯离线桌面开发者工具箱，面向日常高频研发操作场景。

## 特性亮点

- **纯离线运行** -- 所有功能完全本地执行，无需联网，无外部 API 依赖
- **数据本地化** -- 用户数据存储在本地 SQLite，不上传任何数据，隐私安全
- **All-in-One** -- 30+ 常用开发工具集成于一体，替代零散在线工具
- **轻量快速** -- Tauri 2 + Rust 后端，安装包小、启动快、资源占用低
- **可定制** -- 支持自定义快捷键、菜单显隐、数据目录、外观主题

## 功能一览

| 分类 | 工具 | 说明 |
|------|------|------|
| 常用工具 | 代码格式化 | JSON/XML/HTML/Java/SQL 自动识别格式化 |
| | 计算草稿 | 草稿式计算，回车复制结果并保留历史 |
| | 正则工具 | 正则表达式生成、测试、模板库 |
| | 文本对比 | 双栏文本差异对比 |
| | Markdown | Markdown 编辑与实时预览 |
| 编解码 | Base64 | Base64 编码与解码 |
| | URL 编解码 | URL Encode / Decode |
| | MD5 | 计算 MD5 摘要 |
| | SHA/HMAC | SHA-1/256/512 与 HMAC-SHA256 散列 |
| | 二维码生成 | 根据文本生成二维码 |
| 加密与安全 | RSA 加解密 | RSA 公私钥加解密 |
| | AES/DES | AES 与 DES/3DES 加解密 |
| | JWT 解析 | 离线解析 JWT Token |
| | UUID/GUID/密码 | 标识与随机密码生成 |
| 数据转换 | JSON/XML | JSON 与 XML 双向转换 |
| | JSON/YAML | JSON 与 YAML 双向转换 |
| | CSV/JSON | CSV 与 JSON 转换 |
| | 进制转换 | 二/八/十/十六进制互转 |
| | 颜色转换 | 多格式互转、配色推荐、对比度检查 |
| | 文本处理 | 按行去重与排序 |
| 网络与系统 | IP/端口连通 | TCP 连通性测试 |
| | DNS 查询 | 域名解析与记录查询 |
| | Hosts 管理 | 多配置保存与切换（需管理员权限） |
| | 端口占用 | 端口与进程查询 |
| | 环境检测 | Node 与 Java 版本检测 |
| | 快捷键检测 | 全局快捷键冲突检测 |
| 文件与媒体 | 切割与合并 | 大文件切片与合并 |
| | 图片转换 | 格式转换、缩放、裁剪、压缩 |
| 时间工具 | 时间戳转换 | 时间戳与日期互转 |
| | Cron 工具 | Cron 表达式生成与预览 |
| 离线手册 | Vue 3 手册 | Vue 3 中文开发手册 |
| | Element Plus | Element Plus 组件库文档 |

## 技术栈

- Tauri 2 (Rust backend + WebView frontend)
- Vue 3 + Vite + Element Plus
- TypeScript
- pnpm workspace (monorepo)
- SQLite (Rust `rusqlite`)

## 平台支持

当前仅支持 **Windows**（需 Windows 10 及以上）。

## 快速开始

环境要求：

- Node.js >= 18
- pnpm >= 9
- Rust 工具链（`cargo`、`rustc`）
- MSVC + Windows SDK（含 `kernel32.lib`）
- Perl（建议 Strawberry Perl，用于 OpenSSL vendored 构建）

```bash
pnpm install
pnpm dev
```

`pnpm dev` 会启动 `@lazycat/desktop` 的 `tauri dev`。

## 常用命令

| 命令 | 说明 |
|------|------|
| `pnpm install` | 安装全部依赖 |
| `pnpm dev` | 启动开发模式（Tauri dev） |
| `pnpm typecheck` | 全工作区 TypeScript 类型检查 |
| `pnpm build` | 构建全部 packages + 桌面端 |
| `pnpm test` | 运行全部单元测试 |
| `pnpm test:e2e` | 运行端到端测试 |
| `pnpm lint` | ESLint 代码检查 |
| `pnpm lint:fix` | ESLint 自动修复 |
| `pnpm format` | Prettier 格式化 |
| `pnpm format:check` | Prettier 格式检查 |
| `pnpm build:win` | 构建 Windows NSIS 安装包 |
| `pnpm build:portable` | 构建 Windows 便携版 |
| `pnpm build:win:precheck` | 带环境预检的 Windows 构建（推荐） |

## 目录结构

```
apps/desktop/              Tauri 桌面端（Vue 前端 + Rust 命令）
  src/                     Vue 渲染层源码
  src-tauri/               Rust 后端源码
packages/core/             编解码、转换、文本、时间、正则、Cron、生成器
packages/crypto/           RSA/AES/DES 工具封装
packages/formatters/       JSON/XML/HTML/Java/SQL 格式化（Prettier standalone）
packages/network/          连通性、端口、环境检测
packages/file-tools/       文件切割与合并
packages/image-tools/      图片转换、缩放、裁剪、压缩
packages/db/               SQLite 初始化与存储
packages/ipc-contracts/    请求/响应契约定义
resources/manuals/         离线手册资源（Vue 3、Element Plus）
resources/regex-library/   内置正则模板
scripts/                   构建脚本（build-tauri-win.ps1）
```

## 构建与打包

```bash
# NSIS 安装包
pnpm build:win

# 便携版
pnpm build:portable

# 带环境预检（推荐，自动检查 Rust / VS / Windows SDK）
pnpm build:win:precheck
```

## License

MIT
