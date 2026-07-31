# 依赖安全与治理实施计划

**Goal:** 分阶段修复前后端已知依赖漏洞，消除明确无用或重复的依赖能力，并在不混入无关迁移的前提下提高构建可复现性。

**Architecture:** 每个阶段只处理一个风险边界，分别提交、分别验证。生产运行时安全修复优先于开发工具追新；Rust 直接漏洞修复与 Tauri 框架升级分开；任何会改变 Node 支持基线、发布版调试能力或用户可见格式化结果的决策，都必须在对应阶段开始前确认。

**Tech Stack:** pnpm workspace、Vue 3、Element Plus、Vite、Vitest、Tauri 2、Rust、Cargo、RustSec

---

## 0. 执行状态

| 阶段                                  | 状态   | 下一动作                   |
| ------------------------------------- | ------ | -------------------------- |
| 阶段一：前端生产依赖安全升级          | 已完成 | 阶段二已完成               |
| 阶段二：Rust 直接安全漏洞与 YAML 替代 | 已完成 | 阶段三已完成               |
| 阶段三：Node 基线与测试工具链安全升级 | 已完成 | 阶段四已完成               |
| 阶段四：Tauri 前后端同步升级          | 已完成 | 阶段五已完成               |
| 阶段五：依赖图精简与清单治理          | 已完成 | 7.1～7.5 均已处理          |

每完成一个阶段，实施对话必须更新本表状态，并在对应阶段末尾追加简短执行记录：完成日期、实际版本、修改文件、验证命令结果、剩余审计项和后续阶段影响。只记录真实执行结果，不提前把计划项标成完成。

---

## 1. 审计基线

审计日期：2026-07-30。

### 1.1 前端

- 锁定版本：Vue `3.5.28`、Element Plus `2.13.2`、Vite `6.4.1`、Vitest `2.1.9`、happy-dom `15.11.7`。
- `pnpm audit --prod --registry=https://registry.npmjs.org` 报告 7 个生产依赖漏洞：
  - Element Plus 依赖链中的 `lodash/lodash-es 4.17.23`：2 high、2 moderate。
  - Vue compiler-sfc 依赖链中的 `postcss 8.5.6`：2 high、1 moderate。
- 完整审计另包含开发/测试链的 2 critical、22 high、14 moderate，重点是：
  - `happy-dom < 20` 的 VM context escape。
  - `vitest < 3.2.6` 的 UI server 任意文件读取/执行。
  - Vite、Rollup、esbuild、ESLint glob 链的多项开发服务器或拒绝服务漏洞。
- Vitest 2 额外带入 Vite 5.4.21 和 esbuild 0.21，与应用的 Vite 6/esbuild 0.25 重复，安装目录约增加 12.7 MiB。
- README 当前声明 Node.js `>= 18`，但 Node 18 已停止维护；本机审计环境为 Node `24.9.0`、pnpm `10.18.1`，根 `packageManager` 仍声明 pnpm `9.15.0`。

### 1.2 Rust

- RustSec 对 `Cargo.lock` 报告 9 个漏洞命中，涉及 5 个唯一公告：
  - `crossbeam-epoch 0.9.18`，修复版本 `>= 0.9.20`。
  - `hickory-proto 0.24.4`，修复版本 `>= 0.26.1`。
  - `lopdf 0.34.0`，修复版本 `>= 0.42.0`，当前建议目标 `0.44.0`。
  - 三个版本的 `quick-xml` 命中两个高危拒绝服务公告，修复版本 `>= 0.41.0`。
- `serde_yml 0.0.12` 和其 `libyml` 依赖被 RustSec 标记为 unsound、unmaintained，且没有修复版本。
- `anyhow 1.0.102` 存在已修复的 unsound 公告，可定向更新到 `1.0.104`。
- `jsonschema 0.18.3` 的默认 feature 引入了产品未使用的 CLI、HTTP/File resolver，以及独立的 `reqwest 0.12`、`clap`、`tower-http` 链。
- `calamine 0.26.1` 带入 `quick-xml 0.31` 和 `zip 2.4`；项目同时直接使用 `quick-xml 0.37` 和 `zip 8.6`。
- Tauri 兼容范围内可从 `2.10.3` 更新到 `2.11.5`，但会联动约 182 个锁文件包，必须独立验证。

### 1.3 审计命令

默认 npm 镜像没有 audit endpoint，统一显式使用官方审计地址：

```powershell
pnpm outdated -r --format json
pnpm audit --prod --registry=https://registry.npmjs.org
pnpm audit --registry=https://registry.npmjs.org
cargo update --dry-run --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo audit --file apps/desktop/src-tauri/Cargo.lock
```

`cargo-audit` 只作为开发审计工具使用，不加入项目运行时或构建依赖。

---

## 2. 全局执行规则

1. 每个阶段开始前读取当前 `AGENTS.md`、相关经验、目标清单、锁文件和 `git status`；不能依赖本文记录的旧行号或旧版本假设。
2. 目标文件已有未提交改动时先读取 diff；直接冲突则暂停确认。
3. 每阶段只修改列出的范围，不顺带升级其他过期依赖。
4. 每阶段先跑定向验证，再跑阶段要求的完整验证；失败必须定位根因，不通过 overrides 或静默回退掩盖。
5. 每阶段完成后重新执行对应安全审计，记录剩余项及其归属阶段。
6. 不自动启动 `pnpm dev` 或产品 UI；需要桌面冒烟时先由用户明确授权。
7. 不自动提交。若用户要求提交，只暂存当前阶段文件，并使用约定式中文提交信息。

