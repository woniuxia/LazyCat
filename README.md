# Lazycat / 懒猫

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Platform](https://img.shields.io/badge/platform-Windows%2010%2B-0078D4.svg)
![Stack](https://img.shields.io/badge/stack-Tauri%202%20%2B%20Vue%203-42b883.svg)

> 面向开发者的离线效率工作台：把常用在线小工具收拢到一个本地应用里，开箱即用、数据不出机。

一个面向开发者日常场景的纯离线桌面工具箱，聚焦「高频、小而杂、需要本地可信执行」的任务。

**你可以用它做什么？**

- 在一个窗口里完成编码、转换、加密、网络排障、文件处理、时间计算
- 管理常用应用启动项、代码片段、Hosts 配置、离线手册
- 在无网或受限网络环境下，仍然稳定使用完整工具链

## 为什么是 Lazycat

- **纯离线执行**：所有工具本地运行，不依赖外部 API、无 CDN 运行时依赖
- **数据本地优先**：用户数据写入本地 SQLite，不上传云端
- **一站式集成**：覆盖编码、加密、转换、网络、文件、时间、离线手册等常见研发流程
- **轻量桌面架构**：Tauri 2 + Rust 后端，启动快、占用低
- **可定制工作台**：支持收藏、搜索、快捷启动、菜单显隐、快捷键等个性化配置

## Why Open Source

- **可审计**：核心能力与数据流透明，离线工具更值得信任
- **可扩展**：欢迎按自己的工作流新增工具、面板或离线手册
- **共建共享**：把团队内部常用能力沉淀成可复用的开源资产

## 界面预览

### 首图

首页工作台：按分组浏览高频工具，支持搜索、收藏与快速进入。

![首页总览](img/home.png)

### 四宫格

| 快捷启动 | 代码片段 |
|------|------|
| ![快捷启动](img/launcher.png) | ![代码片段](img/code.png) |
| 管理常用应用，一键拉起本地工具链 | 片段收藏、标签过滤、快速复用 |

| 密码库 | Hosts 管理 |
|------|------|
| ![密码库](img/valut.png) | ![Hosts 管理](img/hosts.png) |
| 按环境/分类管理敏感信息，本地存储 | 多配置切换 + 备份历史，适合联调场景 |

## 核心能力

| 模块 | 你将获得 |
|------|----------|
| 常用工具 | 代码格式化、计算草稿、正则、文本对比、Markdown，一站完成 |
| 编解码 | Base64、URL、MD5、SHA/HMAC、二维码等高频能力随开随用 |
| 加密与安全 | RSA、AES/DES、JWT、UUID/GUID、密码生成等本地安全处理 |
| 数据转换 | JSON/XML/YAML、CSV/JSON、进制/颜色/文本转换，减少手工操作 |
| 网络与系统 | IP/端口连通、DNS、Hosts、端口占用、环境检测、快捷键检测 |
| 文件与媒体 | 文件切割合并、图片转换/压缩/裁剪，常见处理无需额外安装软件 |
| 时间工具 | 时间戳转换、Cron 预览、日期计算，覆盖开发排期与调试场景 |
| 离线手册 | Vue 3、Element Plus、MDN JavaScript，本地可查可跳转 |

## 技术栈

- Tauri 2（Rust backend + WebView frontend）
- Vue 3 + Vite + Element Plus
- TypeScript
- pnpm workspace（monorepo）
- SQLite（Rust `rusqlite`）

## 平台支持

当前仅支持 **Windows 10+**。

## 快速开始（开发）

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

```text
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
resources/manuals/         离线手册资源（Vue 3、Element Plus、MDN JavaScript）
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

## 参与贡献

欢迎通过 Issue / PR 参与改进：

1. Fork 仓库并创建分支
2. 完成功能或修复并补充必要测试
3. 提交 PR，说明改动动机与验证方式

也欢迎提交工具建议、离线手册补充、体验优化建议。
如果这个项目对你有帮助，欢迎点一个 Star。

## License

MIT
