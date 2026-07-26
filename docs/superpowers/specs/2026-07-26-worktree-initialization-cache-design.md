# Worktree 初始化与构建缓存优化设计

## 目标

降低必要 worktree 的首次可用时间，避免初始化阶段重复执行完整 Tauri/NSIS 构建，同时复用现有 pnpm 内容仓库和本机 Rust 编译缓存。

## 现状

- 项目已规定默认直接在 `main` 修改，仅在用户要求隔离、存在并行冲突风险或流程强制时创建 worktree。
- pnpm 内容仓库已全局共享，当前路径为 `E:\.pnpm-store\v10`；每个 worktree 仍需独立生成与本地源码绑定的 `node_modules` 链接树。
- 多个 worktree 各自持有 Rust `target`，Tauri、vendored OpenSSL 和 bundled SQLite 等依赖使首次 Rust 构建成本较高。
- 本机已安装 `sccache`，但未配置 `RUSTC_WRAPPER`，尚未产生缓存请求。
- 根命令 `pnpm build` 会进入桌面包并执行完整 Tauri NSIS 构建，不适合作为 worktree 初始化命令。

## 方案

### 初始化边界

必要 worktree 创建后只执行：

```powershell
pnpm install --frozen-lockfile --prefer-offline
```

初始化阶段不得运行 `pnpm build`、`pnpm build:win`、Tauri build 或安装包构建。测试、类型检查、前端构建和 Rust 检查按实际改动范围执行，不作为所有 worktree 的统一启动成本。

### pnpm 缓存

继续使用 pnpm 全局内容仓库，不共享或 junction 各 worktree 的 `node_modules`。后者包含指向当前 worktree workspace 包的链接，且现存分支锁文件并不完全一致，强行共享会造成跨分支依赖污染。

### Rust 缓存

在已安装 `sccache` 的开发机上，将用户级环境变量 `RUSTC_WRAPPER` 设置为 `sccache`。不在仓库 `.cargo/config.toml` 中强制配置，避免未安装 `sccache` 的开发机和 CI 无法构建。

各 worktree 默认保留独立 `target`，使并行构建和最终产物互不覆盖。共享 `CARGO_TARGET_DIR` 仅作为明确串行构建时的人工选项，不写入项目默认配置。

### 项目规范

同步更新 `AGENTS.md` 与 `CLAUDE.md`：

- 增加 worktree 初始化边界规则；
- 在常用命令表中增加推荐的 worktree 初始化命令；
- 说明已安装 `sccache` 时可通过用户环境复用 Rust 编译缓存，但不将其设为项目硬依赖。

两份文件除文件名和互指外继续保持同构。

## 错误处理

- `pnpm install --frozen-lockfile` 失败时直接暴露锁文件或依赖问题，不回退到会修改锁文件的安装方式。
- `--prefer-offline` 允许缓存缺失时访问配置的软件源；完全离线环境可人工改用 `--offline`，但不作为默认命令。
- `sccache` 不存在时不设置 `RUSTC_WRAPPER`，继续使用 Cargo 原生编译流程。

## 验证

1. 读取用户级 `RUSTC_WRAPPER`，确认其值为 `sccache`。
2. 检查 `sccache --show-stats` 可正常执行。
3. 验证 `AGENTS.md` 与 `CLAUDE.md` 的新增规则和命令完全一致。
4. 运行 `git diff --check`，确保无格式问题和无关改动。

本次不创建测试 worktree，也不运行完整构建；前者会制造额外工作区，后者正是本设计要从初始化阶段移除的成本。