---

## 3. 阶段一：前端生产依赖安全升级

### 范围

- `apps/desktop/package.json`
- `pnpm-lock.yaml`
- 仅当升级暴露真实兼容问题时，最小修改对应测试或 Element Plus 调用点。

### 目标

1. Vue 锁定版本从 `3.5.28` 更新到 `3.5.40`。
2. Element Plus 锁定版本从 `2.13.2` 更新到 `2.14.3`。
3. `@vue/compiler-sfc` 与 Vue 保持同版本。
4. 锁文件中的 `lodash` 和 `lodash-es` 至少为 `4.18.1`。
5. 锁文件中的 `postcss` 至少为 `8.5.19`。
6. `pnpm audit --prod` 不再报告本阶段基线中的 7 个生产漏洞。

### 非目标

- 不升级 Vite、Vitest、happy-dom、TypeScript、Tiptap、Monaco、Tauri 或其他业务依赖。
- 不调整 Node/pnpm 支持基线。
- 不修改应用版本号，不做 UI 改版或全局格式化。

### 验证

```powershell
pnpm test
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
pnpm audit --prod --registry=https://registry.npmjs.org
git diff --check
```

额外核对 Element Plus 的表单、日期组件、弹窗、Teleport 和浅色主题相关测试/构建告警。没有用户授权时不启动产品 UI。

### 完成条件

- 目标版本和传递依赖修复线均满足。
- 上述自动化验证全部通过。
- 生产依赖审计清零，或剩余项已证明不属于本阶段依赖链并明确记录。

### 执行记录（2026-07-30）

- 实际版本：Vue 与 `@vue/compiler-sfc` `3.5.40`，Element Plus `2.14.3`，`lodash/lodash-es` `4.18.1`，postcss `8.5.25`。
- 修改文件：`apps/desktop/package.json`、`pnpm-lock.yaml`、`docs/plans/2026-07-30-dependency-governance-plan.md`；升级未暴露需要修改业务代码或测试的兼容问题。
- 验证结果：`pnpm test` 通过（desktop 94 个测试文件、1071 项测试）；`pnpm typecheck` 通过；`pnpm --filter @lazycat/desktop build:web` 通过；`pnpm audit --prod --registry=https://registry.npmjs.org` 返回 `No known vulnerabilities found`；`git diff --check` 通过。
- Element Plus 回归核对：现有表单、日期、弹窗和 Teleport 相关测试通过，渲染层构建及对应 CSS 产物检查通过；构建仅报告 `@vueuse/core` 纯注释位置和既有大 chunk 告警。按阶段限制未启动产品 UI，因此未做运行时视觉验收。
- 剩余审计项：生产依赖审计为 0；开发/测试依赖审计项不属于本阶段，保留到阶段三处理。本阶段不改变阶段二的 Rust 审计基线。

---

## 4. 阶段二：Rust 直接安全漏洞与 YAML 替代

### 范围

- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/src-tauri/Cargo.lock`
- `apps/desktop/src-tauri/src/tools/convert.rs`
- `apps/desktop/src-tauri/src/tools/pdf.rs`
- `apps/desktop/src-tauri/src/tools/dns.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/adapters/dns.rs`
- `apps/desktop/src-tauri/src/tools/request_forward/preflight.rs`
- `apps/desktop/src-tauri/src/tools/pm.rs`
- 对应 Rust 测试和必要 fixtures。

### 目标

1. 用 `serde_norway 0.9.42` 替换 `serde_yml 0.0.12`。
   - 选择依据：RustSec 推荐，且底层使用维护中的 `unsafe-libyaml-norway`。
   - 保持现有 YAML 转换输入/输出契约，补数字、布尔值、null、数组、嵌套对象、多文档/标签错误等边界测试。
2. `lopdf 0.34 -> 0.44`，验证 PDF 信息读取、合并/拆分等当前实际能力。
3. `hickory-resolver 0.24 -> 0.26.1`。
   - 同步把旧 `tokio-runtime` feature 调整为新版有效 feature。
   - 验证 DNS 工具、访问链路诊断、请求转发预检三处调用。
4. `quick-xml 0.37 -> 0.41`，保持 `serialize` 能力。
5. `calamine 0.26 -> 0.36.1`。
   - 利用其 `quick-xml 0.41` 和 `zip 8.6` 依赖消除旧重复分支。
   - 验证 Excel 日期、空单元格、数字/字符串类型和多工作表读取。
6. 定向更新：
   - `crossbeam-epoch >= 0.9.20`
   - `anyhow >= 1.0.104`

### 非目标

- 不升级 Tauri、rusqlite、ureq、windows/windows-sys。
- 不统一 Hyper/ureq 或 OpenSSL/rustls。
- 不借机重写 YAML、PDF、DNS 或 Excel 工具。

### 验证

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml convert -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml pdf -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dns -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml pm -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml request_forward -- --nocapture
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo audit --file apps/desktop/src-tauri/Cargo.lock
git diff --check
```

若 Cargo 测试过滤名与当前模块不一致，先通过 `cargo test -- --list` 定位真实测试名，不把“0 tests”当成通过。

### 完成条件

- 四个直接风险依赖达到修复版本，`serde_yml/libyml` 从锁文件消失。
- 相关用户输入解析行为有回归测试保护。
- RustSec 剩余漏洞只能来自明确归入 Tauri 阶段的传递依赖；否则本阶段不得结束。

### 执行记录（2026-07-31）

- 完成日期：2026-07-31。
- 实际版本：`serde_norway 0.9.42`、`unsafe-libyaml-norway 0.2.15`、`lopdf 0.44.0`（关闭未使用的 default features）、`hickory-resolver/hickory-proto/hickory-net 0.26.1`、`quick-xml 0.41.0`、`calamine 0.36.1`、`zip 8.6.0`、`crossbeam-epoch 0.9.20`、`anyhow 1.0.104`。`serde_yml`、`libyml`、`quick-xml 0.31` 和 `zip 2.4` 已从锁文件消失；Tauri 保持 `2.10.3`，未主动升级 rusqlite、ureq、windows/windows-sys。
- 修改文件：
  - `apps/desktop/src-tauri/Cargo.toml`
  - `apps/desktop/src-tauri/Cargo.lock`
  - `apps/desktop/src-tauri/src/tools/convert.rs`
  - `apps/desktop/src-tauri/src/tools/pdf.rs`
  - `apps/desktop/src-tauri/src/tools/dns.rs`
  - `apps/desktop/src-tauri/src/tools/access_path_diagnostics/adapters/dns.rs`
  - `apps/desktop/src-tauri/src/tools/request_forward/preflight.rs`
  - `apps/desktop/src-tauri/src/tools/pm.rs`
- 验证结果：
  - `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml convert -- --nocapture`：37 passed。
  - `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml pdf -- --nocapture`：20 passed。
  - `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml dns -- --nocapture`：22 passed。
  - `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml pm -- --nocapture`：49 passed。
  - `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml request_forward -- --nocapture`：121 passed。
  - `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`：993 passed、3 ignored、0 failed。
  - `git diff --check`：通过。
- RustSec 审计：使用提交 `7c7ccac53056b87f69ac677f15ea2d9a98a6f8e2`（2026-07-29）的离线数据库执行 `cargo audit --db E:\tmp\lazycat-rustsec-db --no-fetch --stale --file apps/desktop/src-tauri/Cargo.lock`，剩余 4 个漏洞和 21 条 allowed warnings。
  - 4 个漏洞均为阶段四 Tauri 链中的 `quick-xml`：`0.37.5 -> tauri-winrt-notification -> notify-rust -> tauri-plugin-notification` 与 `0.38.4 -> plist -> tauri/tauri-codegen/tauri-plugin` 分别命中 `RUSTSEC-2026-0194`、`RUSTSEC-2026-0195`。
  - 21 条 allowed warnings 不属于漏洞。本阶段明确不扩围处理的两条非 Tauri unmaintained 警告为 `paste 1.0.15 -> rav1e -> ravif -> image 0.25.10` 和 `proc-macro-error2 2.0.1 -> getset -> neli -> local-ip-address 0.6.13`，留作后续依赖治理项。
- 后续影响：阶段三不受本次 Rust 依赖调整影响；阶段四需通过 Tauri 前后端同步升级消除上述 4 个 `quick-xml` 漏洞；两条范围外 allowed warnings 不纳入阶段二或阶段四的漏洞完成条件，后续治理时独立评估其升级范围和回归成本。

---

## 5. 阶段三：Node 基线与测试工具链安全升级

### 开工确认

该阶段会改变开发环境兼容性，开始前必须集中确认一次：

- 推荐方案：最低 Node 提升到 `>= 22.12`。
- 备选方案：继续支持 Node 18，并用兼容版本的 jsdom 替换 happy-dom。该方案增加依赖与 DOM 行为迁移，不推荐。

以下步骤以推荐方案获批为前提。

### 目标

1. README 和根清单明确 Node `>= 22.12`。
2. 根 `packageManager` 与实际采用的 pnpm 10 版本对齐，并补充必要 `engines` 约束。
3. `happy-dom 15.11.7 -> 20.11.1`。
4. desktop 与 formatters 的 `vitest 2.1.9 -> 3.2.7`。
5. Vite 保持 6.x，只更新到至少 `6.4.3`，不在本阶段迁移 Vite 8。
6. 删除 `vitest.config.ts` 中因 Vite 5/6 类型冲突产生的 `vue() as never`。
7. 确认锁文件不再包含 Vitest 2 带来的 Vite 5/esbuild 0.21 分支。
8. 更新 ESLint 同代小版本：
   - ESLint `10.8.0`
   - eslint-plugin-vue `10.10.0`
   - typescript-eslint `8.65.0`

### 非目标

- 不升级 Vite 8、Vitest 4、TypeScript 7 或 `@types/node 26`。
- 不改测试业务断言来迎合错误行为；测试环境差异必须逐项判断。
- 不自动更新 Playwright 浏览器二进制。

### 验证

```powershell
pnpm test
pnpm typecheck
pnpm lint
pnpm --filter @lazycat/desktop build:web
pnpm audit --registry=https://registry.npmjs.org
git diff --check
```

### 完成条件

- Node/pnpm 基线在 README、清单和实际工具链中一致。
- happy-dom、Vitest、Vite 已越过审计修复线。
- Vite 5/esbuild 0.21 重复分支消失。
- 完整测试、类型、构建通过；lint 与升级前基线相比无新增问题，既有问题独立治理。

### 执行记录（2026-07-31）

- 完成日期：2026-07-31。
- 环境基线：根 `engines` 调整为 Node `>= 22.12`、pnpm `>= 10.18.1`，`packageManager` 固定为 `pnpm@10.18.1`；实际验证环境为 Node `24.9.0`、pnpm `10.18.1`。
- 实际版本：happy-dom `20.11.1`、Vitest `3.2.7`、Vite `6.4.3`、ESLint `10.8.0`、eslint-plugin-vue `10.10.0`、typescript-eslint `8.65.0`。审计后额外锁定 yaml `2.8.3`，并在兼容范围内刷新 Rollup `4.62.3`、minimatch `10.2.6`、flatted `3.4.4`、picomatch `4.0.5`、brace-expansion `5.0.9`；esbuild 保持单一 `0.25.12` 分支。
- 修改文件：`README.md`、`package.json`、`eslint.config.mjs`、`apps/desktop/package.json`、`apps/desktop/vitest.config.ts`、`apps/desktop/src/components/RequestForwardPanel.behavior.test.ts`、`packages/formatters/package.json`、`pnpm-lock.yaml` 和本文。阶段一、阶段二已有改动均保留。
- 配置调整：删除 `vue() as never`；根 lint 忽略已有 `.worktrees/**`；为全量并发运行时曾在 5000 ms 边界超时的请求转发行为测试设置单测级 10 秒上限，业务断言未改变。锁文件不再包含 Vite 5、Vitest 2 或 esbuild 0.21 分支。
- 定向验证：两个 happy-dom 测试文件共 49 项通过；请求转发行为测试在一次全量运行中于 5021 ms 超时，单独复跑 1263 ms 通过，增加显式超时后随完整测试通过。
- 完整验证：最终 `pnpm test` 通过（desktop 94 个测试文件、1071 项测试；formatters 无测试并按既有 `--passWithNoTests` 约定退出 0）；`pnpm typecheck` 通过；`pnpm --filter @lazycat/desktop build:web` 通过（Vite `6.4.3`、3483 个模块）。
- Lint 基线：升级前 ESLint `10.0.0`、eslint-plugin-vue `10.8.0`、typescript-eslint `8.56.0` 与升级后工具链均为 220 errors、148 warnings；用户确认本阶段采用“无新增”口径，因此 `pnpm lint` 仍退出 1，既有 368 项不在本阶段批量修复。
- 前端审计：`pnpm audit --prod --registry=https://registry.npmjs.org` 与 `pnpm audit --registry=https://registry.npmjs.org` 均返回 `No known vulnerabilities found`。首次完整审计发现的 12 条 Vite/ESLint 传递链漏洞已通过上述兼容范围内锁文件刷新清零。
- 剩余审计项：前端生产与完整审计均为 0；RustSec 未在本阶段重跑，延续阶段二记录的 4 个阶段四 Tauri `quick-xml` 漏洞和 21 条 allowed warnings。
- 后续影响：Node `>= 22.12` 基线已落地，可供后续工具链演进使用；阶段四仍需独立确认并同步升级 Tauri 前后端，本阶段未修改任何 Tauri 依赖，也未开始阶段四或阶段五。

---

## 6. 阶段四：Tauri 前后端同步升级

### 范围

- `apps/desktop/package.json`
- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/src-tauri/Cargo.lock`
- `pnpm-lock.yaml`
- 仅处理升级暴露的真实 Tauri API 兼容问题。

### 目标版本

- Rust `tauri 2.11.5`
- Rust `tauri-build 2.6.3`
- JS `@tauri-apps/api 2.11.1`
- JS `@tauri-apps/cli 2.11.4`
- Dialog 前后端 `2.7.2`
- Global Shortcut、Single Instance 采用当前 2.x 兼容修复版本。
- Wry/WebView2 由 Tauri 解析到兼容版本，不保留业务无引用的直接 `webview2-com` 钉死。

### 实施要求

1. 先删除 Cargo 清单中 wallpaper CapturePreview PoC 遗留的直接 `webview2-com` 和旧注释。
2. Tauri core、build、前端 API、CLI 和同名插件必须作为同一阶段同步更新。
3. 检查 `cargo update --dry-run` 的全部联动项，重点关注 wry、tao、tray-icon、muda、notify-rust、quick-xml。
4. 不顺带升级 windows/windows-sys、rusqlite 或业务网络栈。

### 验证

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
pnpm test
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
cargo audit --file apps/desktop/src-tauri/Cargo.lock
git diff --check
```

经用户明确允许后再做桌面冒烟，至少覆盖：

1. 主窗口启动与退出。
2. 托盘菜单和恢复窗口。
3. 全局快捷键。
4. 单实例唤醒。
5. Spotlight/通知/Widget 动态窗口。
6. Dialog 调用和 asset protocol 本地资源。

### 完成条件

- Tauri JS/Rust 版本组一致。
- 不再存在业务无引用的 WebView2 直接约束。
- RustSec 中旧 Tauri/通知传递链的 quick-xml 风险消失。
- 自动化验证通过；授权后的桌面冒烟无回归。

### 执行记录（2026-07-31）

- 完成日期：2026-07-31。
- 实际版本：Rust `tauri 2.11.5`、`tauri-build 2.6.3`；JS `@tauri-apps/api 2.11.1`、`@tauri-apps/cli 2.11.4`；Dialog 前后端均为 `2.7.2`；Global Shortcut `2.3.2`、Single Instance `2.4.3`、Notification `2.3.3`、Autostart `2.5.1`。关键传递依赖为 `wry 0.55.1`、`tao 0.35.3`、`tray-icon 0.24.2`、`muda 0.19.3`、`notify-rust 4.18.0`、`tauri-winrt-notification 0.7.3`、`plist 1.10.0`；依赖图只剩 `quick-xml 0.41.0`。
- 修改文件：`apps/desktop/package.json`、`apps/desktop/src-tauri/Cargo.toml`、`apps/desktop/src-tauri/Cargo.lock`、`pnpm-lock.yaml` 和本文。Cargo 清单中的 wallpaper CapturePreview PoC `webview2-com` 直依赖及旧注释已删除；锁定的 `webview2-com 0.38.2` 仅由 Tauri/Wry 间接解析。升级未暴露需要修改业务源码、Tauri 配置或 capability 的 API 兼容问题。
- 修改前依赖检查：`cargo update --dry-run --manifest-path apps/desktop/src-tauri/Cargo.toml` 成功，确认完整更新会联动 179 个包。实施时仅定向更新 Tauri 组及其漏洞链，未顺带刷新 hyper、rustls、tokio、windows/windows-sys、rusqlite、ureq 等业务或范围外依赖。
- 定向验证：全局通知 10 项、快捷键窗口切换 3 项、快捷键导航 6 项、动态参考卡 capability 1 项通过；升级后的 Tauri、Wry、Dialog、Global Shortcut、Single Instance、Notification 和托盘 API 均成功编译。
- 完整验证：`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` 通过（993 passed、3 ignored、0 failed）；`pnpm test` 通过（desktop 94 个测试文件、1071 项测试，formatters 按既有约定无测试并退出 0）；`pnpm typecheck` 通过；`pnpm --filter @lazycat/desktop build:web` 通过。构建仅保留既有的 `@vueuse/core` 纯注释位置和大 chunk 告警。
- Lint 基线：`pnpm lint` 仍为 220 errors、148 warnings，与阶段三基线一致，无新增问题；既有 368 项不在本阶段治理。
- 审计结果：使用本地 `cargo-audit` 执行最新 RustSec 审计，返回 0 个漏洞；阶段二遗留的 4 个 `quick-xml` 漏洞全部消失。`pnpm audit --prod --registry=https://registry.npmjs.org` 与 `pnpm audit --registry=https://registry.npmjs.org` 均返回 `No known vulnerabilities found`。
- 剩余 allowed warnings：RustSec 仍有 21 条，不属于漏洞和阶段四完成条件。其中 10 条为 GTK3 绑定（atk/atk-sys、gdk/gdk-sys、gdkwayland-sys、gdkx11/gdkx11-sys、gtk/gtk-sys、gtk3-macros）unmaintained；其余 unmaintained 为 fxhash、paste、proc-macro-error、proc-macro-error2 及 5 个 unic 组件；unsound 为 glib 与 rand。未在阶段四扩围处理。
- 剩余风险：按边界未启动产品 UI，也未执行桌面冒烟，因此主窗口/退出、托盘、全局快捷键、单实例唤醒、动态窗口、原生通知、Dialog 和 asset protocol 只有编译与自动化验证证据，尚无本轮运行时桌面验收。
- 阶段五影响：阶段五必须基于本次新锁文件重新核对依赖图；本阶段未开始依赖精简、DevTools、前端依赖归属或其他阶段五事项，也未升级 windows/windows-sys、rusqlite、ureq 或业务网络栈。

---

## 7. 阶段五：依赖图精简与清单治理

本阶段按小批次逐项实施，不把所有清理压成一次提交。

### 7.1 Rust 低风险精简

1. `jsonschema` 改为：

   ```toml
   jsonschema = { version = "0.18", default-features = false }
   ```

   当前只编译内存 JSON Schema 和内部 `$ref`，不需要 HTTP/File resolver 或 CLI。验证外部引用仍显式失败，不能静默忽略。

2. `dirs 5 -> 6`，消除 dirs/dirs-sys 双版本。
3. `qrcode` 的 svg/pic feature 没有重依赖，暂不为微小收益改动。

#### 执行记录（2026-07-31）

- 完成日期：2026-07-31；本轮只完成 7.1，7.2～7.5 未开始。
- 清单调整：`jsonschema 0.18.3` 关闭 default features，版本不变；根 `dirs 5.0.1` 升为 `6.0.0`；`qrcode 0.14.1` 及其 default/image/pic/svg features 保持不变。
- 全平台依赖收口：复核发现 `tauri-plugin-autostart 2.5.1 -> auto-launch 0.5.0` 的非 Windows 链路仍保留 `dirs 4.0.0`。经用户确认，vendor crates.io 发布版 `auto-launch 0.5.0`，只把其非 Windows `dirs 4.0` 约束改为 `6`，并通过 `[patch.crates-io]` 接入；LICENSE、README 和四个运行时源码文件与发布版 SHA-256 逐文件一致。锁文件最终只保留 `dirs 6.0.0 / dirs-sys 0.5.0`，Cargo 依赖数从 797 降至 777。
- `jsonschema` 依赖图：CLI、HTTP 和 File resolver features 已关闭，`clap 4.6.1` 与仅由该分支引入的 `reqwest 0.12.28` 从锁文件消失；业务网络栈仍保留既有 `reqwest 0.13.2` 传递链、`ureq 2.12.1`、Hyper/Rustls/Tokio 版本组。
- 版本边界：Tauri `2.11.5`、tauri-build `2.6.3`、Autostart `2.5.1`、windows/windows-sys、rusqlite `0.32.1`、ureq `2.12.1` 均未升级；`auto-launch` Windows 运行时代码未改变。
- 定向验证：`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml schema::tests -- --nocapture` 通过（8 passed）；新增回归测试确认外部 HTTP `$ref` 返回 `valid: false`，错误明确要求 `resolve-http` feature 或自定义 resolver，不会静默忽略。`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml settings -- --nocapture` 通过（2 passed）。
- 完整验证：`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` 通过（994 passed、3 ignored、0 failed）；`pnpm test` 通过（desktop 94 个测试文件、1071 项测试，formatters 无测试并按既有约定退出 0）；`pnpm typecheck` 与 `pnpm --filter @lazycat/desktop build:web` 通过。构建只保留既有的 `@vueuse/core` 纯注释位置和大 chunk 告警。
- Lint 基线：`pnpm lint` 仍为 220 errors、148 warnings，与阶段三、四基线一致；既有 368 项未在本轮处理。
- 审计结果：`pnpm audit --registry=https://registry.npmjs.org` 返回 `No known vulnerabilities found`；本地 `cargo-audit` 使用最新 RustSec 数据库扫描 777 个依赖，返回 0 个漏洞、21 条既有 allowed warnings。
- 剩余风险：vendored `auto-launch` 需要在 Tauri Autostart 正式采用 `auto-launch 0.6` 后移除；本机只编译和测试了 Windows 目标，未执行 Linux/macOS Autostart 运行时验证。本轮未启动产品 UI，也未做桌面运行时冒烟，因此自动启动和外部 Schema 引用只有自动化证据，不构成桌面运行时验收。

### 7.2 发布版 DevTools

Cargo 清单显式启用了 Tauri `devtools`，源码无打开/关闭调用。默认推荐删除该 feature，使 release 包不暴露 Web Inspector；debug 构建不受影响。

该项会改变发布包现场调试能力，实施前必须确认团队没有依赖 release DevTools。若需要保留，记录理由并跳过，不添加新的隐藏开关。

#### 执行记录（2026-07-31）

- 用户确认团队仍依赖 release DevTools 进行现场调试，因此保留 Tauri `devtools` feature；未修改 Cargo 清单、锁文件、业务源码或 Tauri 配置，也未增加隐藏开关。
- 复核结果：`devtools` 只有 `apps/desktop/src-tauri/Cargo.toml` 这一处显式启用点，业务源码没有打开、关闭或切换 DevTools 的调用；Cargo feature 图确认其仍由 `lazycat-desktop` 直接启用。
- 风险接受：release 包继续暴露 Web Inspector，保留现场调试能力的同时维持相应调试面攻击面。本小批次按确认结果跳过依赖修改，未混入后续清理项。

### 7.3 前端依赖归属与构建插件

1. 把运行时静态导入的 `monaco-editor` 从 `devDependencies` 移到 `dependencies`，版本先保持 `0.52.2`。
2. 验证并移除疑似未使用的 `unplugin-auto-import`：
   - 删除 Vite 插件配置。
   - 保留负责模板组件注册的 `unplugin-vue-components`。
   - 以 typecheck、完整测试和 build:web 证明没有隐式导入消费者。
3. 保留显式 `@tiptap/pm` peer，不以“源码无直接 import”为由制造 peer 单例不确定性。

#### 执行记录（2026-07-31）

- 依赖归属：`monaco-editor 0.52.2` 版本不变，从 `devDependencies` 移到 `dependencies`。源码在 `src/utils/monaco-setup.ts` 静态导入 Monaco 主包和 editor/JSON/CSS/HTML/TypeScript worker，`pnpm list --prod` 已将其列为生产依赖。
- 构建插件：删除未使用的 `unplugin-auto-import 21.0.0`、对应 Vite 插件配置和生成文件 `auto-imports.d.ts`；保留 `unplugin-vue-components 31.0.0`、Element Plus resolver、`components.d.ts` 及显式 `@tiptap/pm 3.22.4` peer。
- 安装树变化：`pnpm install --offline` 报告移除 4 个包；锁文件中的 `unplugin-auto-import 21.0.0`、`unimport 5.6.0`、其独立可选链 `@vueuse/core 10.11.1` 和 `vue-demi 0.14.10` 已消失。`local-pkg`、`unplugin-utils` 等仍由 `unplugin-vue-components` 使用，未误删共享依赖。
- 隐式导入复核：AutoImport 只生成过 `ElButton`、`ElInput`、`ElMessage`、`ElSwitch` 声明；业务脚本中的命令式 Element Plus API 均为显式导入，模板组件继续由 Components 插件注册。移除后类型检查、完整测试和生产构建均通过，未发现隐式导入消费者。
- 验证结果：`pnpm test` 通过（desktop 94 个测试文件、1071 项测试，formatters 无测试并按既有约定退出 0）；`pnpm typecheck` 通过；`pnpm --filter @lazycat/desktop build:web` 通过，Monaco 主包及五个 worker 均生成。构建仅保留既有的 `@vueuse/core` 纯注释位置和大 chunk 告警。
- Lint 与审计：`pnpm lint` 仍为既有 220 errors、148 warnings，无新增；`pnpm audit --prod --registry=https://registry.npmjs.org` 和完整 `pnpm audit --registry=https://registry.npmjs.org` 均返回 `No known vulnerabilities found`。
- 剩余边界：本小批次未升级 Monaco、Tiptap 或其他依赖，未修改业务源码，也未启动产品 UI；构建证明资源可打包，不等同于 Monaco 桌面运行时验收。后续清理项未混入本小批次。

### 7.4 可选的 Tiptap 安装树优化

当前 `@tiptap/static-renderer` 的安装树会带入 React/React DOM/scheduler，但实际 Vue 构建使用的 html-string 子路径没有把 React 打入产物。可评估用 `@tiptap/core` 的 `generateHTML` 替代，预期减少约 8 MiB 安装内容。

该替换可能改变 HTML 序列化细节，只有补齐 RichDescriptionViewer 的节点、链接、本地图片和转义输出测试后才能实施；没有测试时保留现状。

#### 执行记录（2026-07-31）

- 实现调整：新增纯序列化入口 `src/rich/render.ts`，`RichDescriptionViewer` 继续负责解析、附件 URL 改写和交互编排，改为调用 `@tiptap/core 3.22.4` 的 `generateHTML`。清单显式声明已有 peer `@tiptap/core`，删除 `@tiptap/static-renderer 3.22.4`，其他 Tiptap 包版本均保持 `3.22.4`。
- 测试基线：新增 5 项 happy-dom 序列化测试，覆盖标题/段落/列表/换行与 marks、合法和危险链接、本地图片 URL 与持久化属性、FileRef 交互所需 class/data 属性，以及普通文本和文件名的 HTML 转义。
- 替换前证据：旧 `renderToHTMLString` 实现有 3 项通过、2 项失败；它会把普通文本和 FileRef 文件名中的 `<img onerror=...>` 解析为真实元素，并吞掉文件名中的 `<draft>` 片段。替换后的同一组测试 5 项全部通过，危险协议仍由既有 `rewriteLocalSrc/sanitizeHref` 清空，文本和自定义节点标签由 DOMSerializer 正确转义。
- 依赖图收益：`pnpm install --offline` 实际移除 4 个包，锁文件不再包含 `@tiptap/static-renderer`、`react 19.2.5`、`react-dom 19.2.5`、`scheduler 0.27.0`。本机包内容合计约减少 7.82 MiB，其中 React 链约 7.22 MiB、Static Renderer 约 0.60 MiB。
- 构建收益：React 原本未进入渲染层产物，因此发布包收益较小；富文本共享 chunk 从约 399.78 kB（gzip 126.88 kB）降至约 395.83 kB（gzip 125.72 kB）。本项主要收益仍是安装树、peer 链和审计维护面收缩，以及修复旧 renderer 的转义缺陷。
- 完整验证：`pnpm test` 通过（desktop 95 个测试文件、1076 项测试，formatters 无测试并按既有约定退出 0）；`pnpm typecheck` 与 `pnpm --filter @lazycat/desktop build:web` 通过。构建仅保留既有的 `@vueuse/core` 纯注释位置和大 chunk 告警。
- Lint 与审计：`pnpm lint` 仍为既有 220 errors、148 warnings，无新增；生产和完整 `pnpm audit` 均返回 `No known vulnerabilities found`。
- 剩余边界：本轮未升级 Tiptap 版本，未启动产品 UI；自动化覆盖了序列化和构建，但未执行 Todo/PM 详情页的桌面运行时视觉验收。本小批次未混入 7.5，后者已在后续独立完成。

### 7.5 遗留脚本依赖

`scripts/scrape-mdn-js.mjs` 硬编码引用 `../node_modules/puppeteer/...`，但清单和锁文件没有 Puppeteer，也没有正式脚本入口。

实施前确认二选一：

1. 已废弃：删除脚本，并检查文档/资源生成链没有引用。
2. 仍维护：正式声明 Puppeteer devDependency、增加 package script，并改用正常包导入。

不得继续保留不可复现的隐式依赖状态。

#### 执行记录（2026-07-31）

- 维护结论：现有 872 个 MDN JavaScript 离线页面、约 72.34 MiB 资源由该脚本生成，`docs/experience/manuals-and-resources.md` 仍保留基于 Puppeteer 和系统 Edge 的抓取方案，因此脚本不是明确废弃项。经用户确认按继续维护处理。
- 清单与脚本：根清单新增 `manuals:sync:mdn-js` 入口和精确版本的开发依赖 `puppeteer-core 25.4.0`；脚本改为标准 ESM 包导入并直接调用 `puppeteer.launch`，继续使用既有系统 Edge 路径，不引入完整 `puppeteer` 或浏览器下载。
- 依赖图变化：锁文件新增 Puppeteer Core 工具链共 23 个包；`pnpm why puppeteer-core` 确认唯一入口为根 devDependency `25.4.0`，没有完整 `puppeteer` 包和 Chrome postinstall 下载链。
- 定向验证：`node --check scripts/scrape-mdn-js.mjs` 通过；ESM 导入及 `launch` API 检查通过；`C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe` 存在；Rust MDN 注册测试通过（1 passed）；旧的硬编码 Puppeteer 导入残留检查无命中。
- 阶段完整验证：`cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` 通过（994 passed、3 ignored、0 failed）；`pnpm test` 通过（desktop 95 个测试文件、1076 项测试）；`pnpm typecheck` 通过；`pnpm --filter @lazycat/desktop build:web` 通过（3483 modules）；`pnpm lint` 仍为既有 220 errors、148 warnings，无新增。
- 审计结果：生产与完整 `pnpm audit` 均为 0 个漏洞；本地 `cargo-audit` 扫描 777 个包，返回 0 个漏洞、21 条既有 allowed warnings。
- 验收边界：未运行真实 MDN 抓取，因为该操作需要联网并会批量覆盖已提交的离线资源；当前证据只覆盖依赖可解析、脚本语法、模块 API、浏览器路径和资源注册，不构成抓取运行时验收。本阶段未启动产品 UI，也未做桌面运行时冒烟。

### 阶段五完成记录（2026-07-31）

- 7.1 已关闭 `jsonschema` 未使用的默认能力、统一 `dirs 6` 并将 Cargo 图从 797 个包降至 777 个；7.2 按用户确认保留 release DevTools；7.3 修正 Monaco 生产依赖归属并移除未使用的自动导入链；7.4 移除 Static Renderer/React 安装链并补齐富文本序列化安全测试；7.5 正式纳管仍在维护的 MDN 抓取脚本。
- 最终自动化基线为 Rust 994 passed、3 ignored，desktop 95 个测试文件、1076 项测试；类型检查、渲染层构建和 `git diff --check` 通过，前端与 RustSec 漏洞均为 0。阶段五没有升级 windows/windows-sys、rusqlite、ureq、Tauri 版本组或业务网络栈。
- 剩余风险：release Web Inspector 按团队调试需求继续保留；vendored `auto-launch` 需要在上游采用兼容 `dirs` 后移除；Tiptap 渲染未做桌面视觉验收；MDN 脚本未做真实联网抓取。以上均不由自动化验证替代运行时验收。

### 验证

每个小批次至少执行对应定向测试，并在阶段结束执行：

```powershell
pnpm test
pnpm typecheck
pnpm lint
pnpm --filter @lazycat/desktop build:web
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
pnpm audit --registry=https://registry.npmjs.org
cargo audit --file apps/desktop/src-tauri/Cargo.lock
git diff --check
```

---

## 8. 暂缓升级清单

这些项目不是当前安全闭环的一部分，必须独立评估，不能在前述阶段顺带处理。

| 依赖                       | 当前方向                                             | 开工前置                             |
| -------------------------- | ---------------------------------------------------- | ------------------------------------ |
| `rusqlite 0.32 -> 0.40`    | 有 bundled SQLite 修复收益，但涉及约 41 个 Rust 文件 | 完整数据库、迁移、事务和二次启动测试 |
| `ureq 2 -> 3`              | API 和错误模型迁移，保留同步短请求边界               | health check、思源、上线包定向测试   |
| Tiptap `3.22 -> 3.29`      | 七个包同升                                           | 富文本 JSON/HTML 序列化和图片测试    |
| Prettier/SQL/XML formatter | 会改变用户可见输出                                   | formatter golden tests               |
| TypeScript 7               | 工具链主版本                                         | Node 基线稳定、生态兼容确认          |
| Vite 8 / Vitest 4          | 可在后续统一现代工具链                               | Node `>= 22.12` 已落地，单独迁移     |
| Monaco `0.52 -> 0.56`      | 0.x 版本和 worker 集成风险                           | Diff、Schema、语言 worker 冒烟       |
| `windows/windows-sys`      | 调用面大且与 Tauri/WebView 版本耦合                  | Tauri 阶段稳定后独立处理             |

---

## 9. 明确不做的替换

- 不把 Monaco 替换为 CodeMirror；当前差异编辑、Schema 诊断和多语言 worker 使用较深，迁移不是依赖治理。
- 不强行统一 Hyper 与 ureq；前者是异步转发服务，后者是同步短请求。
- 不强行统一 OpenSSL 与 rustls；SSH/SFTP、Vault、RSA/AES、TLS 诊断和请求转发边界均有真实用途。
- 不单独裁剪 `image` 默认 feature；`windows-icons` 同样无条件启用 image 默认能力，单改直接依赖没有实际收益。
- 不替换 Frappe Gantt、SortableJS、markdown-it、regexp-tree/railroad-diagrams；当前均有实际深度调用，暂无低成本等价替代。

---

## 10. 最终验收

全部阶段完成后执行：

```powershell
pnpm test
pnpm typecheck
pnpm lint
pnpm --filter @lazycat/desktop build:web
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
pnpm audit --prod --registry=https://registry.npmjs.org
pnpm audit --registry=https://registry.npmjs.org
cargo audit --file apps/desktop/src-tauri/Cargo.lock
git diff --check
```

最终交付记录必须包含：

1. 直接依赖和主要传递依赖的前后版本。
2. npm 与 RustSec 剩余告警；每项要么修复，要么有明确风险接受依据。
3. 各阶段实际执行的测试、类型、lint、构建和桌面冒烟结果。
4. Node、pnpm、Rust 最低版本和审计环境版本。
5. 暂缓升级项的最新状态，不把未实施内容描述成已完成。
