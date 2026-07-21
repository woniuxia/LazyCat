# Process Log

本文件记录 LazyCat 项目中重要/复杂操作的处理流程与踩坑经验。

**使用次数规则**：每条记录有 `使用次数` 字段，初始为 0。后续会话遇到相同问题并参考该记录时 +1，并追加引用日期。当使用次数 >= 3 时，固化到 `CLAUDE.md` 对应章节。

---

<!-- 新记录添加在此处，最新的在最上面 -->

## 2026-07-21: Windows 本地打包使用唯一防呆入口

**场景**: 用户只说“打包”时，代理误把 Windows 打包理解为 NSIS 安装包，执行了名称含 `precheck` 但实际会完整构建 NSIS 的脚本。

**问题**:
1. `release:win` 虽已默认生成 lite portable，但本地打包仍需调用者手动传版本和 `-SkipUpload`，通用“打包”缺少唯一入口。
2. `build:win:precheck` 的名称容易被理解为轻量检查，实际会继续执行 `tauri build --bundles nsis`。

**解决**:
1. 新增 `pnpm package:win`，自动读取根版本并固定调用 `release-all-win.ps1 -SkipUpload`，默认只生成 lite portable zip 和 SHA256。
2. 保留旧命令行为，在 NSIS 构建脚本入口增加纠错提示。
3. 在 `AGENTS.md`、`CLAUDE.md` 和 README 中统一命令决策：未指定产物类型的“打包”必须走 `package:win`。

**关键点**:
- 通过唯一命令表达用户意图，比继续堆叠文档提醒更可靠。
- 本地打包与正式发布必须分开：前者禁止上传，后者继续显式要求 tag。

**涉及文件**:
- scripts/package-lite-win.ps1
- scripts/build-tauri-win.ps1
- package.json
- AGENTS.md
- CLAUDE.md
- README.md
- apps/desktop/src/utils/windowsPackagingCommand.test.ts

**验证**:
- pnpm --filter @lazycat/desktop test -- src/utils/windowsPackagingCommand.test.ts
- PowerShell AST 解析
- pnpm test
- pnpm typecheck

**使用次数**: 0

## 2026-07-20: 请求转发日志工作台布局与时间筛选优化

**场景**: 请求转发在宽屏/全屏下需要稳定排列日志筛选控件，同时提升规则导航和 HTTP 日志表的扫描效率，并降低时间范围输入成本。

**解决**:
1. 规则条目将标题独占首行，协议、运行状态和自动启动标识下移到独立元信息行，避免标题与状态互相挤压。
2. 日志工具栏改为基于观测区域容器宽度的网格布局，宽屏保持固定列位，780px 以下使用等宽双列，480px 以下降为单列；Select 和 InputNumber 显式约束为网格列宽，避免内部最小宽度造成叠加。
3. 两个原生 datetime-local 输入合并为 Element Plus 日期时间范围选择器，默认值为当前时间前一小时至当天 23:59:59，仍在查询入口统一转为 UTC。
4. HTTP 请求方式从路径文本中拆出为独立日志列；响应式降级按列索引同步调整，窄栏仍保留结果、请求方式、路径和时间。

**关键点**:
- 日期选择器持有本地时间字符串，默认区间由纯函数生成，避免把本地日期语义和 UTC 查询语义混在组件模板中。
- 可拖拽三栏布局的工具栏断点应基于实际中栏容器宽度，而不是只看浏览器视口。
- 新增表格列时必须同步更新 aria-colcount、各容器断点的 grid-template-columns 和隐藏列索引。

**涉及文件**:
- apps/desktop/src/components/RequestForwardPanel.vue
- apps/desktop/src/components/request-forward/RequestForwardRuleList.vue
- apps/desktop/src/components/request-forward/RequestForwardLogList.vue
- apps/desktop/src/components/RequestForwardPanel.test.ts
- apps/desktop/src/utils/requestForward.ts
- apps/desktop/src/utils/requestForward.test.ts

**验证**:
- pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts（98 通过）
- pnpm typecheck
- pnpm --filter @lazycat/desktop build:web

**使用次数**: 0

## 2026-07-20: 上线包已有归档采用确认后完整替换

**场景**: 目标归档目录已存在时，用户需要在取消和直接覆盖之间做一次明确选择，同时保证新包生成失败不会破坏旧归档。

**解决**:
1. 启动前通过独立 `target_check` action 查询当前项目目标路径；存在时使用二次确认框，覆盖授权不持久化。
2. `overwriteExisting` 从前端显式传入 Rust，缺失或非布尔值均按安全路径处理；后端启动前和最终提交时都重新校验目标状态。
3. 新产物始终先写 staging；覆盖提交把旧目录重命名到 runId 专属 backup，再原子切换 staging，切换失败恢复旧目录，成功后清理 backup。

**关键点**:
- 目录检查只服务于交互，不能替代提交时校验，避免检查与启动之间的竞态静默覆盖。
- “完整替换”不能用复制覆盖实现，否则旧文件会残留；同卷 `rename` 到备份后再提交才能保证旧目录不被半成品直接删除。
- 覆盖目标必须是目录；文件目标、预先存在的 staging/backup 路径都要显式失败。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/release_package.rs`
- `apps/desktop/src-tauri/src/tools/release_package_archive.rs`
- `apps/desktop/src-tauri/src/tools/release_package_runtime.rs`
- `apps/desktop/src/components/ReleasePackagePanel.vue`

**验证**:
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`
- `pnpm --filter @lazycat/desktop exec vitest run src/utils/releasePackage.test.ts src/components/ReleasePackagePanel.test.ts`
- `pnpm typecheck`

**使用次数**: 0

## 2026-07-19: 请求转发结构化错误与恢复动作

**场景**: 运行错误需要同时服务即时 IPC 失败、运行状态展示和后续批量操作，并兼容历史纯文本错误。

**解决**:
1. 在 Rust 域内用带 marker/version 的 JSON envelope 统一输出稳定错误码、原始 message 和失败后的真实 runtime state；分类集中维护，并按错误阶段与固定上下文分层匹配。
2. 前端纯函数严格校验 marker、版本、错误码和 state；支持直接 JSON 与 `Error.message` 内嵌 envelope，未知或畸形输入保留完整原文并回退 `unknown`。
3. 恢复动作由纯映射生成，Panel 只编排现有启动、编辑和无副作用预检；建议端口必须绑定当前规则的实际预检结果，并由用户显式应用到编辑态。

**关键点**:
- 监听占用只能在监听绑定上下文中识别 `AddrInUse`/Windows 10048，不能把任意 connect/io 错误归为端口占用。
- 分类规则有优先级：强 DNS 阶段/固定上下文先于 TLS，明确 TLS/证书/握手上下文其次，通用 DNS 关键字最后兜底；既避免目标主机名中的 `tls`/`certificate` 误导分类，也避免 TLS 的 `invalid dnsname` 被误判为 DNS。
- 晚到的恢复预检响应必须同时校验 request token、selection token 和 rule id。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/request_forward/{model,mod,runtime,tcp}.rs`
- `apps/desktop/src/{types/request-forward.ts,utils/requestForward.ts,components/RequestForwardPanel.vue}`
- 对应 Rust 与前端测试

**验证**:
- `cargo test --target-dir E:\tmp\lazycat-task1-target request_forward -- --nocapture`（105 项通过）
- `pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts src/composables/useRequestForwardPreflight.test.ts`（89 项通过）
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --target-dir E:\tmp\lazycat-task1-target`
- `pnpm typecheck`

**使用次数**: 0

## 2026-07-19: 上线包按目标并行与项目级日志

**场景**: 同一次上线包任务需要按需构建前端或后端，两端并行执行且允许一端失败后保留另一端成功产物，同时让运行日志归属于具体项目。

**解决**:
1. 由轻量协调器创建共享 staging、派生前后端工作线程并统一提交最终目录；工作线程只归档自己的独立顶层目标。
2. 工作线程的业务失败只汇总为失败目标，不传播共享取消；只有用户取消才设置共享取消标记、终止两个 PID 槽并清理本次 staging。
3. 有成功目标时统一提交并返回 `succeeded` 或 `partially_succeeded`；全部失败不生成最终目录，用户取消不保留半成品。
4. 前端运行态按 `projectId` 保存当前会话最近一次结果，按 `frontend` / `backend` 分列有界保存日志；运行目标每次打开确认弹窗时恢复为全选。

**关键点**:
- 并行工作线程不能分别提交最终目录，否则会产生覆盖、半成品可见和取消竞态；协调器只负责生命周期与最终提交，不承担目标内构建逻辑。
- 业务失败和用户取消是不同控制信号：前者允许另一端继续，后者必须传播到全部工作线程。
- 长任务日志需要同时绑定 `runId` 和业务实体 `projectId`，避免项目切换或迟到事件覆盖当前视图。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/release_package*.rs`
- `apps/desktop/src/composables/useReleasePackageRuntime.ts`
- `apps/desktop/src/components/ReleasePackagePanel.vue`

**验证**:
- `cargo test release_package --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture`（24 项通过）
- `pnpm test src/components/ReleasePackagePanel.test.ts src/composables/useReleasePackageRuntime.test.ts src/utils/releasePackage.test.ts`（37 项通过）
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-19: 请求转发监听端点快捷操作

**场景**: 在请求转发观测工作台中补充监听与目标地址复制、HTTP 本地访问和命令示例，减少手工拼接地址与命令。

**解决**:
1. 用纯函数统一生成监听端点、本地 HTTP URL 和 PowerShell/curl 命令；IPv6 端点统一加方括号，HTTP 访问时把通配监听地址映射到对应回环地址。
2. 标题区子组件只展示协议相关操作并发出语义事件；剪贴板与外部链接副作用集中在主面板，失败统一显示实际错误。
3. HTTP 命令始终指向本地监听 URL；TCP/UDP 只提供地址复制，不展示浏览器或 HTTP 命令入口。

**关键点**:
- 监听地址、HTTP 可访问 URL、转发目标是三个不同语义；复制监听地址保留真实 bind host，浏览器 URL 才把 `0.0.0.0` / `::` 映射为 `127.0.0.1` / `::1`。
- PowerShell 与 POSIX shell 的单引号转义规则不同，命令示例应分别转义，不能直接插值未处理的 URL。

**涉及文件**:
- `apps/desktop/src/utils/requestForward.ts`
- `apps/desktop/src/components/RequestForwardPanel.vue`
- `apps/desktop/src/components/request-forward/RequestForwardEndpointActions.vue`
- 对应前端测试与自动生成组件声明

**验证**:
- `pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts`（69 项通过）
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `git diff --check`

**使用次数**: 0

## 2026-07-19: 高收益首版工具统一补齐工作流与正确性边界

**场景**: 集中优化 Markdown、文本对比、图片转换、PDF、JSON Schema、环境检测、快捷启动和浏览器身份八个已有工具。
**问题**:
1. 多个首版工具只有核心算法或单次执行入口，缺少文件打开/保存、结果导出、覆盖保护、键盘替代和错误恢复。
2. 部分 UI 暴露的参数与真实能力不一致，例如非 JPEG 质量参数、Schema 组合样例、环境命令只要能启动就标记成功。
3. 浏览器身份和快捷启动的数据模型已经具备扩展字段，但 UI 与执行链路没有完整使用。
**解决**:
1. Markdown 接入本地 GFM、安全净化和代码高亮；Diff 补双文件、语言、差异导航与导出；通用文本读取限制 20 MB。
2. 图片和 PDF 对覆盖源文件、输出冲突、裁剪范围和合并顺序做前后端显式校验，并补结果目录入口。
3. Schema 样例生成支持本地 `$ref`、循环保护、`allOf` 合并和显式组合分支；环境检测增加退出码、超时、多 PATH 与关键环境变量诊断。
4. Launcher 开放路径和参数编辑、失效路径提示与键盘排序；浏览器身份扩展到 Edge + Chrome，并同步 Spotlight provider。
**关键点**:
- UI 参数必须与后端真实能力一致；不支持的质量或解析能力要明确说明，不能制造“已生效”的假象。
- 文件型工具必须把覆盖保护、输出定位、大小/编码边界和未保存状态作为主流程，而不是事后补丁。
- 扩展多提供方能力时，配置、统计、别名和隐藏状态必须按 provider 隔离，同时保持旧配置兼容。
**涉及文件**:
- `apps/desktop/src/components/{MarkdownPanel,DiffPanel,ImagePanel,PdfPanel,JsonSchemaPanel,EnvPanel,LauncherPanel,BrowserProfilesPanel}.vue`
- `apps/desktop/src-tauri/src/tools/{file,image,pdf,schema,env,launcher,browser_profiles}.rs`
- 对应 bridge、types、Spotlight provider、utils、tests 与本地前端依赖
**验证**:
- `pnpm test`（67 个测试文件、656 项通过）
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --bin lazycat-desktop`（572 项通过；本地 UDP fixture 需沙箱外权限）

**使用次数**: 0

## 2026-07-19: 请求转发预检状态机行为化测试

**场景**: 预检请求需要同时绑定当前编辑意图和规范化表单，并保证并发、切换规则与组件卸载后的晚响应不会污染界面。

**解决**:
1. 把预检的 token、接受快照、loading 和 result 收口到独立 composable，面板只保留表单校验、busy gate、提示文案与“检测并启动”的 ready gate。
2. 上下文快照显式覆盖 selection token、编辑目标、草稿状态和规范化写入 payload 的全部字段；只有请求 token 与当前上下文都一致时才应用结果或上报错误。
3. 用 deferred Promise 行为测试覆盖当前响应、过期成功/失败、A/B 并发解锁、已接受结果失效和卸载失效，组件源码测试只验证接线边界。

**关键点**:
1. 旧请求的 `finally` 也可能破坏新请求的 loading，解锁必须额外校验最新 token。
2. `invalidate` 必须同时推进 token、清空结果与接受快照并立即解锁；不能只清 UI 结果。
3. 是否允许“检测并启动”必须验证接受快照仍匹配当前上下文，不能只依赖最近一次结果的 `ready`。

**涉及文件**:
- `apps/desktop/src/composables/useRequestForwardPreflight.ts`
- `apps/desktop/src/composables/useRequestForwardPreflight.test.ts`
- `apps/desktop/src/components/RequestForwardPanel.vue`
- `apps/desktop/src/components/RequestForwardPanel.test.ts`

**验证**:
- `pnpm test src/composables/useRequestForwardPreflight.test.ts src/components/RequestForwardPanel.test.ts src/utils/requestForward.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `git diff --check`

**使用次数**: 0

## 2026-07-19: Cron 方言兼容与输出校验

**场景**: Cron 工具生成的表达式被复制到 Linux Crontab、Spring 和 Quartz 等不同运行环境后频繁报字段数或日/周字段错误。
**问题**:
1. 面板把 Spring 6 字段当成通用格式输出，Linux Crontab 会把秒字段错当分钟；Quartz 还要求日和周字段必须有一个为 `?`。
2. Linux/Spring 与 Quartz 的数字星期编号不同，直接用同一个解析器预览会出现“校验通过但触发日偏移”。
3. `L/W/#` 由部分框架支持，但当前 Rust 预览解析器不支持，旧模板中的 `L` 会在工具内直接报错。
**解决**:
1. 新增 `linux5`、`spring6`、`quartz` 目标方言；规范化、生成、预览、描述统一携带 `standard`，输出按目标字段数和日/周规则转换。
2. 预览使用内部 6 字段表达式，并按方言转换数字星期；输出仍保留用户目标方言，避免把内部实现细节复制到生产。
3. 面板默认 Linux 5 字段，切换目标时自动修正秒字段和 Quartz `?`；复制前强制后端校验；移除无法可靠预览的 `L` 模板，对特殊语法和混合星期编号显式报错。
**关键点**:
- Cron 的字段数量和星期编号不是跨实现的稳定协议，目标运行环境必须成为输入的一部分。
- 不能只依赖内部解析器返回成功；复制、模板和预览都要走同一个目标方言规范化路径。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/cron.rs`
- `apps/desktop/src/components/CronPanel.vue`
- `apps/desktop/src/utils/cron.ts`
- `apps/desktop/src/utils/cron.test.ts`
- `apps/desktop/src/types/cron.ts`
**验证**:
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::cron::tests -- --nocapture`
- `pnpm --filter @lazycat/desktop test -- src/utils/cron.test.ts`
- `pnpm --filter @lazycat/desktop typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-18: 访问链路诊断高级参数持久化

**场景**: 在访问链路诊断与单项探测共享 `network_diagnostics` 设置 envelope 的前提下，持久化诊断高级参数。
**解决**:
1. 将默认协议、连接 IP、TLS/HTTP 覆盖、指定 DNS、代理画像和两级超时统一纳入 `diagnosisAdvancedParams`，旧设置缺少字段时按既有默认值归一化。
2. 诊断组件初始化时恢复参数，修改后短延迟保存；卸载前刷新待保存内容。
3. 每次写入先读取当前 envelope 再合并，报告、收藏、历史和高级参数互不覆盖。
**关键点**:
1. 多个常驻组件共享同一设置 key 时，各自只能更新所属字段，不能持有启动时快照直接覆盖整个 envelope。
2. 持久化边界必须校验枚举和数值范围，损坏值回到与表单一致的默认值。
**涉及文件**:
- `apps/desktop/src/components/network/NetworkDiagnosisWorkspace.vue`
- `apps/desktop/src/utils/networkDiagnosticsPersistence.ts`
- `apps/desktop/src/utils/networkDiagnosticsPersistence.test.ts`
- `apps/desktop/src/components/NetworkPanel.test.ts`

**验证**:
- `pnpm test src/utils/networkDiagnosticsPersistence.test.ts src/components/NetworkPanel.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-18: 请求转发可读性与双侧栏宽度偏好

**场景**: 提升请求转发工作台的文字可读性，并让规则导航与日志详情都支持可持久化的宽度调整。
**解决**:
1. 请求转发范围内统一上调导航、统计、日志、详情和规则弹窗的文字基线，不修改全局 Element Plus 主题。
2. 规则摘要拆成“监听”和“转发”两行，协议与运行状态保留在标题行，长端点允许自然换行并保留完整悬停文本。
3. 左侧规则栏复用详情栏的宽度偏好模式：拖拽结束或键盘调整后持久化首选宽度，窗口缩放只临时钳制渲染宽度。
4. 规则字段说明迁移到标签旁的问号 tooltip，同时支持鼠标悬停和键盘聚焦；校验错误仍保留在字段下方。
**关键点**:
- 双侧 splitter 必须各自维护拖拽起点、首选宽度和设置 key，避免互相覆盖。
- 宽度钳制要给中央工作区保留最低可用空间；移动端继续使用单列布局并隐藏 splitter。
- tooltip 只承载帮助说明，不能取代字段标签、校验错误或暴露监听地址的安全警告。
**涉及文件**:
- `apps/desktop/src/components/RequestForwardPanel.vue`
- `apps/desktop/src/components/request-forward/RequestForwardRuleList.vue`
- `apps/desktop/src/components/request-forward/RequestForwardRuleForm.vue`
- `apps/desktop/src/components/request-forward/RequestForwardRuleDialog.vue`
- `apps/desktop/src/components/request-forward/RequestForwardLogList.vue`
- `apps/desktop/src/components/request-forward/RequestForwardLogInspector.vue`
- `apps/desktop/src/utils/requestForward.ts`
**验证**:
- `pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts`（56 通过）
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 1（2026-07-19：继续提升请求转发全页字体基线）

## 2026-07-18: 访问链路诊断分阶段引导

**场景**: 在已有代理、Hosts、DNS、TCP、TLS、HTTP 六层事实报告上，补足体系化、循序渐进的排查体验。
**问题**:
1. 六个步骤平铺时，用户能看到事实但需要自己判断所处阶段、首要异常和下一步顺序。
2. 早期步骤的普通警告可能排在后续明确失败之前，导致“首要定位”和建议顺序互相矛盾。
3. 取消、跳过、阻断和失败的阶段语义不同，不能统一显示为成功或异常。
**解决**:
1. 前端将步骤归并为“路径判定、连接建立、服务响应”三阶段，阶段状态仅从既有 lifecycle/outcome 派生，不新增诊断事实源。
2. 运行中聚焦当前步骤；终态优先定位首个失败或阻断步骤，再处理 warning/unverified。顶部建议把焦点步骤对应项提前，其余建议保持原链路顺序。
3. 每个阶段保留完整步骤、原始错误、关联建议和折叠证据；cancelled 明确显示“未完成”，skipped 仍按适用性结果处理。
4. 后端终态结论优先带入真实错误消息；无错误对象时再按 blocked、warning、unverified 等状态生成明确文案。
**关键点**:
1. “排查引导”可以是报告事实的稳定派生视图，但不能在前端臆造根因或成功结论。
2. 首要定位、建议排序和步骤内建议必须引用同一组 evidenceId，避免展示口径分裂。
3. 阶段聚合必须保留 lifecycle 与 outcome 的独立语义，尤其不能把 cancelled 阶段标成“已通过”。
**涉及文件**:
- `apps/desktop/src/components/network/NetworkDiagnosisWorkspace.vue`
- `apps/desktop/src/utils/accessPathDiagnosticsView.ts`
- `apps/desktop/src/utils/accessPathDiagnosticsView.test.ts`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/runtime.rs`

**验证**:
- `pnpm --filter @lazycat/desktop exec vitest run src/utils/accessPathDiagnosticsView.test.ts src/components/NetworkPanel.test.ts`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml access_path_diagnostics -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-18: 访问链路诊断前端原位替换

**场景**: 保留工具 ID `network`，将旧连通性面板替换为分层访问链路诊断，并保留 PING/TCP/UDP 单项探测。
**问题**:
1. Tauri 事件可能早于 `diagnosis_start` 返回，事件也可能丢失、重复或晚到；只监听事件会漏掉首个快照，只轮询则无法及时展示步骤进度。
2. 步骤 lifecycle 与 outcome 是独立事实，前端合并成一个成功/失败标签会掩盖 blocked、skipped、cancelled 和 unverified。
3. 旧 UDP 无响应结果曾使用 `reachable: true`，不能继续展示为“可达”；旧历史目标也不能用简单冒号切分，否则破坏 IPv6。
**解决**:
1. 诊断工作区先注册监听再 start；start 返回前按 runId 缓存最高 sequence，返回后立即 get 恢复事实源，并用串行 setTimeout 轮询兜底。所有快照按 active runId 和单调 sequence 接收。
2. 步骤分别展示 lifecycle、outcome、原始错误和折叠证据；后端未提供 conclusions 时明确显示“步骤汇总”，不在前端伪造结论。
3. 父面板默认“链路诊断”，用 v-show 保留诊断和单项探测实例状态；单项探测继续读写旧收藏/历史 key，UDP 分成收到响应、明确不可达、无响应无法判断三态。
4. IPv6 socket 目标统一格式化为 `[IPv6]:port`，旧记录通过方括号规则和最后一个冒号解析；目录名称改为“访问链路诊断”，工具 ID 和 registry 保持不变。
**关键点**:
1. 长任务 UI 中事件是低延迟通知，get 快照是恢复来源；两条路径必须进入同一个 sequence 去重函数。
2. 取消后保留已完成步骤，卸载时取消仍在运行的任务并处理 listener Promise 与卸载竞态。
3. 兼容期应保留旧设置 key 和数据，不在 UI 替换任务中提前执行迁移；数据 schema 和脱敏导出交由独立迁移任务处理。
**涉及文件**:
- `apps/desktop/src/components/NetworkPanel.vue`
- `apps/desktop/src/components/network/NetworkDiagnosisWorkspace.vue`
- `apps/desktop/src/components/network/NetworkQuickProbe.vue`
- `apps/desktop/src/components/NetworkPanel.test.ts`
- `apps/desktop/src/utils/accessPathDiagnosticsView.ts`
- `apps/desktop/src/utils/accessPathDiagnosticsView.test.ts`
- `apps/desktop/src/composables/toolCatalog.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/components/NetworkPanel.test.ts src/utils/accessPathDiagnosticsView.test.ts src/utils/accessPathInput.test.ts src/utils/networkFavorites.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-18: 访问链路 TCP、TLS、HTTP 探测

**场景**: 在长任务运行时和只读环境适配器之后，接入真实 TCP、TLS、HTTP 与 HTTP 代理 CONNECT 探测，同时保留原 network action。
**问题**:
1. 直连、HTTP 代理访问 HTTP、HTTP 代理 CONNECT 访问 HTTPS 的实际连接对象不同，不能把“代理端口可达”写成“目标端口可达”。
2. TLS 需要分离连接 IP、SNI、证书校验名和信任来源；OpenSSL 本地链结果不能冒充 Windows 系统信任或在线吊销结论。
3. HTTP 必须限制重定向、响应头、响应体和整体耗时；TLS/CONNECT 失败时不能合成 HTTP 状态，同时不能泄露凭据或敏感查询参数。
**解决**:
1. TCP 对候选 A/AAAA 地址做最多 4 路并发探测，逐地址保留地址族、耗时、原始 OS 错误和错误分类；代理路径明确将 destinationRole 标为 proxy。
2. TLS 对直连地址逐个握手；代理路径先逐代理地址 CONNECT，再在隧道上握手。契约使用独立 `verifyHostname`，不从 SNI 或 HTTP Host 推导；证据分开记录 SNI、校验名、证书链、OpenSSL 链验证、Windows trust unsupported 和 revocation unverified。
3. HTTP 使用通用 transport connector 统一驱动明文 TCP 与 TLS stream，支持 HEAD、405/501 受限 GET、重定向和 HTTP 代理 absolute-form；每跳重新建立受限连接并保留连接证据。
4. 默认不发送 Cookie、Authorization、Proxy-Authorization 或请求体；请求保留实际 query，证据和 Location 对敏感 query 值脱敏。
5. 主链地址只来自 connectionIp、目标 IP 或 OS 有效解析；指定 DNS 仍是旁路对照。代理画像 unresolved、无候选地址、CONNECT/TLS 前置失败分别标为 blocked，不包装成失败或成功。明文正向代理无法可靠表达独立 connectionIp 时显式标记 unsupported。
**关键点**:
1. 无共享运行上下文时，各步骤独立重算同一代理画像并建立自己的连接；每步证据必须记录实际对端，不能根据前一步结果静态推断。
2. HTTPS 的 HTTP connector 只在主机名和 OpenSSL 链校验通过后发送请求；Windows 信任和吊销能力不足仍在 TLS 证据中显式保留。
3. 阻塞 OpenSSL/HTTP socket 操作放入 spawn_blocking，并按统一 deadline 设置阶段剩余 socket timeout；CancellationToken 在 DNS/TCP/CONNECT/TLS 后和 HTTP 写请求前检查，取消后不再发起后续请求，晚到结果也不能回写终态。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/probes/tcp.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/probes/tls.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/probes/http.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/probes/mod.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/runtime.rs`

**验证**:
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml access_path_diagnostics -- --nocapture`（68 通过）
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::network::tests -- --nocapture`（8 通过）
- `pnpm test src/utils/accessPathInput.test.ts`（13 通过）
- `pnpm typecheck`

**使用次数**: 0

## 2026-07-16: 高密度工作台分区与连续日志自动刷新

**场景**: 将请求转发右侧工作台拆为配置/观测两个任务型 Tab，并让运行日志复用既有状态轮询自动刷新，同时保持分页连续和已有数据可读。

**关键点**:
1. 配置和运行观测属于不同任务上下文，使用保持挂载的独立 Tab 与滚动容器；已有规则记住会话内 Tab，新建草稿强制进入配置，观测页不展示无关的保存操作。
2. 自动刷新不能把最新首页直接拼到旧分页尾部，否则突发新增会制造 offset 缺口。后台从 `offset = 0` 查询连续窗口，根据 `total` 增量扩展已加载深度，必要时补查，再整体替换连续前缀。
3. 复用既有串行 `setTimeout` 轮询，不新增计时器。日志请求、筛选防抖或观测写操作冲突时只保留一个带选择与筛选快照的待执行意图；终止轮完成或消费该意图后不重启状态轮询。
4. 后台刷新失败保留当前日志，并使用独立的非阻塞错误提示；首次或手动加载失败仍走替换式错误态，避免可用旧数据被隐藏。
5. 信息密度通过收紧区块间距、内边距和日志行节奏提升，不缩小正文可读基线，不修改全局 Element Plus 主题覆盖。

**涉及文件**:
- `apps/desktop/src/components/RequestForwardPanel.vue`
- `apps/desktop/src/components/request-forward/RequestForwardRuleForm.vue`
- `apps/desktop/src/components/request-forward/RequestForwardLogList.vue`
- `apps/desktop/src/utils/requestForward.ts`
- 对应 `RequestForwardPanel.test.ts` 与 `requestForward.test.ts`

**验证**:
- `pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts`（42 通过）
- `pnpm test`（57 个测试文件、584 个测试通过）
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- 未自动启动产品 UI，运行时视觉冒烟未执行

**使用次数**: 0

## 2026-07-18: 访问链路诊断契约与输入归一化

**场景**: 为原位替换 `network` 工具先建立前后端统一契约，不提前接入运行时或网络探测。
**问题**:
1. URL、域名、IPv4、IPv6 和带端口地址不能依赖简单冒号切分，裸 IPv6、`[IPv6]:port` 和 `host:port` 的语义不同。
2. 步骤执行状态与诊断结论混用时，`blocked/skipped/cancelled` 会被错误表达成探测失败或成功。
3. TypeScript 与 Rust 分别定义模型时，字段可选性、枚举值和 JSON 命名容易漂移。
**解决**:
1. TypeScript 纯函数统一输出协议、规范化主机、目标类型、端口、路径、URL、SNI、HTTP Host 和连接 IP；URL 默认使用 HTTPS，并显式校验覆盖项。
2. 生命周期固定为 `pending/running/completed/blocked/skipped/cancelled`，结果固定为 `success/warning/failed/unverified`，两者独立建模。
3. 报告使用 `schemaVersion = 1`，证据、结论和建议通过稳定 ID 引用；Rust serde 使用 `camelCase` 字段和 `snake_case` 枚举值，并用序列化测试锁定 wire format。
**关键点**:
1. IPv6 应交给结构化解析器校验和规范化；裸 IPv6 只表示地址，带端口时必须使用方括号。
2. SNI、HTTP Host 和连接 IP 是三个独立字段，不能从同一个字符串在探测阶段临时猜测。
3. 跨语言契约要同时对齐字段名、枚举值、可选性和空集合序列化行为，不能只对齐类型名称。
**涉及文件**:
- `apps/desktop/src/types/access-path-diagnostics.ts`
- `apps/desktop/src/types/index.ts`
- `apps/desktop/src/utils/accessPathInput.ts`
- `apps/desktop/src/utils/accessPathInput.test.ts`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/model.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/mod.rs`
- `apps/desktop/src-tauri/src/tools/mod.rs`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/accessPathInput.test.ts`
- `pnpm --filter @lazycat/desktop typecheck`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --bin lazycat-desktop access_path_diagnostics`

**使用次数**: 0

## 2026-07-18: 访问链路诊断长任务运行时

**场景**: 在真实代理、DNS、TCP、TLS、HTTP 探测器接入前，先建立支持进度、取消和快照恢复的诊断任务协议。
**问题**:
1. 通用 `tool_execute` 同步返回，无法承载逐步事件、整体取消和事件丢失后的最终状态恢复。
2. 用户取消、单步超时、整体超时和晚到结果并发发生时，旧结果可能覆盖终态或制造伪成功。
3. 窗口关闭、应用退出和大量已完成任务会让后台任务或快照注册表持续残留。
**解决**:
1. 新增独立 `diagnosis_start/get/cancel` Tauri command；start 只返回 runId，快照事件携带单调 sequence，get 返回注册表中的最新事实。
2. 每个 run 使用独立 `CancellationToken`、步骤状态和窗口所有权；状态先落快照再发事件，终态拒绝晚到步骤结果，重复取消保持幂等。
3. 单步和整体超时分别落为 `step_timeout` 与 `diagnosis_timeout`，保留已完成步骤；窗口关闭和应用退出主动取消并移除所属任务。
4. 活跃任务设置并发上限，终态快照按创建顺序保留固定数量；当前未接入的探测器显式返回 `skipped/capability_not_implemented`。
**关键点**:
1. 事件是通知通道，注册表快照才是恢复来源；事件发送失败不能回滚已落地状态。
2. 取消必须先使 run 进入终态，再丢弃底层晚到结果；不能只在前端停止 loading。
3. 任务所有权绑定启动窗口，get/cancel 不允许跨窗口访问同一个 runId。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/runtime.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/mod.rs`
- `apps/desktop/src-tauri/src/events.rs`
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src/bridge/events.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/types/access-path-diagnostics.ts`
- `apps/desktop/src/types/index.ts`

**验证**:
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --bin lazycat-desktop access_path_diagnostics -- --nocapture`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --bin lazycat-desktop tools::contract_tests::rust_events_are_declared_on_frontend -- --nocapture`
- `pnpm --filter @lazycat/desktop test src/utils/accessPathInput.test.ts`
- `pnpm --filter @lazycat/desktop typecheck`

**使用次数**: 0

## 2026-07-18: 访问链路诊断只读环境适配器

**场景**: 为访问链路诊断接入代理、Hosts 和 DNS 环境证据，同时保持系统配置只读并明确能力边界。
**问题**:
1. 不同客户端分别采用 WinINET、WinHTTP 或代理环境变量，不存在适用于所有应用的唯一代理结论；PAC/WPAD 未执行时也不能套用静态代理伪造决策。
2. 系统有效解析可能受 Hosts、缓存、NRPT、VPN 和 DoH 影响，指定 DNS 查询只能作为旁路对照，二者不能混成同一种 DNS 结果。
3. Hosts 和 DNS 旧能力会把部分错误压成空结果，无法区分无记录、读取失败、格式错误和服务器响应错误。
**解决**:
1. 代理适配器分别读取环境变量、当前用户 WinINET 和 WinHTTP 画像；支持域名/IP/端口/CIDR `NO_PROXY`，输出配置来源、命中规则和脱敏代理端点。WinINET 使用 `WinHttpGetIEProxyConfigForCurrentUser`，WinHTTP 使用 `WinHttpGetDefaultProxyConfiguration`。
2. PAC/WPAD 只报告 configured + unsupported；运行请求支持 `auto/environment/windows_user/winhttp/direct` 画像选择，强制直连也写入显式证据。
3. Hosts 适配器复用现有系统路径，报告目标命中、重复、多地址、IPv4/IPv6 混合、注释记录、相关注释和不可见字符/格式问题。
4. DNS 适配器用 OS `getaddrinfo` 表达系统有效解析，用 Hickory 对指定服务器旁路查询 A/AAAA/CNAME/PTR，并区分 `no_records/nxdomain/servfail/refused/timeout/transport_error/malformed_response`。
5. Windows DNS 缓存、NRPT、DoH 明确标记 unsupported；三类适配器结果统一写入步骤 evidence，不把能力缺失表达成成功。
**关键点**:
1. 代理凭据、PAC 查询参数和 fragment 在进入 evidence 前必须脱敏，不能依赖报告导出阶段再补救。
2. A 成功但 AAAA/CNAME 无记录是正常 DNS 组合，不能仅因可选类型无记录把整步标成警告。
3. IP 目标未指定旁路 DNS 时应标记 DNS 不适用；指定服务器后才执行 PTR 对照查询。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/adapters/proxy.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/adapters/hosts.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/adapters/dns.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/adapters/mod.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/runtime.rs`
- `apps/desktop/src-tauri/src/tools/hosts.rs`
- `apps/desktop/src/types/access-path-diagnostics.ts`
- `apps/desktop/src/types/index.ts`

**验证**:
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --bin lazycat-desktop access_path_diagnostics -- --nocapture`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --bin lazycat-desktop tools::hosts::tests -- --nocapture`
- `pnpm --filter @lazycat/desktop test src/utils/accessPathInput.test.ts`
- `pnpm --filter @lazycat/desktop typecheck`

**使用次数**: 0

## 2026-07-17: Windows 发布脚本默认只构建 lite portable

**场景**: 日常 Windows 打包通常只需要不内置 WebView2 的绿色包，但原发布脚本每次都会构建 lite/full 安装包与绿色包共四个产物。

**问题**:
1. lite portable 只依赖 release 运行文件和资源，却被迫先执行带 fixed WebView2 的 NSIS 完整构建，耗时和磁盘开销明显偏高。
2. 哈希、缺失检查和 GitHub 上传写死四个产物，无法让默认模式只处理本次实际选择的包。
3. 单独复制一份 lite 脚本会重复版本校验、Git/tag、压缩和上传逻辑，后续容易分叉。

**解决**:
1. `release-all-win.ps1` 新增 `-AllPackages`；默认使用 `tauri build --no-bundle` 并只生成 lite portable，传开关时完整保留原四包流程。
2. 用模式对应的 `$artifacts` 集合统一驱动缺失检查、SHA256 和 GitHub Release 上传；`-SkipBuild` 也只校验当前模式应有产物。
3. 根命令新增 `release:win` 作为默认入口，`release:all:win` 显式传 `-AllPackages`，让命令名和行为保持一致。

**关键点**:
1. 模式专属前置条件应延迟到对应分支；默认 lite portable 不应检查固定版 WebView2 或初始化 full stage。
2. 构建产物集合必须成为哈希和上传的唯一来源，不能扫描输出目录，否则历史残留文件可能被误传。

**涉及文件**:
- `scripts/release-all-win.ps1`
- `package.json`
- `README.md`
- `AGENTS.md`
- `CLAUDE.md`

**验证**:
- PowerShell AST 语法解析
- Tauri CLI `build --help` 确认支持 `--no-bundle`
- package scripts 断言与模式分支静态检查
- `pnpm release:win -- -Tag v0.7.1 -SkipBuild -SkipUpload`（SHA 仅包含 lite portable）
- `pnpm release:all:win -- -Tag v0.7.1 -SkipBuild -SkipUpload`（SHA 包含原四个产物）

**使用次数**: 0

## 2026-07-16: 长生命周期网络服务的运行态与观测一致性

**场景**: 在同步 Tauri tool dispatch 中承载 HTTP / TCP / UDP 长生命周期服务，同时提供自启动、退出清理、统计日志和可编辑前端面板。

**关键点**:
1. 同步 IPC 不等于每次调用临时创建异步 runtime；由进程级 manager 持有专用 Tokio runtime、规则句柄和逐规则操作锁，启动/停止/查询都围绕同一份运行态真值串行化。
2. `autoStart` 是持久化意图，`running/failed/stopped` 是运行时事实，不能互相冒充。跨持久化与运行时的启动/停止要有反向补偿；应用退出先关闭生命周期闸门，再逐条停服务，但不改写用户的自启动意图。
3. 观测必须有界且不能反向拖慢转发：流式 preview tap 只复制上限内字节并继续透传原 chunk，日志用容量/保留上限控制；落库失败独立暴露，不把仍正常的网络转发伪装为失败。
4. UDP 按客户端隔离 session；session 汇总落库前先进入 finalize gate，并等待已登记的在途 I/O 记账退出，避免结束摘要被迟到的成功回包越过或重复计数。
5. 自动轮询、筛选防抖和详情切换同时存在时，响应写回需校验请求 token、选择意图与参数快照；轮询不得覆盖 dirty 表单，清空/重置等观测变更与在途轮询互斥。
6. 启动网络监听前先解析下游全部地址并做自转发检查；wildcard 监听只覆盖同地址族的 loopback/本机接口，不能用“同端口”粗暴拒绝所有远端目标。协议本身的显式拒绝（如暂不支持 HTTPS）应早于 DNS 预检。
7. runner 的 stop 合约应是“消费句柄并终止任务，即使清理返回错误”；manager 收到 stop error 后必须进入 `failed` 且不保留 handle，用户停止仍持久化关闭自启动意图，应用退出则不修改该意图。
8. HTTP 响应头已发送后 body 流失败必须在当前 request trace 上幂等完成，保留请求 metadata、状态码和已传输字节；不要同时追加匿名流失败事件和成功完成事件。
9. 前端写操作期间统一锁定规则选择与表单；异步 mutation 捕获目标 ID 和选择意图，晚响应只在意图仍一致时更新选择/表单。后台发现当前 dirty 规则已删除时，应显式结束旧编辑上下文并同步新选择。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/request_forward/`
- `apps/desktop/src/components/RequestForwardPanel.vue`
- `apps/desktop/src/utils/requestForward.ts`

**验证**:
- `cargo test request_forward -- --nocapture`（80 通过，含 DNS/wildcard 自转发、stop 错误终态、HTTP body 流失败、HTTP 流式/SSE、HTTPS 显式拒绝、TCP half-close、UDP 双客户端隔离）
- `cargo test contract_tests -- --nocapture`
- `cargo test`
- `pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts`
- `pnpm test`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-16: 完整移除抓包工具

**场景**: 从桌面应用中完整移除依赖 Npcap/PCAP 的抓包工具，同时保留名称相近但职责无关的快速捕获、收纳箱剪贴板采集和壁纸 CapturePreview。
**问题**:
1. 抓包工具跨越前端目录、异步组件注册、Tauri 独立 command、Rust 工具模块和 Cargo feature，只有隐藏菜单会留下不可达死代码与依赖。
2. `capture` 在仓库中还用于 QuickCapture、Inbox clipboard capture 和 wallpaper capture，不能按关键词批量删除。
3. README、双规范文档和旧实施计划仍引用抓包入口或已删除组件，源码删除后会形成失效文档。
**解决**:
1. 先给 `toolCatalog` 增加退役工具回归测试，确认测试因 `capture` 仍存在而失败，再移除目录项和组件注册使其通过。
2. 删除 `CapturePanel.vue`、`tools/capture.rs`、6 个 Tauri command 及注册，并移除 `pcap`、`etherparse`、`libc` 可选依赖和 `capture` feature；同步更新 Cargo.lock。
3. 清理组件声明、README、旧计划引用，并同步更新 `AGENTS.md` / `CLAUDE.md` 的章节编号和新增工具说明。
**关键点**:
1. 完整删除工具按“目录入口 -> 组件注册 -> IPC/command -> 后端模块 -> 依赖与锁文件 -> 文档”逐层闭环扫描。
2. 对高频通用词先建立精确标识集合（`CapturePanel`、Npcap、PCAP command、Cargo feature），避免误删同名但不同域的功能。
3. 依赖移除后必须运行 Cargo 检查，让锁文件同步裁剪仅由该功能引入的传递依赖。
**涉及文件**:
- `apps/desktop/src/composables/toolCatalog.ts`
- `apps/desktop/src/tool-registry.ts`
- `apps/desktop/src/components/CapturePanel.vue`
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src-tauri/src/tools/capture.rs`
- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/src-tauri/Cargo.lock`
- `README.md`、`AGENTS.md`、`CLAUDE.md`

**验证**:
- `pnpm test src/composables/toolCatalog.test.ts`（2 通过）
- `pnpm test`（57 个测试文件、579 个测试通过）
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-14: SQL 实体生成器基类字段排除

**场景**: Java 实体生成需要从多个基类模板汇总字段排除，同时只生成一个合法父类继承声明。

**关键点**:
1. 先把 SQL 列转换成最终 Java 属性名，再与基类字段集合精确匹配；不要混用数据库列名和属性名。
2. 字段过滤必须发生在类型 import、MyBatis-Plus 注解和字段正文生成之前，所有派生输出共享同一个过滤后集合，避免残留无用 import 或注解。
3. Java 多个“基类配置”应拆成一个实际父类和多个字段模板；只有实际父类生成 `extends/import`，其余模板只提供排除字段。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/sql_entity.rs`
- `apps/desktop/src-tauri/src/tools/convert.rs`
- `apps/desktop/src/components/SqlEntityPanel.vue`
- `apps/desktop/src/components/SqlEntityBaseClassDialog.vue`
- `apps/desktop/src/utils/sqlEntityBaseClass.ts`

**验证**:
- `cargo test sql_entity:: -- --nocapture`
- `cargo test sql_to_entity_ -- --nocapture`
- `cargo test contract_tests -- --nocapture`
- `pnpm test src/utils/sqlEntityBaseClass.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-11: 结构治理批次 3（Todo 域）行为保持拆分

**场景**: 执行结构治理路线图批次 3：Todo 前端域迁入 `components/todo/`、`TodoPanel.vue`（2961 行）拆 5 个 composable、Rust `todo.rs`（3253 行）目录化。plan `docs/superpowers/plans/2026-07-11-structure-refactor-batch3-todo-plan.md`。
**使用次数**: 0
**结果**: TodoPanel.vue 2961 → 1878 行；todo/mod.rs 3253 → 619 行 + 7 子模块（types/helpers/recurrence/reminders/taxonomy/items/pm_link）；22 个 Rust 用例、634 前端用例、e2e 全程保持通过，共 14 个提交。
**问题与关键点**:
1. **`.vue` 迁 `.ts` 会暴露从未被检查的类型问题**：本项目 typecheck 是纯 tsc、不覆盖 `.vue` script。搬进 composable 后新暴露两处：`itemDraft.kind` 从未声明（运行时恒 undefined，两处 `!== "recurring"` 判断恒真）——在 `TodoItemDraft` 接口上声明 `kind?: TodoKind` 保持运行时行为不变；`todoLinkedPmItem.projectId` 实际可空——ref 类型放宽为 `number | null`。均为零运行时改动的类型适配，diff 中显式可见。
2. **源码断言测试随代码走**：TodoPanel 的 3 个组件测试盯具体函数源码。quick-add/title-enter 盯的代码（quickAddContext、onQuickAddCreated、onTitleEnter、saveItem 胶水）刻意留壳层实现零测试改动；edit-focus 盯的 enterEditMode 随编辑器状态机迁出，测试同步增加对 `useTodoDetailState.ts` 的 readFileSync 读取，断言内容不变。
3. **共享 `let` 标志跨 composable 需 ref 化**：`skipProjectWatch` 原是壳层 `let`，watch 移入 useTodoPmLink 后改为 composable 内 ref 返回，壳层引用点 `.value` 化（本批唯一预期的非纯移动适配）。
4. **Rust 目录化用 `mod x; use x::*;` 私有 glob 胶水**（批次 2 同款）：函数体零改动，仅 `fn` → `pub(crate) fn` + 子模块显式 `use super::helpers::*` 等。**偏差**：`mod tests` 未按 plan 拆到子模块而是整体留在 mod.rs——测试共享 80+ 行 `create_test_conn` 建表夹具和 seed 系列夹具，拆分收益小于夹具共享成本；私有 use 导入对子模块可见，`use super::*` 让测试零改动，测试路径与基线完全一致便于对账。
5. **只被测试使用的顶部导入会被 cargo check 误报 unused**：删除后 `cargo test` 编译才炸（Timelike/DateTime/params!/json!）。处理：移进 `mod tests` 内部 use。每步必须 `cargo check`（零警告）+ `cargo test todo`（用例数对账）双跑。
6. **子模块内 `super::` 指向变化**：原 mod.rs 里 `super::attachments`（= tools::attachments）搬进子模块后要改 `crate::tools::attachments`；本地模块与全局 helper 同名时要显式核对解析目标。
7. sed 行区间删除比长字符串 Edit 稳，但边界必须先 `sed -n` 逐块核对——本批一次多删一行（formatDate 函数头）由 build:web 立即暴露；每步验证兜底有效。
**涉及文件**: `components/todo/`（14 文件迁移）、`composables/useTodo{ItemFilters,ScheduleFields,CrudActions,PmLink,DetailState}.ts`、`src-tauri/src/tools/todo/`（8 文件）
**验证**: 每步 `pnpm typecheck` + `build:web` + `pnpm test`（634）/ `cargo check` 零警告 + `cargo test todo`（22 用例对账）；收尾 `cargo test` 全量 467、`pnpm test:e2e` 2 passed。cargo 全量偶发 1 个计时敏感用例失败（api_mock delay 类），复跑即绿。
**遗留改良点（只记不做）**: TodoPanel 死代码 `initialCreateSchedule`、`disabledFiveMinuteMinutes/disabledAllSeconds`、`snoozeItem` 无调用方；`itemDraft.kind` 恒 undefined 的两处判断语义待确认。

## 2026-07-09: API Mock 响应体格式化扩展到全部文本语言（复用 @lazycat/formatters）

**场景**: API Mock 路由表单中响应内容（文本类型）原本只有"格式化 JSON"按钮（`JSON.parse` + `JSON.stringify` 实现），需要扩展为对编辑器可识别的全部语言（JSON/XML/HTML/CSS/JavaScript）提供格式化。
**解决**:
1. 复用 `@lazycat/formatters` 包（格式化工具面板 `FormatterPanel.vue` 已在用），不在组件里重写格式化逻辑。
2. 包内缺 CSS/JS 格式化，新增 `formatCss`（`prettier/plugins/postcss`）和 `formatJavaScript`（复用已有 babel/estree 插件），零新增 npm 依赖——prettier standalone 的插件全部随主包分发。
3. JSON 从 `JSON.parse/stringify` 改为 Prettier `formatJson`：基于 AST 文本重排，不丢大整数精度（`JSON.parse` 会把超精度整数转 double）。
4. 组件内用 `Record<language, formatter>` 映射表分发；按钮文案/错误提示的语言标签抽为 `getMockBodyFormatLabel` 纯函数放 `utils/apiMock.ts` 并配套单测。
**关键点**:
1. `@lazycat/formatters` 是 workspace 包且 `main` 指向 `dist/index.js`，修改其 `src` 后必须先 `pnpm --filter @lazycat/formatters build` 重新生成 dist，否则 desktop 的 typecheck/构建/运行用的仍是旧产物。
2. prettier standalone 插件路径：`prettier/plugins/postcss`（css/less/scss）、`prettier/plugins/babel` + `prettier/plugins/estree`（JS/JSON，estree 是 printer 必须一起传）。
**涉及文件**:
- `packages/formatters/src/index.ts`
- `apps/desktop/src/utils/apiMock.ts`
- `apps/desktop/src/utils/apiMock.test.ts`
- `apps/desktop/src/components/api-mock/ApiMockRouteForm.vue`

**验证**:
- `pnpm --filter @lazycat/formatters build`
- `pnpm vitest run src/utils/apiMock.test.ts`（27 通过）
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-08: 契约测试专用接口未标 #[cfg(test)] 导致 44 个 dead_code 警告

**场景**: `cargo run` 报 44 个警告，绝大多数是 40 个工具模块的 `supported_actions()`、`mod.rs` 聚合函数与 `events.rs::ALL` "never used"。这些是 2026-07-07 横切面治理 X1-X4 引入的契约对账清单，实际只被 `#[cfg(test)] mod contract_tests` 引用，生产编译剖面看不到使用者。
**使用次数**: 0
**问题**:
1. 测试专用的契约接口（`supported_actions` × 40 + `mod.rs` 聚合 + `events::ALL`）未标 `#[cfg(test)]`，`cargo build/run` 下触发 dead_code 警告共 42 个。
2. `pm_siyuan.rs` 的 `build_siyuan_target_hpath` 为 `pub(crate)`，但参数类型 `SiyuanLocation` 仅 `pub(super)`，触发 private_interfaces 警告。
3. `widget/session.rs` 的 `read_inner` 是"替代旧 state::snapshot"的重构遗留，全仓库无调用方。
4. 附带：`widget/pulse.rs` 测试里残留 `let start = Instant::now();`（测试改写后未删），仅在 `--tests` 剖面报警。
**解决**:
1. 给全部 42 处契约接口加 `#[cfg(test)]`（40 个模块用 Python 按精确行匹配批量插入，UTF-8 读写；函数签名 40 处完全一致且前面均为空行，无 doc 注释干扰）。
2. `build_siyuan_target_hpath` 降为 `pub(super)`——所有调用方（模块内部 + `pm.rs` 测试）都在 `crate::tools` 内。
3. 删除 `read_inner` 与 pulse 测试残留变量（连带删除仅其使用的 `use std::time::Instant`）。
**关键点**:
1. 只被 `#[cfg(test)]` 代码引用的项必须同样标 `#[cfg(test)]`，否则每次生产编译都报 dead_code；新增工具域时，新模块的 `supported_actions()` 要带上 `#[cfg(test)]`（与 `contract_tests.rs` 的 SYNC_GUIDE 同步语义）。
2. 代价是这些函数只在 `cargo test` / `cargo check --tests` 时编译；验证契约清单需跑 test 剖面，仅 `cargo check` 不覆盖。
3. 消警告前先定性：本批 42/44 是"测试基础设施被误判死代码"，删除会破坏契约安全网；真正的死代码只有 2 处。
**涉及文件**:
- `src-tauri/src/tools/*.rs`（40 个模块 + `mod.rs`）、`src-tauri/src/events.rs`
- `src-tauri/src/tools/pm_siyuan.rs`、`src-tauri/src/tools/widget/session.rs`、`src-tauri/src/tools/widget/pulse.rs`
**验证**:
- `cargo check` 与 `cargo check --tests` 均 0 警告
- `cargo test` 461 通过 0 失败（契约对账测试全绿）

## 2026-07-08: Tauri 窗口缺失 capabilities 白名单导致 Spotlight 事件静默失效

**场景**: 浏览器身份设置别名后，Spotlight 用别名搜索不到；昨晚已实现的 `browser-profiles-changed` 事件刷新机制（2026-07-07 记录）在运行时完全不生效。
**使用次数**: 0
**问题**:
1. `capabilities/default.json` 的 `windows` 白名单缺少 `spotlight` 和 `pomodoro-prompt`，这两个窗口的 `@tauri-apps/api/event` `listen`/`emit` 调用全部被 Tauri ACL 拒绝。
2. Spotlight 三条刷新链路全部依赖 event 插件且全部包了 catch 静默吞错：`SPOTLIGHT_RESET`（唤出重置 + 重新预取）、`browser-profiles-changed`（别名变更局部刷新）、`spotlight-config-changed`（配置跨窗口同步）。运行时 Spotlight 缓存永远停留在应用启动时的快照，别名设置后直到整个应用重启才可搜索。
3. 症状被两层机制掩盖：自定义 command（`tool_execute` 等）不受 ACL 管控所以挂载时首轮预取正常，数据"看起来有"；`focusInput` 的 `el.select()` 全选旧查询词，首次按键即覆盖，掩盖了重置事件失效的可见痕迹。
4. 同类前科：2026-05-10 `76be630` widget 窗口因同一原因数据加载失败，当时只补了 `widget`，未排查其余窗口。
**解决**:
1. `capabilities/default.json` 的 `windows` 数组补齐 `spotlight` 与 `pomodoro-prompt`，与 `main.rs` 中全部 `WebviewWindowBuilder` label + 主窗口对齐。
2. `cargo check` 验证 tauri-build 对 capability 文件的构建期校验通过。
**关键点**:
1. 新增 Tauri 窗口时必须同步把 label 加进 `capabilities/default.json` 的 `windows`，否则该窗口所有插件命令（event/dialog/notification 等）静默失败；自定义 command 不受影响，容易造成"部分功能正常"的假象。（已第 2 次踩坑，下次 +1 后固化到 CLAUDE.md）
2. 跨窗口事件链路排查顺序：先查 capabilities 白名单，再查事件名一致性，最后查监听回调逻辑；`listen()` 被 ACL 拒绝时 promise reject，会被"best-effort"式 catch 吞掉。
3. 排查时前端匹配逻辑、后端存储、事件封装层层验证都正常但运行时不工作，应怀疑权限/环境层而非代码逻辑层；本地可直接读 `%USERPROFILE%\.lazycat\config.json` 指针与数据目录 sqlite 验证落库事实。
**涉及文件**:
- `apps/desktop/src-tauri/capabilities/default.json`
**验证**:
- `cargo check`（tauri-build 构建期校验 capability 文件）
- 手工冒烟待用户执行：重启应用 → 修改别名 → 唤出 Spotlight 搜索别名

## 2026-07-07: IPC 契约对账与横切面治理 X1-X4

**场景**: 执行《IPC 契约对账与横切面治理》：为前后端 action、应用级事件名建立对账安全网，并在 Snippet / Launcher / Hosts / Dns 四个试点面板收敛 IPC 错误反馈与列表搜索防抖写法。
**使用次数**: 0
**问题**:
1. `src/bridge/tauri.ts`、Rust `tools/mod.rs` 和挂件刷新白名单是三份手工清单，已出现 todo 白名单幽灵 action，运行前缺少漂移报警。
2. 应用级事件名散落在 Rust emit 和前端 listen/emit 字符串里，常规 grep 容易漏掉换行形态事件。
3. 面板 IPC 错误反馈长期存在裸 `invokeToolByChannel` + 手写 try/catch、局部 ipc 包装和静默 catch 混用；搜索防抖也有多套手写 timer。
**解决**:
1. Rust 工具模块显式暴露 `supported_actions()`，`tools/mod.rs` 聚合域 action；契约测试逐行解析 `tauri.ts` 的 `CHANNEL_MAP`，用 `>=300` 哨兵断言防止格式变更后假绿，并双向检查前后端 action 与挂件白名单。
2. 前端新增 `bridge/events.ts`、Rust 新增 `events.rs`，Rust `ALL` 只引用具名常量；契约测试解析前端 `APP_EVENTS`，断言 `events::ALL ⊆ APP_EVENTS`，同时清理无 listener 的 `pomodoro-prompt-refresh` emit。
3. `useToolInvoke` 增加 `errorPrefix` 与 `invokeSilent`；用户触发操作走 `invokeWithLoading(..., { errorPrefix })`，后台可降级读取走 `invokeSilent` 并写明静默原因。
4. 新增 `useDebouncedKeyword` / `useListSearch` 分层双 API：后端搜索面板只用关键字防抖，本地过滤面板复用同一防抖层再叠加 matcher。Snippet 搜索 260ms -> 300ms、Hosts 搜索新增 300ms 防抖是本批明确允许的微变。
5. 试点面板按“行为清单 → 改造 → 验证 → 提交”执行；Snippet 用 `useDebouncedKeyword` 触发后端搜索，Launcher / Hosts 用 `useListSearch` 本地过滤，Dns 无搜索框只收敛错误反馈。
**关键点**:
1. 解析式契约测试必须配哨兵数量断言和文件头格式注释，否则格式改动后可能解析为空但测试仍通过。
2. 事件常量双侧维护时，Rust `ALL` 必须由具名常量组成，避免“常量定义一份、测试集合再手写一份”的双写漂移。
3. 面板收敛样板优先使用多实例解构 loading：`const { loading: saving, invokeWithLoading: invokeSave } = useToolInvoke()`；后台统计、预加载和一致性检查失败不应打扰用户时用 `invokeSilent` 并注释原因。
4. `useToolInvoke` 当前仍是 boolean loading，并发调用会互相覆盖；本批只记录观察点，后续如遇同一实例并发请求再改为计数式 loading。
5. 涉及产品 UI 的手工冒烟仍需与用户协调运行应用；本批自动执行了 e2e 冒烟，未擅自启动桌面 UI。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/*.rs`
- `apps/desktop/src-tauri/src/tools/contract_tests.rs`
- `apps/desktop/src-tauri/src/events.rs`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/bridge/events.ts`
- `apps/desktop/src/composables/useToolInvoke.ts`
- `apps/desktop/src/composables/useListSearch.ts`
- `apps/desktop/src/components/{SnippetPanel,LauncherPanel,HostsPanel,DnsPanel}.vue`

**验证**:
- `cargo test data_dictionary -- --nocapture`
- `cargo test`
- `pnpm --filter @lazycat/desktop test src/composables/useToolInvoke.test.ts`
- `pnpm --filter @lazycat/desktop test src/composables/useListSearch.test.ts`
- `pnpm --filter @lazycat/desktop test`
- `pnpm typecheck`
- `pnpm test:e2e`
- `pnpm --filter @lazycat/desktop build:web`

**完成定义勾稽**:
1. 三份 action 清单与两侧事件常量已纳入 `cargo test` 对账，todo 幽灵词条清零。
2. `src/bridge/events.ts` 与 `src-tauri/src/events.rs` 已建立，应用级事件名裸字面量清理到常量定义处。
3. 四个试点面板已成为错误反馈与搜索防抖样板，规范经验已沉淀在本条记录中。

## 2026-07-07: Spotlight 预取缓存变更用 provider 级事件失效

**场景**: 浏览器身份别名保存后，Spotlight 驻留窗口仍显示旧别名，导致新别名搜不到、旧别名还能命中。
**使用次数**: 1（2026-07-08 排查同一问题时引用，发现该机制因 capabilities 白名单缺失未生效，见 2026-07-08 记录）
**问题**:
1. 浏览器身份的搜索字段构建逻辑已正确复用别名，但 Spotlight 已打开窗口持有 `prefetchAll()` 的 provider 缓存，不会因为浏览器身份面板保存配置自动重建。
2. 局部刷新和全量 `prefetchAll()` 都会写同一个 `browser-profiles` provider 结果，异步竞态下旧全量请求可能覆盖新局部结果。
3. 从 Spotlight 启动身份会更新使用统计，若不刷新缓存，空输入结果的排序权重仍可能滞后。
**解决**:
1. 增加 `browser-profiles-changed` Tauri 事件封装，浏览器身份别名、隐藏状态、Edge 路径和 Spotlight 启动成功后统一通知变更。
2. Spotlight 监听该事件后只重新 `prefetch` 浏览器身份 provider，并替换该 provider 的结果，不清空其他 provider 缓存。
3. 为浏览器身份 provider 写入增加统一版本号守卫，阻止过期的局部刷新或全量预取回写。
**关键点**:
1. provider item 的搜索字段正确，不代表驻留 Spotlight 缓存会自动更新；跨窗口状态变更要显式失效。
2. provider 级局部刷新应和全量预取共享同一个写入版本，而不是各自维护独立锁。
3. 会影响排序权重的默认动作也属于缓存变更源，不能只监听配置编辑路径。
**涉及文件**:
- `apps/desktop/src/spotlight/browser-profiles-events.ts`
- `apps/desktop/src/spotlight/browser-profiles-refresh.ts`
- `apps/desktop/src/components/SpotlightPanel.vue`
- `apps/desktop/src/components/BrowserProfilesPanel.vue`
- `apps/desktop/src/spotlight/providers/browser-profiles.ts`
**验证**:
- `pnpm test src/utils/browserProfiles.test.ts src/spotlight/providers/browser-profiles.test.ts src/spotlight/browser-profiles-events.test.ts src/spotlight/browser-profiles-refresh.test.ts src/spotlight/search.test.ts src/utils/spotlight-active-index.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-07-04: 任务清单快速添加栏（输入行 + 日期/优先级内联速选）

**场景**: 按 `docs/superpowers/plans/2026-07-04-todo-quick-add-plan.md` 4 阶段 TDD 实施：列表视图工具栏下方常驻快速添加栏，回车即建、焦点保留连续录入，继承筛选上下文（分类/项目/优先级默认值），后端零改动复用 `tool:todo:item-create`。
**使用次数**: 0
**问题**:
1. plan 描述的组件行为测试（mock bridge + 触发回车）在本项目不可执行：无 `@vue/test-utils`、无 DOM 环境、vitest 配置未挂 vue 插件（.vue 文件无法在测试中 import）。
2. "今天"取下一个 5 分钟刻度需要日期进位（23:58 → 次日 00:00），但既有 `getCreateDraftDefaultDateTime` 的 `% 1440` 分钟回绕不带日期进位（其日期恒为"明天"故无此问题），不能直接复用取整段。
3. 优先级速选需要"未手动选择时跟随筛选默认值、手动选择后不再跟随"，两者若共用一个状态，筛选变化会覆盖用户手动值。
4. 新任务可能被关键词搜索或手动覆盖的优先级筛选排除在可见列表外，高亮会静默失效让用户以为没建上。
5. "选日期…"从 el-dropdown 菜单项触发 el-date-picker 弹层，dropdown 关闭与 picker 打开有时序竞争。
**解决**:
1. 遵循项目既有约定：组件/面板测试用源码断言式（readFileSync + 断言关键守卫存在，plan 引用的 `TodoPanel.edit-focus.test.ts` 正是此模式），行为级覆盖全部下沉到纯函数 `buildQuickAddPayload` 的 11 个真实单测（时间取整、午夜/跨月进位、继承降级、字段裁剪、5 分钟对齐）。
2. "今天"用 `new Date(y, m, d, h, floor(min/5)*5 + 5)` 构造，分钟溢出由 Date 构造器自动进位跨日；"明天"/指定日期走 `combineLocalDateTime(date, DEFAULT_TIME)`（`DEFAULT_TIME` 从 todoSchedule 做最小导出）。
3. `priorityOverride`（手动值）与 `context.priorityDefault`（自动默认）独立建模：显示用 `priorityOverride ?? priorityDefault`，无任何 watch 反写，仅 Esc/手动清除置 null；着色区分（灰=默认跟随，彩=显式指定）。
4. 创建成功后 `await loadItems()` 再按最终渲染口径 `displayActiveItems`（关键词过滤 → 分桶 → 优先级/分类筛选之后）判可见性：可见则卡片高亮 1.5s 渐隐（连续录入后一次 clearTimeout 覆盖前一次），不可见则 `ElMessage.info` 兜底提示。
5. 隐藏 el-date-picker 放 0 尺寸锚点容器，dropdown command 里 `nextTick(() => datePickerRef.handleOpen())` 等关闭流程结束再开弹层（同数据字典 Dropdown→Dialog 时序经验）。
**关键点**:
1. in-flight 守卫直接复用 `useToolInvoke().loading`（`if (loading.value) return`），不另设 submitting 标志；失败提示由 `invokeWithLoading` 内置 `ElMessage.error` 承担，组件不重复 toast、不清空输入。
2. IME 守卫沿用面板模式：`@keydown.enter.exact.prevent` + `if (event.isComposing) return`。
3. 成功路径只清标题（保留日期/优先级选值便于连续录入同类任务），`focus()` 保持焦点；Esc 才全量重置——"清空"与"重置"是两个语义。
4. 卡片高亮用 `animation ... both` 会覆盖 `.todo-card` 原有 slideIn 入场动画，属预期（高亮即入场反馈）；定时器在 `onBeforeUnmount` 清理。
**涉及文件**:
- `apps/desktop/src/utils/todoQuickAdd.ts`（新增）+ `todoQuickAdd.test.ts`；`todoSchedule.ts`（DEFAULT_TIME 导出）
- `apps/desktop/src/components/TodoQuickAddBar.vue`（新增）+ `TodoQuickAddBar.test.ts`
- `apps/desktop/src/components/TodoPanel.vue`（挂载/上下文/反馈/高亮样式）+ `TodoPanel.quick-add.test.ts`
**验证**:
- `pnpm test`（574 个，全部通过）
- `pnpm typecheck`、`pnpm --filter @lazycat/desktop build:web`

## 2026-07-04: JsonTreeViewer 查看+编辑扩展（搜索定位/复制菜单/树内编辑/撤销重做/三消费方接入）

**场景**: 按 `docs/superpowers/specs/2026-07-04-json-tree-viewer-extensions-plan.md` 把只读 JSON 树升级为通用查看+编辑组件,并接入 JSON 处理面板（文本/树双模式编辑）、JWT 解码、CSV 输出三个消费方;数据字典零改动获得查看增强。
**使用次数**: 0
**问题**:
1. 受控组件的"编辑回流"与"外部换文档"如何区分:消费方以 `ref` 持有文档时回传的 `value` 是 reactive Proxy,直接 `===` 比较最近 emit 的根会把每次编辑误判为换文档,展开状态与 undo 栈每次编辑后被清空。
2. 撤销/重做需要同时恢复文档与展开状态,且展开 key 在数组 insert/remove/move、rename 后会整体位移。
3. 同一节点 key 与 value 可各自命中搜索,高亮不能只按节点 key 粒度做(key 命中会连带值一起高亮)。
4. JSON 处理面板经 `<component :is>` 切换即卸载(无 keep-alive),树内编辑不能等切回文本才序列化,否则丢编辑。
5. 面板 `watchPendingInput` 剪贴板注入会在树模式下外部覆写 `input`,需要与自身回写区分。
**解决**:
1. `useJsonTreeEditing` 记录 `lastEmittedRaw`,`onValueChange` 用 `toRaw(newValue)` 与之比较——相等为编辑回流(保持展开/undo/搜索),不等为外部换文档(清双栈、取消编辑态、回调组件重置展开+清搜索)。组件内部树构建/patch 一律基于 `toRaw(props.value)`,避免 raw/proxy 混合子树破坏结构共享。
2. 历史栈直接存快照 `{ value, expandedKeys }`(不可变 patch 引擎产出新根、旧根结构共享,快照零拷贝成本),undo/redo 恢复快照不再做迁移;展开迁移 `migrateExpandedKeys` 只在 op 成功时执行一次,对 encode 后的 key 字符串做前缀替换/索引平移(`i:` 段只动 base 直属子层)。空 key 插入取消=弹出 past 栈顶且不入 future(被弃状态不产生 redo 项)。
3. 命中标识用 `${field}:${nodeKey}`(`jsonTreeSearchMatchId`)贯穿 composable → viewer → node,label 与 value 各自独立高亮;编辑中的行抑制高亮。
4. 树内每次 `update:value` 立即 `JSON.stringify(v, null, 2)` 回写 `input`,文本任何时刻都是事实源;文本类操作(格式化/转换)执行前仅切回文本模式、不再重写。
5. 记录最近回写文本 `lastWrittenBack`,`watch(input)` 中不相符的变化按换文档处理:重过闸门(`canEnterJsonTree`,先查 1MB 体积再 parse),成功刷新树、失败退回文本模式并提示。
**关键点**:
1. Vue 受控组件回流判定必须 `toRaw` 比较,消费方文档建议 `shallowRef` 持有(组件文档注释写明契约:原样回写、不得 clone)。
2. 结构共享的不可变更新使快照式 undo 可行:只沿目标路径浅克隆祖先,历史栈存整根引用,无需逆操作模型。
3. 行内编辑"Enter 提交、Esc/失焦取消"配合编辑态单一来源(`editingState.key`):成功才 `endEdit`,失败(如重名)保留编辑态让用户修正;浏览器移除聚焦元素不派发 blur,残余 blur 由 `cancelEditing` 的 null 守卫兜底。
4. 值输入宽松解析:先严格 `JSON.parse`,失败整段按字符串——初始草稿必须用 JSON 字面量文本(`formatJsonPrimitive`,字符串带引号),否则字符串 `"123"` 无操作提交会变 number。
5. `el-segmented` 用 `:model-value` + `@update:model-value` 受控写法,切树失败(闸门拒绝)可停留文本不回弹;新增 Element Plus 组件后 `components.d.ts` 由 build 再生成,需一并提交。
**涉及文件**:
- `apps/desktop/src/utils/jsonTreeSearch.ts`、`jsonTreeEdit.ts`、`jsonProcessTree.ts` 及配套测试;`jsonTreeView.ts`(+`toJsonPath`/`findJsonTreeParentNode`/`JsonTreePath`)
- `apps/desktop/src/composables/useJsonTreeSearch.ts`、`useJsonTreeEditing.ts` 及配套测试
- `apps/desktop/src/components/common/JsonTreeViewer.vue`、`JsonTreeNode.vue`、`JsonTreeNodeMenu.vue`(新增)、`JsonTreeViewer.test.ts`
- `apps/desktop/src/components/JsonProcessPanel.vue`、`JwtPanel.vue`、`CsvJsonPanel.vue` 及三个新增结构断言测试
- `apps/desktop/src/types/json-tree.ts`(新增)
**验证**:
- `pnpm test`(544 个,全部通过)
- `pnpm typecheck`、`pnpm --filter @lazycat/desktop build:web`
- 数据字典右键菜单与既有 JsonTreeViewer 结构断言回归通过

## 2026-07-04: API Mock 细节交互优化（组件拆分/未保存拦截/延迟模拟/并发改造）

**场景**: 按 `docs/superpowers/specs/2026-07-04-api-mock-ux-design.md` 实施 API Mock 联调反馈闭环与编辑效率优化，含路由级延迟模拟。
**使用次数**: 0
**问题**:
1. 单线程 accept 循环内串行处理请求，路由延迟会队头阻塞其他请求，且 `service_stop` 的 join 会被 sleep 卡住。
2. 行内启停开关与路由表单的 `enabled` 字段是两份状态，启停后表单里过期值会在下次保存时静默覆盖启停结果。
3. 表单未保存拦截需要区分"切换路由"（只涉及路由表单）和"切换项目/新建项目草稿"（同时重置两个表单）两种作用域。
4. `refreshRoutes` 自动选中首条路由会在保存项目等刷新路径上静默覆盖用户正在编辑的路由草稿。
**解决**:
1. 每连接 `thread::spawn`（快照 `Arc<Vec<_>>`），RAII 计数守卫限并发 64、超限 503；延迟 sleep 按 100ms 分片检查 stop，停止时直接断开不吐旧响应；`service_stop` 仍只 join accept 线程。Windows 下 accept 出的 socket 继承非阻塞模式，连接线程内先 `set_nonblocking(false)` 并加读超时。
2. 行内启停成功后，若目标路由正是表单加载对象，同步更新表单和 dirty 基线中的 `enabled`（解析基线 JSON 打补丁），其他字段 dirty 状态不受影响。
3. dirty 判断用"赋值时序列化基线快照 + 比较"实现（`serializeMockRouteForm`/`serializeMockProjectForm` 纯函数），`fileId` 从独立的 `routeFile` ref 并入快照；拦截弹窗用 `ElMessageBox` `distinguishCancelAndClose` 做三选（保存并切换/放弃/留下），保存顺序先项目后路由，任一失败留在原地。
4. 自动选中首条路由增加 `!isRouteDirty` 条件；保存路由成功后立即重建基线再触发刷新，避免刷新路径把刚保存的表单又判为 dirty。
5. 拖拽排序沿用数据字典模式：`Sortable.create`（forceFallback + filter 交互元素），onEnd 由子组件算出完整 id 顺序 emit，父级乐观更新数组 + 后端事务提交 + 失败刷新回滚。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/api_mock.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src/components/ApiMockPanel.vue`
- `apps/desktop/src/components/api-mock/`（ProjectList / RouteList / RouteForm / LogList）
- `apps/desktop/src/utils/apiMock.ts` + `.test.ts`
- `apps/desktop/src/types/api-mock.ts`、`apps/desktop/src/bridge/tauri.ts`
**验证**:
- `cargo test api_mock -- --nocapture`（27 个）与全量 `cargo test`（457 个）
- `pnpm test`（443 个）、`pnpm typecheck`、`pnpm --filter @lazycat/desktop build:web`

## 2026-07-02: API Mock Content-Type 选择与响应内容校验前端闭环

**场景**: 优化 API Mock 路由表单的 `Content-Type` 配置，支持常见类型选择、自定义 MIME，并提醒用户填写或上传匹配的响应内容。
**使用次数**: 0
**问题**:
1. `Content-Type` 原来是普通文本输入，常见类型需要手写，容易出现拼写错误或前后空白。
2. JSON、XML、HTML、表单类响应内容的可判断错误和不确定风险如果直接写在组件里，会和保存流程耦合。
3. 文件响应只能按扩展名推断，不能强阻断，但需要在用户选择的类型明显不一致时提醒。
**解决**:
1. 在 `apiMock.ts` 集中维护常见 `Content-Type` 预设、trim、MIME 归一化、Header 基本形态校验和响应内容检查。
2. `ApiMockPanel.vue` 使用 `filterable + allow-create + clearable` 的 `el-select`，保存前只阻断明确错误，warning 继续保存。
3. 文件导入和文件响应保存时复用同一个不匹配提醒函数，`application/octet-stream` 作为通用二进制不提示。
**涉及文件**:
- `apps/desktop/src/components/ApiMockPanel.vue`
- `apps/desktop/src/utils/apiMock.ts`
- `apps/desktop/src/utils/apiMock.test.ts`
**验证**:
- `pnpm test src/utils/apiMock.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-07-02: 连通性测试收藏夹用 settings JSON 做轻量持久化

**场景**: 为 IP/端口联通工具增加常用目标收藏、收藏命名和点击回填能力。
**使用次数**: 0
**问题**:
1. 收藏项只是用户偏好，不应新增业务表或后端通道扩大维护面。
2. 面板已有历史记录，但历史数据包含结果状态，不能直接复用为“用户命名收藏”的事实源。
3. 收藏清洗、默认名称、重复目标去重和回填规则如果写在 Vue 组件里，后续容易和 UI 状态混在一起。
**解决**:
1. 收藏项存入 `user_settings` 的 `network_test_favorites` JSON，字段只包含名称、协议、主机、端口、超时和创建时间。
2. 新增 `networkFavorites.ts` 纯函数，集中处理收藏构造、持久化数据清洗、重复目标置顶替换和回填表单值。
3. `NetworkPanel.vue` 只负责命名弹窗、列表展示、持久化调用和点击回填。
**涉及文件**:
- `apps/desktop/src/components/NetworkPanel.vue`
- `apps/desktop/src/utils/networkFavorites.ts`
- `apps/desktop/src/utils/networkFavorites.test.ts`
**验证**:
- `pnpm test src/utils/networkFavorites.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-07-02: API Mock 运行服务生命周期与用户反馈要闭环

**场景**: 复查 API Mock 实现逻辑并从用户使用角度优化启动、重启、文件响应和 CORS 预检体验。
**使用次数**: 0
**问题**:
1. 删除运行中的 Mock 项目只删除数据库配置，进程内服务仍可能继续监听端口，用户在界面上已看不到该服务。
2. 同一路径存在多 Method 路由时，CORS 预检只按路径选路由，可能返回错误的 `Access-Control-Allow-Methods`。
3. 文件响应沿用默认 JSON Content-Type，用户导入图片、PDF 等文件后容易得到错误响应类型；运行中保存配置只有“需重启”标签，缺少直接动作。
4. 重复导入同一文件会共享磁盘副本，按 `file_id` 清理可能误删仍被其他文件记录引用的副本；更新不存在的路由会伪成功。
**解决**:
1. `project_delete` 先停止对应运行服务，再删除配置和清理文件副本。
2. OPTIONS 预检读取 `Access-Control-Request-Method`，有目标 Method 时按该 Method 和路径共同选择 CORS 路由。
3. 前端抽出 API Mock 纯函数处理运行态动作、0.0.0.0 可访问 URL、文件 Content-Type 推断和文件大小展示；面板补空态、禁用路由标识、一键“重启生效”和明确错误提示。
4. 文件副本清理按 `stored_path` 检查其他仍被路由引用的文件记录；路由更新检查受影响行数，缺失时显式报错。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/api_mock.rs`
- `apps/desktop/src/components/ApiMockPanel.vue`
- `apps/desktop/src/utils/apiMock.ts`
- `apps/desktop/src/utils/apiMock.test.ts`
**验证**:
- `cargo test api_mock -- --nocapture`
- `pnpm test src/utils/apiMock.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-07-02: 浏览器身份搜索体验保持纯函数复用

**场景**: 优化浏览器身份面板，多 Edge Profile 场景下用户需要快速按别名、Edge 显示名或 Profile 目录定位身份。
**使用次数**: 0
**问题**:
1. 面板只有平铺列表，身份数量增加后用户需要逐行查找。
2. Spotlight 已有搜索字段构建逻辑，面板若另写字符串匹配会形成双重规则。
3. 隐藏身份如果搜索命中仍折叠，用户会误以为没有结果。
**解决**:
1. 在 `browserProfiles.ts` 增加 `filterBrowserProfiles`，复用 `buildBrowserProfileSearchFields` 和 `matchScore`，支持别名、Edge 名、Profile 目录和中文拼音首字母。
2. `BrowserProfilesPanel.vue` 只维护 `searchQuery` 和展示状态，先排序再过滤再分隐藏组。
3. 搜索态下隐藏身份自动展开，并把计数从“常用/隐藏”切换为“匹配/总数”。
**涉及文件**:
- `apps/desktop/src/utils/browserProfiles.ts`
- `apps/desktop/src/utils/browserProfiles.test.ts`
- `apps/desktop/src/components/BrowserProfilesPanel.vue`
**验证**:
- `pnpm test src/utils/browserProfiles.test.ts src/spotlight/providers/browser-profiles.test.ts src/spotlight/config-store.test.ts`
- `cargo test browser_profiles -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-07-02: 浏览器身份启动器避免复用通用参数拆分

**场景**: 新增 Edge Profile 启动器，按 Profile 目录名发现、展示、别名管理和 Spotlight 启动。
**使用次数**: 0
**问题**:
1. Launcher 的通用参数启动会用空白拆分，`--profile-directory=Profile 2` 会被拆坏。
2. Edge Profile 显示名可变且可能重复，不能作为稳定 key。
3. 面板和 Spotlight 都需要展示名、排序和权重，若各自实现会形成双重规则。
**解决**:
1. 后端独立 `browser_profiles` 模块启动 Edge，并把 profile 参数作为单个 `Command` arg。
2. 稳定 key 固定使用目录名，扫描结果为事实源，`user_settings` 只做覆盖层。
3. 前端抽 `browserProfiles.ts` 纯函数，面板和 Spotlight 共用展示名、排序和权重。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/browser_profiles.rs`
- `apps/desktop/src/components/BrowserProfilesPanel.vue`
- `apps/desktop/src/utils/browserProfiles.ts`
- `apps/desktop/src/spotlight/providers/browser-profiles.ts`
**验证**:
- `cargo test browser_profiles -- --nocapture`
- `pnpm test src/utils/browserProfiles.test.ts src/spotlight/providers/browser-profiles.test.ts src/spotlight/config-store.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-07-02: API Mock 运行态与持久配置分离

**场景**: 新增 API Mock 工具，支持多个项目持久化配置、每项目手动启动本地 HTTP 服务、文件响应副本和运行期最近请求日志。
**使用次数**: 0
**问题**:
1. Mock 项目/路由需要持久化，但服务线程、停止信号和请求日志不能写入数据库，否则应用重启会产生“看似运行”的伪状态。
2. 本地 HTTP 服务测试如果固定端口或共用自增项目 ID，Rust 并行测试会互相抢端口或污染全局运行态注册表。
3. 文件响应如果直接保存用户原始路径，会扩大运行时本地文件读取范围。
**解决**:
1. SQLite 只保存项目、路由和受控文件副本元信息；运行服务放在进程内 registry，项目列表通过当前配置签名与启动快照签名判断“需重启”。
2. Rust 冒烟测试使用 `127.0.0.1:0` 获取空闲端口，并给每个内存库设置不同 `api_mock_projects` 自增区间，避免并行测试 project_id 冲突。
3. 文件导入统一复制到 `<dataDir>/api-mock/files/`，请求返回前 canonicalize 并校验仍位于受控目录内；路由只引用 `file_id`。
**关键点**:
1. 本地服务启动前先读取不可变路由快照；首版不热更新，配置变化只提示重启。
2. 请求日志只记录时间、方法、路径、状态、命中路由、耗时和错误摘要，不保存完整请求头/请求体。
3. 删除路由或项目后按引用计数清理文件副本；磁盘清理失败只返回 warning，不回滚已完成的配置删除。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/api_mock.rs`
- `apps/desktop/src/components/ApiMockPanel.vue`
- `apps/desktop/src/utils/apiMock.ts`
- `apps/desktop/src/types/api-mock.ts`
**验证**:
- `cargo test api_mock -- --nocapture`
- `pnpm test src/utils/apiMock.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`


## 2026-07-01: 通用 JSON 树视图要把遍历规则留在纯函数层

**场景**: 为数据字典详情区把原始 JSON `<pre>` 替换成可折叠的通用只读 JSON 树视图。
**使用次数**: 0
**问题**:
1. 如果在业务面板里递归遍历 JSON，会把展开状态、路径编码、循环引用和复制格式化逻辑混进数据字典组件。
2. 直接用路径字符串拼接做节点 key，字段名包含点号、反斜杠或数组样式字符时容易冲突。
3. 从 hover-only 复制按钮改成显式工具栏时，源码结构测试会继续约束旧 DOM 和旧函数。
**解决**:
1. 新增 `jsonTreeView.ts` 纯函数，集中处理树构建、摘要、展开 key 收集、最大深度保护和安全复制格式化。
2. 通用 `JsonTreeViewer.vue` 只负责渲染、实例内展开集合和工具栏动作；数据字典只传 `rawJson` 和既有 `selectedJson`。
3. 同步更新数据字典源码结构测试，断言 `JsonTreeViewer` 接入、`copy-text` 传参和关系分组仍位于 JSON 区之后。
**关键点**:
1. `collectExpandedKeysByDepth(root, 2)` 的语义应固定为展开根节点和第一层可展开子节点。
2. 树构建和复制格式化都要处理循环引用，避免只保护渲染路径但复制路径仍可能崩溃。
3. 业务组件不应重复维护 JSON 复制按钮；复制成功/失败反馈由通用组件封装。
**涉及文件**:
- `apps/desktop/src/utils/jsonTreeView.ts`
- `apps/desktop/src/components/common/JsonTreeViewer.vue`
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`
**验证**:
- `pnpm test src/utils/jsonTreeView.test.ts src/components/common/JsonTreeViewer.test.ts src/components/DataDictionaryPanel.context-menu.test.ts`
- `pnpm test src/utils/dataDictionary.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-07-01: 番茄钟后台触发与前端倒计时分离

**场景**: 新增工作日番茄钟，支持 08:00 弹窗确认、默认 25/5 循环、12:00-13:30 午休跳过、17:00 结束。
**使用次数**: 0
**问题**:
1. 如果前后端都各自维护倒计时，午休跳过、跨窗口刷新和下班结束会形成双重真值。
2. 如果用前端定时器触发每日 08:00 弹窗，主窗口隐藏或未打开时会漏提醒。
3. 第一版若直接建业务表，会给后续自定义配置带来不必要迁移成本。
**解决**:
1. 后端只负责每日提示、今日会话状态和持久化；前端用纯函数根据配置、开始时间和当前时间推导阶段。
2. 主进程启动独立番茄钟调度线程，满足条件时创建 `pomodoro-prompt` 独立 WebView。
3. 配置和当日会话先存 `user_settings` JSON，默认配置保留可扩展字段。
**关键点**:
1. 午休跳过必须按有效工作时长计算，不能把 12:00-13:30 算进番茄周期。
2. 每日提示一旦展示就写入 `prompted`，避免用户关闭弹窗后每 30 秒重复打扰。
3. 弹窗动作走统一 `tool:pomodoro:*` 通道，并广播 `pomodoro-state-changed` 刷新主面板。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/pomodoro.rs`
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src/components/PomodoroPanel.vue`
- `apps/desktop/src/components/PomodoroPrompt.vue`
- `apps/desktop/src/utils/pomodoroSchedule.ts`
**验证**:
- `pnpm test src/utils/pomodoroSchedule.test.ts`
- `pnpm typecheck`
- `cargo test pomodoro`
- `pnpm --filter @lazycat/desktop build:web`









## 2026-06-28: 数据字典体验状态优先显式建模

**场景**: 数据字典主面板需要优化字段配置加载、搜索失败和空态体验，避免用户误操作或误判当前结果。
**使用次数**: 0
**问题**:
1. 字段配置抽屉先清空本地草稿再异步加载字段，若加载中允许保存，可能提交空字段配置并影响关系配置。
2. 搜索失败只弹 toast 且保留旧结果，用户容易把旧结果误认为当前关键词命中。
3. “无结果”空态没有区分无字典、无记录、无匹配和失败状态，不能指导用户下一步。
**解决**:
1. 为字段配置抽屉增加独立 `fieldLoading`，加载期间禁用保存，并在保存入口二次提示；后端 `update_fields` 同步拒绝空字段数组。
2. 为搜索增加 `searchError`，失败时清空当前结果和详情，在结果区展示错误与重试入口。
3. 用 computed 拆分结果区空态标题、说明和动作，分别覆盖导入字典、替换空字典数据和调整关键词/字段检索配置。
**关键点**:
1. 对会写入配置或重建索引的面板，加载态必须参与保存按钮禁用和保存函数早返回。
2. 异步搜索失败不要只依赖 toast；结果容器需要显式错误态，避免旧数据伪装成当前结果。
3. 空态不是单一文案，应根据数据规模、搜索范围、关键词和失败状态给出可执行下一步。
**涉及文件**:
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`
- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
**验证**:
- `pnpm test src/components/DataDictionaryPanel.context-menu.test.ts`
- `pnpm typecheck`
- `cargo test data_dictionary -- --nocapture`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-06-27: Spotlight 数据字典接入使用 query-time provider

**场景**: Spotlight 需要搜索数据字典记录，并支持打开定位、复制显示字段和懒加载复制完整 JSON。
**使用次数**: 0
**问题**:
1. 数据字典记录数量和单条 JSON 体积不可控，不适合在 Spotlight 空输入时预取。
2. Spotlight 前端不应重复实现数据字典字段路径解析、标题字段和显示字段摘要规则。
3. 异步 query-time provider 可能出现旧响应覆盖新查询结果。
**解决**:
1. 扩展数据字典 `search` 返回 `title` 和 `summary`，并用 `includeRawJson: false` 支持轻量候选。
2. Spotlight provider 增加可选 `search(query, ctx)`，数据字典只在有效关键词下按需请求。
3. Spotlight 查询结果用请求序号绑定当前 query 和 scope，旧响应直接丢弃。
4. 完整 JSON 复制通过 `record-detail` 懒加载，候选 payload 不保存 `rawJson`。
**关键点**:
1. 大数据源优先 query-time 搜索，不要塞进通用预取集合。
2. 动态 JSON 展示规则由数据字典后端单一维护，Spotlight 只做展示映射和动作编排。
3. `providerId:itemId` 去重时要合并预取和 query-time 结果，避免重复行。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
- `apps/desktop/src/types/data-dictionary.ts`
- `apps/desktop/src/spotlight/types.ts`
- `apps/desktop/src/spotlight/search.ts`
- `apps/desktop/src/spotlight/providers/data-dictionary.ts`
- `apps/desktop/src/components/SpotlightPanel.vue`
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/App.vue`
**验证**:
- `cargo test data_dictionary -- --nocapture`
- `pnpm test src/spotlight/providers/data-dictionary.test.ts src/spotlight/search.test.ts src/spotlight/config-store.test.ts src/utils/spotlight-query.test.ts src/components/DataDictionaryPanel.context-menu.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-06-27: 数据字典查询排序使用记录级派生 sort_key

**场景**: 数据字典“全部”查询需要先按左侧字典顺序，再按每个字典自己的记录排序配置返回结果。
**使用次数**: 0
**问题**:
1. 只在查询阶段解析 `raw_json` 排序会让全局搜索排序链路复杂，也不利于截断前排序。
2. 直接用 `normalized_value` 排序会混淆等值匹配和排序语义，数字排序也容易出错。
3. 降序如果反转整个排序键，会把缺失值或同值记录的兜底顺序也反转。
**解决**:
1. 在 `data_dictionary_records` 增加非空派生 `sort_key`，由当前 `sort_field_path`、`sort_direction` 和 `row_index` 编码生成。
2. 未配置排序字段或记录缺失排序字段时，把 `row_index` 编入 `sort_key` 作为兜底排序，不在查询 SQL 里额外补 CASE。
3. 降序只反转业务值编码段，不反转 bucket 和 row_index 兜底段，查询始终 `ORDER BY sort_key COLLATE BINARY ASC`。
**关键点**:
1. 派生排序键必须在导入、替换、字段配置保存、重建索引和历史数据回填路径同步维护。
2. 排序键是可重建索引，不是业务事实源；`raw_json` 仍是唯一事实源。
3. 排序必须发生在结果截断前，不能先取 100 条再在前端排序。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
- `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`
**验证**:
- `cargo test data_dictionary -- --nocapture`
- `cargo check`
- `pnpm test src/components/DataDictionaryPanel.context-menu.test.ts src/utils/dataDictionary.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-06-27: 数据字典大 JSON 导入绕开 IPC 文本负载

**场景**: 数据字典导入 10MB 级 JSON 文件，用户反馈超过约 5MB 后内容被截断或无法完整导入。
**使用次数**: 0
**问题**:
1. 原导入流程只支持在前端文本框中粘贴 JSON，并把完整文本放进通用 `tool_execute` IPC payload。
2. Rust 端 `serde_json` 解析和 SQLite 存储没有 5MB 业务限制；风险集中在前端文本输入和 IPC 大字符串传输边界。
3. 继续提升文本框或 IPC 负载阈值会扩大通用通道风险，也不能解决超大文件导入的内存与交互体验问题。
**解决**:
1. 前端导入弹窗新增“选择 JSON 文件”，选中文件后预览/保存只传 `inputPath`，文本框保留给小数据粘贴。
2. 后端新增 `read_import_input`，`import_preview` / `create` / `replace_records` 统一支持 `inputPath` 或原有 `input`。
3. 新增 10MB+ JSON 文件路径预览测试，锁住大文件不经 IPC 文本传输也能完整解析的行为。
**关键点**:
1. 大文件导入优先传路径让后端读文件，避免把大内容塞进通用 IPC payload。
2. 文本输入和文件路径是两个互斥来源，预览快照要绑定“当前来源”，否则保存可能提交和预览不一致的数据。
3. 模板里的复杂 JSON 示例不要直接写在绑定表达式中，容易被 Vue 模板解析截断；用 computed 字符串承载。

**涉及文件**:
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`
- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

**验证**:
- `cargo test data_dictionary -- --nocapture`
- `pnpm test src/components/DataDictionaryPanel.context-menu.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-06-27: 数据字典关系查询使用字段值派生索引

**场景**: 数据字典支持主键字段、字典间关系、详情页正向/反向关联和单字典索引重建。
**使用次数**: 0
**问题**:
1. 关系查询如果每次解析所有 `raw_json`，会把详情页加载变成跨字典全表扫描。
2. 仅靠 `normalized_value` 会把 JSON `null` 和字符串 `"null"` 混在一起，关系匹配语义不清。
3. 历史字典升级后没有字段值索引，不能把“索引未建”静默表现成“没有关联结果”。
4. 主键字段是业务唯一键；缺失、空值、非标量和重复主键不能继续作为合法关系目标。
**解决**:
1. 新增 `data_dictionary_record_values` 派生索引，按 `record_id + field_path` 存储 `value_type / value_text / normalized_value`，关系查询只走索引。
2. 在 `data_dictionaries` 增加 `primary_field_path` 和 `field_value_indexed_at`；详情查询遇到索引未就绪时返回可操作错误，提示单字典重建索引。
3. 导入、替换、字段配置保存和重建索引统一维护字段值索引；字段值索引失败回滚强一致数据，FTS 仍按既有降级策略处理。
4. 主键异常记录在导入/替换/保存主键配置时跳过并返回拆分统计；重建索引不删除历史原始记录，但异常主键不参与关系匹配。
**关键点**:
1. 动态 JSON 关系能力仍应保持 `raw_json` 为唯一事实源，字段值表只是可重建索引。
2. 索引表需要显式记录值类型，避免通过字符串归一化丢掉 JSON 类型语义。
3. 字段配置抽屉同时保存字典级配置、字段配置和关系配置，后端必须作为最终校验来源。
4. 详情页异步加载必须绑定请求序号，快速切换搜索结果时旧响应不能覆盖当前记录。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/utils/dataDictionaryRelations.ts`
- `apps/desktop/src/types/data-dictionary.ts`
- `apps/desktop/src/bridge/tauri.ts`

**验证**:
- `cargo test data_dictionary -- --nocapture`
- `pnpm exec vitest run src/utils/dataDictionary.test.ts src/utils/dataDictionaryRelations.test.ts src/components/DataDictionaryPanel.context-menu.test.ts src/utils/dataDictionaryMenu.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-06-26: 数据字典字段标题与字段配置排序

**场景**: 数据字典字段配置支持指定列表标题字段，并支持展示字段拖拽排序、展示/非展示字段分组管理。
**使用次数**: 0
**问题**:
1. 字段已有 `display_name` 作为字段标签，如果复用它表达“列表标题字段”，会把字段显示名和记录标题来源混成一个状态。
2. 记录排序配置已在字典级 `sort_field_path`，字段展示顺序已在 `data_dictionary_fields.sort_order`；新增标题字段也应是字典级单一选择。
3. Element Plus 表格不直接暴露行拖拽，需要在表格 body 上挂 Sortable，并在保存前重新写回 gapless `sortOrder`。
4. `el-drawer` 动画期间表格 body 可能尚未渲染，过早初始化 Sortable 会静默失败；如果每次打开都按可见性重排，也会覆盖用户已保存的手动排序。
5. 如果展示字段和非展示字段混在一张表内排序，隐藏字段会参与拖拽索引，用户拖动展示字段后容易出现顺序不符合预期。
**解决**:
1. 在 `data_dictionaries` 新增 `title_field_path`，`list/get/search` 都返回 `titleFieldPath`；搜索结果可直接根据原始 JSON 计算列表标题。
2. `update_fields` 同时保存 `titleFieldPath` / `sortFieldPath`，统一校验配置字段必须存在于本次提交字段集合中，空值回退默认来源标题。
3. 前端抽出 `buildResultTitle`、`orderDataDictionaryFieldDrafts`、`moveDataDictionaryFieldDraft`、`setDataDictionaryFieldVisibility`，字段抽屉打开和保存时统一把展示字段排在非展示字段前并重新编号。
4. 字段表格增加 `row-key="fieldPath"`，在 Drawer `opened` 后只对展示字段表初始化 Sortable；表格行未渲染时短暂重试。
5. 字段配置 UI 拆成“展示字段”和“非展示字段”两张列表；拖拽只影响展示字段，展示开关负责在两组之间移动字段，避免隐藏字段参与排序。
**关键点**:
1. “字段标签”“记录标题字段”“记录排序字段”“字段展示顺序”是四个独立模型，不能复用同一字段表达多种语义。
2. 可配置字段路径必须走后端校验，避免保存已不存在字段后前端只能静默回退。
3. 表格行拖拽保存完整字段顺序即可，后端只负责持久化 `sort_order`，避免前后端各自推断顺序。
4. Teleport / Drawer / Table 组合里做 DOM 级增强时，初始化时机必须绑定可见生命周期并允许条件重试。
5. 当用户只需要排序展示字段时，优先从交互结构上拆分展示/非展示集合，不要靠一张表里的复杂索引规则补救。

**涉及文件**:
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`
- `apps/desktop/src/utils/dataDictionary.ts`
- `apps/desktop/src/utils/dataDictionary.test.ts`
- `apps/desktop/src/types/data-dictionary.ts`
- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`

**验证**:
- `pnpm test src/utils/dataDictionary.test.ts src/components/DataDictionaryPanel.context-menu.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo test data_dictionary -- --nocapture`

## 2026-06-26: 数据字典左侧导航排序独立持久化

**场景**: 数据字典左侧导航支持拖拽排序。
**使用次数**: 1（2026-06-26 排查左侧导航拖拽启动失败时参考）
**问题**:
1. 字典列表原先按 `updated_at DESC` 返回；如果直接复用更新时间表达导航顺序，会把“内容最近更新”和“用户手动排序”混成一个状态。
2. 前端本地拖拽排序如果不落库，刷新后会丢失；如果只提交移动项和目标项，后端还要推断当前完整列表状态。
3. “全部”是固定全局搜索入口，不应参与具体字典排序。
**解决**:
1. 在 `data_dictionaries` 增加 `nav_order`，`list` 按 `nav_order ASC, updated_at DESC, id DESC` 返回；新建字典使用 `MAX(nav_order)+1` 放到末尾。
2. 新增 `reorder` action，前端 drop 后提交完整字典 id 顺序，后端事务内批量写入 gapless `nav_order`，并校验空数组、非数字、非正数和不存在字典。
3. 前端左侧具体字典项使用原生 HTML5 drag/drop；“全部”按钮固定顶部不设置拖拽事件；保存失败时恢复本地顺序并重新加载列表。
**关键点**:
1. 用户导航顺序应是独立模型，不能复用更新时间、名称或字段排序配置。
2. 排序保存优先提交完整顺序，后端只负责校验和持久化，避免双方各自推断产生双重真值。
3. 固定入口与可排序实体要在交互层分开，否则全局入口会被误持久化成业务数据顺序。

**涉及文件**:
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`
- `apps/desktop/src/types/data-dictionary.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`

**验证**:
- `cargo test data_dictionary -- --nocapture`
- `pnpm test src/components/DataDictionaryPanel.context-menu.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-06-25: 数据字典排序配置应作为字典级单一真值

**场景**: 字段配置中新增排序配置，支持指定排序字段和升降序。
**问题**:
1. 字段配置表已有 `sort_order`，语义是字段展示顺序；如果复用它表达记录排序，会让“字段顺序”和“记录顺序”混成一个状态。
2. 每个字段都保存是否排序会产生多个字段同时声明排序的双重真值，保存和查询都要额外仲裁。
3. 记录排序必须在截断 100 条前完成；先查 100 条再在前端排序会导致大数据字典排序结果不完整。
**解决**:
1. 在 `data_dictionaries` 上新增 `sort_field_path` / `sort_direction`，字典级保存单一排序配置；`data_dictionary_fields.sort_order` 继续只表示字段展示顺序。
2. `update_fields` 同时保存字段配置和排序配置，并校验 `sortFieldPath` 必须存在于提交字段中。
3. 当前字典搜索在后端按配置解析 `raw_json` 排序后再截断，数字按数值比较，缺失值升序/降序都排最后；无排序配置时保持原始行序。
**关键点**:
1. 同一个页面上的“字段排序”和“记录排序”要分开建模，避免一列多义。
2. JSON 字段排序优先复用已存在的转义点路径解析规则，不能用简单 `split('.')` 破坏含点字段名。
3. 排序、分页和截断同时存在时，排序必须发生在截断之前。

**涉及文件**:
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/types/data-dictionary.ts`
- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`

**验证**:
- `cargo test data_dictionary -- --nocapture`
- `cargo check`
- `pnpm test src/utils/dataDictionary.test.ts src/components/DataDictionaryPanel.context-menu.test.ts`
- `pnpm typecheck`

**使用次数**: 0

## 2026-06-25: 数据字典异步 IPC 结果必须绑定当前意图

**场景**: 修复数据字典快速切换字典、快速搜索和导入替换时的状态不一致问题。
**问题**:
1. 侧栏选中态由 `selectedId` 驱动，但详情和动作按钮依赖异步 `get` 返回的 `currentDictionary`，旧响应晚到会让高亮字典与实际操作目标不一致。
2. 搜索参数变化后，旧搜索响应仍可能覆盖新结果；同时限制 100 条时缺少截断提示。
3. 导入预览成功后继续编辑 JSON，保存会提交新输入但界面仍展示旧预览；替换写入前也缺少目标和覆盖范围确认。
**解决**:
1. 字典选择用请求序号校验，切换后先清空当前动作目标，旧 `get` 响应直接丢弃。
2. 搜索用请求序号加参数快照校验，只允许与当前 `scope / dictionaryId / keyword` 完全一致的响应写回；后端多取一条返回 `hasMore`，前端展示截断提示。
3. 导入预览记录输入快照，当前输入与预览不一致时禁用保存；替换模式保存前弹窗确认目标字典、旧记录数和新记录数。
**关键点**:
1. IPC 请求结果写 UI 状态时，必须校验“响应对应的用户意图仍是当前意图”，不能只靠 await 顺序。
2. destructive / replacement 类操作应绑定打开弹窗时的目标对象，并在提交前二次确认影响范围。

**涉及文件**:
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/types/data-dictionary.ts`
- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`

**验证**:
- `cargo test data_dictionary -- --nocapture`
- `pnpm --filter @lazycat/desktop test src/utils/dataDictionary.test.ts`
- `pnpm --filter @lazycat/desktop typecheck`

**使用次数**: 0

## 2026-06-25: Element Plus 右键菜单本地弹层与函数 ref 时序

**场景**: 数据字典列表项使用 `el-dropdown trigger="contextmenu"` 承载管理菜单，用户反馈菜单中的“替换”和“字段”点击后无法稳定打开本地 `el-dialog` / `el-drawer`，随后又出现 `Maximum recursive updates exceeded in component <DataDictionaryPanel>`。
**问题**:
1. Element Plus `el-dropdown-item` 点击时会先执行 `hideOnClick` 关闭 Popper，再触发父级 `command`。
2. “重命名/删除”走 `ElMessageBox` 服务式弹窗，不依赖当前组件内状态；“替换/字段”直接修改组件内弹层状态，容易和 Dropdown 的关闭点击栈冲突。
3. 模板函数 ref 会在渲染期间执行；如果 `setDictionaryMenuRef` 写入响应式 `ref`，就会在渲染期间再次触发组件更新，形成递归更新。
4. 只用源码字符串测试无法覆盖命令执行时序和响应式写入风险，容易漏掉这种交互回归。
**解决**:
1. 抽出 `dispatchDictionaryMenuCommand`，集中分发字典菜单命令。
2. 对会打开本地弹层的 `replace/fields` 使用 `setTimeout(..., 0)` 延后到下一个 macrotask；`rename/delete` 保持立即执行。
3. 菜单实例缓存改为普通 `Map<number, DictionaryMenuInstance>`，不参与 Vue 响应式系统；函数 ref 只 `set/delete` 这个 Map。
4. 新增 `dataDictionaryMenu.test.ts` 覆盖延后执行、立即执行和未知命令忽略三类行为，并在组件菜单测试中约束函数 ref 缓存不得使用响应式 `ref`。
**关键点**:
1. Dropdown 菜单项里打开组件内 Dialog / Drawer 时，优先让 Dropdown 的关闭流程先结束。
2. 服务式 MessageBox 和组件内受控弹层是两种不同模型，不能只因为都“打开弹窗”就按同一时序处理。
3. 模板函数 ref 只适合写非响应式外部缓存；不要在函数 ref 里替换 reactive/ref 对象。
4. 对交互命令分发，优先测可执行 helper；对 `.vue` 内响应式约束，可用源码测试兜住高风险结构。
**涉及文件**:
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/components/DataDictionaryPanel.context-menu.test.ts`
- `apps/desktop/src/utils/dataDictionaryMenu.ts`
- `apps/desktop/src/utils/dataDictionaryMenu.test.ts`

**验证**:
- `pnpm test src/components/DataDictionaryPanel.context-menu.test.ts src/utils/dataDictionary.test.ts src/utils/dataDictionaryMenu.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-06-24: 数据字典工具采用原始 JSON + 派生检索文本模型

**场景**: 新增数据字典工具，支持导入 JSON array、为嵌套字段维护字段含义，并提供当前字典和跨字典全局搜索。
**问题**:
1. JSON object 字段不固定，不能为每个字段动态建列，否则多字典、字段变化和嵌套路径会导致 schema 膨胀。
2. 用户期望“输入一段内容就匹配字段值”，SQLite FTS5 的 token 匹配不能完全替代包含匹配，尤其是中文、编号片段和短字符串。
3. 跨字典搜索需要结果携带来源字典，同时每个字典的字段配置仍需隔离。
**解决**:
1. SQLite 中保留 `raw_json` 作为唯一事实源，字段配置写入 `data_dictionary_fields`，记录检索文本写入 `data_dictionary_records.search_text / normalized_search_text`。
2. 嵌套 object 展开为点路径；原始 key 中的 `.` 和 `\` 在路径段内转义，数组和复杂对象作为叶子值序列化为紧凑 JSON 字符串。
3. 搜索始终执行 `normalized_search_text LIKE ... ESCAPE '\'` 保证包含匹配；FTS5 表 `data_dictionary_fts` 作为可用时的补充候选，创建或写入失败不阻断主流程。
4. `search` action 使用 `scope: "current" | "all"`，全局搜索返回 `dictionaryId / dictionaryName / rowIndex / rawJson / matches`，前端用字典来源标签区分结果。
**关键点**:
1. 动态 JSON 数据优先“原始值 + 派生索引”，不要把不稳定字段提升为数据库列。
2. FTS5 适合加速词项检索，但产品语义是“包含匹配”时必须保留 LIKE 或等价兜底。
3. 跨字典能力只要表结构从一开始带 `dictionary_id`，后续主要是查询范围和 UI 来源展示，不需要重做存储模型。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/data_dictionary.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src/components/DataDictionaryPanel.vue`
- `apps/desktop/src/utils/dataDictionary.ts`
- `apps/desktop/src/utils/dataDictionary.test.ts`
- `apps/desktop/src/types/data-dictionary.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/composables/toolCatalog.ts`
- `apps/desktop/src/tool-registry.ts`
- `docs/superpowers/specs/2026-06-24-data-dictionary-design.md`
- `docs/plans/2026-06-24-data-dictionary.md`

**验证**:
- `cargo test data_dictionary`
- `pnpm test src/utils/dataDictionary.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-06-24: Todo 详情编辑标题聚焦失效

**场景**: 双击任务列表项进入右侧编辑页后，需要自动聚焦标题输入框；现有实现设置了 `nextTick + setTimeout`，但实际测试未生效。
**问题**:
1. `TodoDetailEdit.vue` 通过 `defineExpose({ titleInputRef })` 暴露内部 `ref`。
2. `TodoPanel.vue` 按 `todoDetailEditRef.value?.titleInputRef.value?.focus()` 访问，依赖父组件看到的暴露值仍是 `Ref`。
3. 同类问题也影响编辑时间快捷入口对 `scrollRef.value` / `scheduleRef.value` 的访问。
4. Vue `<script setup>` 暴露给父组件的 ref 存在自动解包行为，父组件不应穿透子组件内部 ref 形态。
**解决**:
1. 在 `TodoDetailEdit.vue` 暴露明确方法 `focusTitleInput()`，由子组件内部访问 `titleInputRef.value?.focus()`。
2. 同步暴露 `focusScheduleInput()`，把滚动到时间字段并聚焦首个可用输入框的逻辑留在子组件内部。
3. `TodoPanel.vue` 只调用 `todoDetailEditRef.value?.focusTitleInput()` / `focusScheduleInput()`，保留创建/编辑模式校验和定时取消逻辑。
4. 更新静态回归测试，防止再次回到 `.titleInputRef.value` / `.scheduleRef.value` 的脆弱访问。
**关键点**:
1. 跨组件操作 DOM / 组件实例时，优先暴露语义方法，不暴露内部 ref 结构。
2. `defineExpose` 的 ref 解包细节不应成为父组件契约。
**涉及文件**:
- `apps/desktop/src/components/TodoDetailEdit.vue`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/components/TodoPanel.edit-focus.test.ts`

**验证**:
- `pnpm test src/components/TodoPanel.edit-focus.test.ts src/components/TodoPanel.title-enter.test.ts`
- `pnpm typecheck`

**使用次数**: 0

## 2026-06-12: Vault 存储重构为仅密码加密，Spotlight 支持按账号搜索

**场景**: 用户要求 Spotlight 在密码库锁定状态下也能按账号等字段搜索凭据条目并复制密码/账号。账号原与密码一起加密在 `encrypted_blob` 中，锁定时不可搜，需把存储模型重构为「只有密码加密、其余字段明文」。完整走了 brainstorming → spec（3 轮子代理评审）→ plan → 实施流程。
**问题**:
1. 加密数据格式变更涉及存量迁移，且解密必须有主密码，迁移时机只能落在某次解锁会话中。
2. 升级/降级共存期间存在混合格式：旧版编辑会整体加密写回，导致明文列陈旧（非 NULL），按 `plain_fields IS NULL` 判定回填会漏。
3. `record_usage` 原要求活跃会话，锁定态「复制账号」的计数会静默丢失，spec 初稿声明与实际行为矛盾（评审子代理发现）。
4. provider 模块顶层 import bridge/tauri 且注册副作用链经 registry 拖入重依赖，单测不可直接 import。
**解决**:
1. 纯函数三件套收口格式语义：`split_fields`（完整字段 → 加密部分/明文部分）、`blob_is_legacy`（blob 含非密码键即旧格式）、`merge_fields`（旧格式直接以 blob 为准、忽略陈旧明文；新格式 plain + password 合并），全部可无 DB 单测。
2. 回填判定用「blob 含非密码键」而非「明文列为 NULL」，使迁移幂等且能自愈降级期编辑；`cmd_unlock` 中**先回填再建会话**，关闭并发 IPC 读到混合状态的窗口；`change_password` 重加密循环作为第二触达路径顺手拆分。
3. 回填 UPDATE 不触碰 `updated_at`，避免迁移扰动「最近使用」排序；单行失败 eprintln 跳过，下次解锁重试，不阻断解锁。
4. `record_usage` 取消会话要求（仅递增明文计数列，与 `meta_list` 免会话同口径）。
5. 前端单测用 `vi.mock("../registry")` 斩断 registry → tool provider 的重依赖链，`buildItem`/`buildSubtitle` 加 export 直测。
**关键点**:
1. 加密数据格式演进的通用模式：「统一读取函数 + 格式判定谓词 + 解锁时机回填」，新旧格式共存期全功能可用，回退安全（新格式密码仍可被旧版解密）。
2. 迁移幂等条件应基于「数据本身的格式特征」（blob 键集合）而不是「迁移标记列」，否则降级期写入会绕过标记产生陈旧状态。
3. spec 子代理评审能抓住真实矛盾（如锁定态计数静默丢失、merge 语义被陈旧明文污染），3 轮迭代成本远低于实施后返工。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/vault.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src/spotlight/providers/vault.ts`
- `apps/desktop/src/spotlight/providers/vault.test.ts`
- `docs/superpowers/specs/2026-06-11-spotlight-vault-account-search-design.md`
- `docs/superpowers/specs/2026-06-11-spotlight-vault-account-search-plan.md`

**验证**:
- `cargo test --bins`（296 passed，含 9 个新增拆分/合成单测）
- `pnpm test`（245 passed，含 7 个新增 provider 单测）
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-05-07: Living Wallpaper（合成壁纸）端到端落地

**场景**: 实现 Living Wallpaper 全套：跨 PM/Todo 数据聚合 → hidden WebView 渲染信息层 → CapturePreview 抓 PNG → 与原壁纸合成 → IDesktopWallpaper 设回桌面 → 心跳/事件驱动调度 + 老板键 + 退出策略。Tauri 2.10 + Rust + Vue 3 + windows-rs 0.61，约 17 个 commit。

**问题**:
1. tool_execute 是 `#[tauri::command] fn ... -> ToolResponse` 同步 IPC，但 wallpaper.apply 需要 `AppHandle`（emit + with_webview），且涉及跨线程握手（UI 线程抓帧 vs 工作线程主流程）。如何在不引入 tokio、不破坏现有同步分发的前提下完成？
2. CapturePreview 是 `ICoreWebView2::CapturePreview` + COM stream + 异步回调，PoC 已抽出三段原语（`capture_inner` / `pump_messages` / `read_stream_to_vec`）。要让 PoC 调试入口（`wallpaper-poc-canvas` route）和正式 apply 路径（`wallpaper-canvas` route）共用一份实现，但 PoC 是 `cfg(all(windows, debug_assertions))`、apply 在 release 也要跑。
3. 后端发完 `wallpaper://dashboard-data` 事件后什么时候能抓帧？冷启 hidden WebView ~300ms 创建 + ~200ms Vue 挂载，热路径 ~50ms；如果在 emit 之前监听器还没注册，就丢事件；如果 emit 之后才创建窗口，前端没收到。
4. 心跳调度若没有跳过条件，用户锁屏 / 全屏游戏期间也会强行刷新；空闲降频又涉及 `GetLastInputInfo` + 32 位 tick 回环；锁屏判定用 `WTSRegisterSessionNotification` 需要建 message-only window，太重。
5. PM/Todo CRUD 后想立刷壁纸，但用户连按 3 个完成不能 fire 3 次合成；需要 trailing-edge 5s debounce。
6. 启用壁纸（enable）→ 备份原图 → 合成新图 → 设桌面后，用户退出 LazyCat 默认应恢复原图，否则合成图永久残留在桌面。

**解决**:
1. **AppHandle 路由 + 全程同步握手**。沿 `settings::execute_with_app` 模式扩展：`wallpaper::execute_with_app(action, payload, &AppHandle)` 只在需要 app 的 action 上转发（`apply` / `resume`）；其余仍走 sync `execute`。`tools/mod.rs::execute_tool_with_app` 处只多一行 `"wallpaper" => wallpaper::execute_with_app(...)`。整条 apply 流程不要 async：emit / set_wallpaper / image 编解码本身就是 sync；canvas 握手用 `std::sync::mpsc + tauri::Listener`；CapturePreview 跨 UI/worker 线程用 `Arc<Mutex<Option<Result<Vec<u8>>>>>` + `AtomicBool` done flag + 5ms 轮询。整条流程在工作线程上同步完成，IPC 直接返回结果。
2. **抽到 `tools/wallpaper/capture.rs`，cfg(windows) 真实 + 非 Windows stub，不带 debug_assertions**。PoC 的 `capture_inner` 改为 `crate::tools::wallpaper::capture::capture_inner(webview)` 一行 delegate。public API 用 `pub use imp::capture_inner;` + `cfg(not(windows))` 提供同名 stub fn，调用方完全不感知平台差异。
3. **两阶段握手 + 边沿监听**。`WallpaperCanvas.vue` 挂载完成立即 `emit('wallpaper://canvas-mounted')`；数据到达后等 2 RAF 再 `emit('wallpaper://canvas-ready')`。后端 apply：先 `is_canvas_open(app)` 记录冷启 / 热路径；冷启时 `wait_for_event('wallpaper://canvas-mounted', 2s)`；之后**先注册** `canvas-ready` listener **再 emit** 数据，避免 emit 后立即响应漏接；ready 超时 2.5s 不致命，仍尝试 capture（前端可能渲染慢但已绘制）。
4. **多模块各管一事，all 同步轮询，failures 回退「不锁定」/「不空闲」**。`lock.rs` 用 `OpenInputDesktop + GetUserObjectInformationW` 取桌面名，"Default" 之外都判锁定；`fullscreen.rs` 用 `SHQueryUserNotificationState` 三态判定（`QUNS_BUSY` / `QUNS_RUNNING_D3D_FULL_SCREEN` / `QUNS_PRESENTATION_MODE`）；`idle.rs` 用 `GetLastInputInfo + GetTickCount64`，`u32::wrapping_sub` 处理 tick 回环。所有判定只读不写 state，scheduler::should_skip 串成一条调用链；不再需要后台轮询线程也不需要 message-only window。
5. **trailing-edge 滚动 deadline**。`events.rs::start` 在 std::thread 里 `mpsc::sync_channel(16)` recv → 收到第一条事件后 `deadline = now + 5s`，每来一条新事件就 `deadline = now + 5s` 重置；`recv_timeout(deadline - now)` 超时即触发 apply。`tools/mod.rs::execute_tool` wrap 一层在 PM/Todo 数据变更类 action 成功后 `notify_data_changed` —— 仅 `try_send` 不阻塞业务流，channel 满直接丢（debounce 后只 fire 一次反正等价）。`should_skip` 与 scheduler 同口径，避免事件驱动绕过禁用 / 锁屏。
6. **`RunEvent::ExitRequested` 钩子 + `restore_wallpaper` 复用**。把 `.run(ctx)` 拆成 `.build(ctx).run(|h, ev| match ev { ExitRequested => on_app_exit })`：托盘 Quit / 最后一窗关闭 / `app.exit()` 三个路径都覆盖。`on_app_exit` 读 `wallpaper.exit_behavior`：`keep_last` no-op，`restore_original`（默认）调既有 `restore_wallpaper`。错误只 stderr，不阻塞退出。

**关键点**:
1. **「需要 AppHandle」≠「需要 async」**。Tauri command 本身就是同步包装；`emit` / `with_webview` / `Listener::listen` 都不要求 async runtime。整个 apply 链路用 std::sync::mpsc + Arc<Mutex> 完成跨线程握手，比引入 tokio 简单得多，也避免了 IPC 工作线程上 `block_on` 可能的 reentrancy 风险。
2. **PoC 与正式实现共用底层原语，cfg gate 用 `pub use` + `not(windows)` stub fn 同名互补**。`capture.rs` 是这次最干净的复用：PoC 改为一行 delegate，正式 apply 直接调；release/debug 都可用，跨平台编译过 stub。windows-rs 0.61 有些类型（如 `HDESK` ↔ `HANDLE`）没自动转换，需要手工 `HANDLE(h.0)` 包一次（结构体都是 `*mut c_void`），编译器会精确指出错误位置。
3. **跨 webview 事件握手必须「先注册再 emit」**。Tauri 事件不重放；如果 emit 之后才注册，事件已经派发完毕。两阶段握手（mounted 通知存活 + ready 通知绘制完成）解决冷启动顺序问题；冷启专用 `wait_for_event('canvas-mounted')`，热路径直接跳到 ready 等待。
4. **「跳过判定」是中央枢纽**。心跳 `should_skip` 串 4 个判定（enabled / paused / lock / fullscreen），事件驱动 `should_skip` 串同一组（虽然代码重复一份，函数都很短）；任何新增切净源（Spotlight / 第三方引擎）只改这两处。`lock`/`fullscreen`/`idle` 各自 cfg(windows) 真实 + 非 Windows stub 返回 false（不锁定 / 不空闲），保证「失败回退到不暂停」—— 错误的暂停比错误的刷新更难诊断。
5. **content hash 去重 + force 标记两套**。`apply_with_force(app, force: bool)`：心跳 / 事件驱动 force=false（DefaultHasher 比对 dashboard JSON + base path + base mtime + position + mode，命中跳过整条 compose+persist+set 链路）；手动「立即刷新」/ 老板键 resume / restore force=true。`enable / restore / boss_key.toggle` 三处显式调 `apply::invalidate_input_hash()`，保证桌面切换边界后下一帧必然真渲一次。hash 用 sentinel 0 表示「无效」，结果碰撞回退到 1 防误判。
6. **「事件驱动 + 心跳」双路径但只信 hash**。前者用 trailing-edge 5s debounce 防抖；后者用空闲降频（5min idle → 60min sleep）+ 30s 分块 sleep + 边沿检测「刚回来立刷」。两路径都走 `apply_with_force(force=false)`，靠 hash 去重避免无意义合成；用户感受是「数据一变 5s 内就刷新」「闲着不耗 CPU」「回来立刻看到最新」。

**涉及文件**:
- `apps/desktop/src-tauri/Cargo.toml`（windows features 追加 `Win32_System_StationsAndDesktops` / `Win32_System_SystemInformation` / `Win32_UI_Input_KeyboardAndMouse` / `Win32_UI_Shell_PropertiesSystem` / `Win32_UI_WindowsAndMessaging` / `Win32_Graphics_Gdi`）
- `apps/desktop/src-tauri/src/tools/wallpaper/`（新增模块）：
  - `capture.rs`（CapturePreview 原语）/ `compose.rs`（区域 / 采样 / 缓存 / 合成 / 写盘）/ `desktop.rs`（IDesktopWallpaper + SysParam 双层）
  - `data.rs` + `dashboard_logic.rs`（跨 PM/Todo 聚合）
  - `hidden.rs`（hidden WebView 生命周期）/ `apply.rs`（主流程 + hash 去重）
  - `scheduler.rs`（心跳 + 空闲降频 + burnout）/ `events.rs`（trailing-edge debounce）
  - `lock.rs` / `fullscreen.rs` / `idle.rs`（三个独立检测模块）
  - `boss_key.rs`（Ctrl+Alt+W toggle）
  - `mod.rs`（execute / execute_with_app 路由 + on_app_exit）
- `apps/desktop/src-tauri/src/tools/pm_today.rs`（`priority_rank` 提为 `pub`）/ `tools/todo.rs`（`is_open_status` 提为 `pub`）—— 跨模块复用判定
- `apps/desktop/src-tauri/src/tools/mod.rs`（dispatch_tool 抽出 + PM/Todo CRUD action 白名单 → notify_data_changed）
- `apps/desktop/src-tauri/src/main.rs`（scheduler/events/boss_key 注册；`.run(ctx)` 拆 build+run 接 ExitRequested）
- `apps/desktop/src-tauri/src/wallpaper_poc.rs`（`capture_inner` 改为 delegate）
- `apps/desktop/src/components/`：`WallpaperPanel.vue`（状态卡片 + 三组配置）/ `WallpaperCanvas.vue` + `WallpaperOverviewBlock.vue` + `WallpaperTodoList.vue` + `WallpaperExtensionSlot.vue`（hidden WebView 信息层 360×800）
- `apps/desktop/src/WallpaperCanvasApp.ts`（mount entry）/ `main.ts`（`wallpaper-canvas` 路由）
- `apps/desktop/src/composables/toolCatalog.ts`（"更多工具" 组追加桌面壁纸入口）/ `tool-registry.ts`（注册 WallpaperPanel）
- `docs/superpowers/specs/2026-05-05-living-wallpaper-{design,plan}.md`（设计与实施文档）

**验证**:
- `cargo test --bin lazycat-desktop tools::wallpaper::` → 67 passed（含 4 个 hash 纯函数单测）
- `cargo test --bin lazycat-desktop tools::` → 287 passed（PM/Todo 既有用例不受 dispatch_tool 包装影响）
- `cargo check --bin lazycat-desktop` → ok（仅 pm_siyuan 既有 `private_interfaces` 警告，与本次无关）
- `pnpm typecheck` → ok
- `pnpm --filter @lazycat/desktop build:web` → ok（`WallpaperPanel-*.js` / `WallpaperCanvasApp-*.js` 独立分块）
- 真机端到端实测仍待补：需启动 dev server 在真桌面环境触发一次 `tool:wallpaper:apply` 验证 base 解码 / region 计算 / DPI scale / set_wallpaper 链路

**使用次数**: 0

---

## 2026-04-19: PM 视图扩展与列表渐进式渲染

**场景**: 在 PM 面板上新增「今日 / 列表 / 日历 / 四象限」4 个视图，原本只有看板和甘特两个视图，切换靠 `el-switch`。扩展后需要 6+ 视图共享一个切换器，并对大数据量做响应式与渲染性能兜底。

**问题**:
1. 面板内多视图系统没有注册表；新增一个视图要在 `PmPanel.vue` 的 template、script、watch 里同时改 `v-if === 'xxx'`，扩展性差。
2. 上下文（overview / project-<id>）× 视图正交要求：切项目后视图选择要记住；但 `user_settings` 读写的 key 策略要统一，不然不同入口（侧栏/切换器）切不同步。
3. 列表视图 `el-table` 全量渲染在 1000+ 条数据下首帧明显卡顿；直接迁 `el-table-v2` 会丢失排序、多选、Popover 内联编辑等能力。
4. 切换器响应式降级若用 CSS 媒体查询，面板可能被嵌在不同父布局，阈值不一致；若观察组件自身 `inline-flex` 宽度，又会陷入“自己撑多宽就是多宽”的反馈循环。

**解决**:
1. 新增 `composables/pmViewRegistry.ts`：集中注册 6 个视图（id / label / icon / defineAsyncComponent），`PmPanel.vue` 通过 `<component :is="currentView.component">` 渲染。新增视图只改注册表一处。
2. 新增 `composables/usePmViewMemory.ts` 封装「上下文 → viewId」记忆，`user_settings` key 规则 `pm:view:overview` 与 `pm:view:project-<id>`；侧栏「今日」入口和顶部切换器共用同一个 `setView`，避免两个路径分叉。
3. 列表视图不迁 v2，改为渐进式渲染：当 `groupBy === 'none'` 且过滤后 >500 行时，首批渲染 200 行，滚动到底部 240px 内自动追加 200；排序/筛选/分组变化时重置到首批并回到顶部。滚动监听节流交给浏览器（单次 scroll 事件判断），实现成本低于集成 `vue-virtual-scroller`。
4. 切换器观察 `document.documentElement`（视口尺寸）的 `ResizeObserver`，阈值 1100px；label 在 compact 模式下由 `el-tooltip` 补齐。观察视口等价于「窗口宽度」但不用 CSS 媒体查询，仍保持可被父布局复用的语义。

**关键点**:
1. 面板内的「视图注册表」是 `tool-registry.ts` 的微缩复刻：同样的 `id → async component` 模式，拿到扩展性的同时避免 PmPanel 再膨胀。
2. 渐进式渲染是 el-table 虚拟滚动的廉价替代品：性能目标（首帧 <50ms、滚动不卡）可达，且不触碰成熟组件的多选/排序/内联编辑契约。数据量上到万级再考虑真正虚拟化。
3. `ResizeObserver(document.documentElement)` 是「窗口宽度但不用媒体查询」的稳妥写法；观察组件自身 inline-flex 容器会形成反馈循环，观察父元素又受父布局影响，观察视口最中立。
4. 后端 5 个新 action（`item_today_list / item_today_counts / item_calendar_range / item_matrix_bucket / item_batch_update`）配合 `pm_items` 已有索引 `idx_pm_items_project_status/end_at/status/updated_at/completed_at`，跨项目查询在千行规模下 <5ms，无需额外性能工程。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/pm_today.rs`（新增）
- `apps/desktop/src-tauri/src/tools/pm_calendar.rs`（新增）
- `apps/desktop/src-tauri/src/tools/pm_matrix.rs`（新增）
- `apps/desktop/src-tauri/src/tools/pm.rs`（新增 `item_batch_update`）
- `apps/desktop/src-tauri/src/tools/mod.rs`、`helpers.rs`
- `apps/desktop/src/composables/pmViewRegistry.ts`（新增）
- `apps/desktop/src/composables/usePmViewMemory.ts`（新增）
- `apps/desktop/src/composables/usePmListPrefs.ts`（新增）
- `apps/desktop/src/components/PmViewSwitcher.vue`、`PmKanbanView.vue`、`PmTodayView.vue`、`PmListView.vue`、`PmCalendarView.vue`、`PmMatrixView.vue`、`PmMatrixQuadrant.vue`、`PmTodayCard.vue`、`PmTodaySection.vue`（新增）
- `apps/desktop/src/components/PmPanel.vue`、`src/bridge/tauri.ts`
- `CLAUDE.md`、`AGENTS.md`（新增 04.7 PM 域视图扩展小节）

**验证**:
- `cargo test tools::pm_` → 19 passed
- `pnpm typecheck`
- 后续需补 `pnpm test` / `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-08: PM 侧栏排序口径与项目计数口径必须拆开建模

**场景**: 用户先后确认了两条 PM 规则：`archived` 项目不能再接收工作项，且 `archived` 项目不参与“按任务总数排序”；随后修复 A4 / A6 / A7 时，需要同时解决“总览只看 active”“侧栏排序失真”“总览摘要口径不一致”三类问题。

**问题**:
1. 原实现把 `item_counts()` 同时承担了两个职责：
   - 给项目卡和总览卡提供数字
   - 给侧栏排序提供总任务数依据
2. 一旦后端只统计 active 项目，archived 项目的卡片数字会被误显示成 `0`，总览摘要也会只覆盖 active；但如果粗暴地把 archived 也纳入排序，又会和“archived 不参与排序”的规则冲突。
3. 总览 `item_list()` 原来带 `WHERE p.status = 'active'`，导致 archived 项目下的工作项在总览消失、在单项目视图出现，形成展示分裂。

**解决**:
1. 先把“可见集合”修正为真实全量：
   - 总览 `item_list()` 去掉 `p.status = 'active'`
   - 让 archived 项目的既有工作项也进入总览
2. 再把“计数”和“排序”明确拆开：
   - `item_counts()` 返回所有项目的真实 `total / done`
   - 这样 archived 项目卡数字和总览摘要都基于真实数据
3. 前端 `sortPmProjectsForSidebar()` 改成两段式规则：
   - `active` 项目排在前面，按任务总数降序
   - `archived` 项目统一置后，只按 `sortOrder / name / id` 走稳定兜底
4. 侧栏文案同步改成“活跃项目优先，按任务总数排序”，避免继续把 archived 误描述成参与同一排序口径。

**关键点**:
1. “项目卡要显示真实数字”和“项目列表要怎么排”是两个不同问题，不能继续共用同一口径硬绑在一起。
2. 当产品规则变成“archived 不参与排序”时，最稳的实现不是不给 archived 计数，而是保留真实计数、单独处理排序分支。
3. 总览如果要承担全局入口语义，可见集合和摘要口径必须对齐；否则用户会看到“卡片数和列表内容不是一个世界”的错觉。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/pm.rs`
- `apps/desktop/src/utils/pmVisual.ts`
- `apps/desktop/src/utils/pmVisual.test.ts`
- `apps/desktop/src/components/PmPanel.vue`
- `process.md`

**验证**:
- `cargo test pm::tests -- --nocapture`
- `pnpm test src/utils/pmVisual.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-08: 本周工作面板改为按 PM 计划时间命中本周统计

**场景**: 用户反馈“本周工作”里没有拿到本周进行中的项目管理任务；确认后的口径是：PM 任务是否进入“本周工作”，只看 `startAt / endAt` 是否命中本周，不再依赖 `status` 或 `completedAt`。

**问题**:
1. `pm.rs` 里的 `weekly_work()` 原实现把 PM 数据硬编码为 `status = 'done' AND completed_at >= 最近 7 天起点`，因此只会返回近 7 天已完成任务，进行中或测试中的本周任务会被直接漏掉。
2. 面板标题、空态和工具描述仍写着“最近 7 天工作 / 完成工作汇总”，和“本周工作”的实际入口名称、用户心智以及目标口径不一致。
3. `WeeklyWorkPanel.vue` 的时间列和分组排序都依赖 `completedAt`，即使后端补回 PM 任务，前端也无法正确展示计划时间范围。

**解决**:
1. 在 `pm.rs` 新增纯 helper：
   - `normalize_pm_weekly_range()`
   - `resolve_pm_weekly_window_hit()`
   - `resolve_current_week_window()`
   统一收口“单边日期补全、倒序日期纠正、本周窗口判定和排序日期钳制”。
2. `weekly_work()` 改为：
   - 本周窗口按“周一 00:00 ~ 周日 23:59:59”计算
   - PM 任务只要 `startAt / endAt` 归一化后与本周有交集就返回
   - 不再过滤 `status`
   - Todo 仍按完成时间统计，但窗口同步改为“当前自然周”而不是“最近 7 天”
3. PM 返回体补充 `startAt`、`endAt`、`sortAt`，让前端直接显示计划时间范围并按本周命中时间排序。
4. `WeeklyWorkPanel.vue` 同步改为“本周工作”文案，PM 项展示计划时间范围与状态标签；排序从 `completedAt` 切换为统一 `sortAt`。
5. `App.vue` 的工具描述一并改成“按本周时间范围汇总工作项”，避免入口文案继续误导。

**关键点**:
1. “本周工作”如果要承载 PM 计划视角，不能继续复用“已完成事项”思路；PM 应看排期区间与本周窗口是否相交，而不是看完成态。
2. 对只有单边日期的 PM 任务，周统计也要命中；更稳的做法是把单边日期归一成单日区间，再统一做 overlap 判断。
3. 周窗口如果要避免“23:59:59.xxx”边界遗漏，SQL 过滤尽量用 `[weekStart, nextWeekStart)` 的半开区间，而不是字符串形式的 `<= 周日 23:59:59`。
4. 当统计口径从“完成时间”切到“计划时间范围”后，前端时间列、排序字段和空态文案必须一起切换，否则用户会觉得数据虽然回来了，但展示仍然不对。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/pm.rs`
- `apps/desktop/src/components/WeeklyWorkPanel.vue`
- `apps/desktop/src/types/pm.ts`
- `apps/desktop/src/App.vue`
- `process.md`

**验证**:
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml weekly_ -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-08: Base64 面板自动识别前端收口为纯函数校验 + 手动选择持久化

**场景**: 用户要求按 `2026-04-07-base64-auto-detect-design.md` 实现 Base64 自动识别与类型同步，同时先审校 spec，再按审校后的规则落地前端最小改动方案。

**问题**:
1. 现有 `EncodePanel.vue` 只有一个 `base64UrlSafe` 布尔开关，编码与解码都完全依赖当前按钮状态，粘贴已编码文本时很容易因为类型没切对而直接解码失败。
2. 这次目标不是改后端能力，而是让“输入时自动同步显示 + 解码时按识别结果纠偏”同时成立，因此如果把逻辑直接堆进组件，手动选择、自动识别、切工具恢复三者的优先级会很快变乱。
3. Rust 侧实际用的是 `base64 0.22` 的 `STANDARD` 和 `URL_SAFE_NO_PAD`；如果前端偷懒用 `atob` / `Buffer` 判断，会和后端在 padding、无 padding、trailing bits 上出现细微边界偏差。
4. Base64 输入状态当前通过 `EncodePanel.vue` 顶部普通 `<script>` 里的模块级 `encodeState` 持久化；如果只记显示状态、不记“手动选择”，切换工具回来后歧义输入的解码决策会漂移。

**解决**:
1. 先审校 spec，把实现边界收口为唯一方案：
   - `manualChoice` 必须进入 `encodeState`
   - `detectedKind` 只做运行时派生，不持久化
   - 自动识别同步显示不得反写 `manualChoice`
   - 前端可解码性校验必须自己实现，不能依赖浏览器宽松解码器
2. 新增 `apps/desktop/src/utils/base64.ts`，把识别与解码决策抽成纯函数：
   - `detectBase64Kind`
   - `resolveBase64DecodeKind`
   - 内部按 Standard / URL-safe 两套字母表、padding 规则和 trailing bits 规则做轻量校验
3. `EncodePanel.vue` 中把 Base64 类型切换改成显式 `@update:model-value` 入口，只在用户主动点击时写入 `manualChoice`，避免自动同步污染手动兜底状态。
4. 输入变化时统一走 `watch(base64Input, ..., { immediate: true })`：
   - 明确类型时自动同步显示
   - 歧义类型时按 `manualChoice ?? "standard"` 显示
   - 非 Base64 输入不改当前显示类型
   - 输入清空时重置 `manualChoice`
5. 解码时不直接信当前按钮状态，而是重新识别一次当前输入，再通过 `resolveBase64DecodeKind` 决定最终走 Standard 还是 URL-safe 通道。
6. 新增 `apps/desktop/src/utils/base64.test.ts`，固定明确类型、歧义输入、trailing bits 非法输入、空白输入和解码决策优先级。

**关键点**:
1. 这种“自动识别 + 仍保留手动切换”的交互，最容易错在把自动同步和手动选择混成一个状态；更稳的做法是把“当前显示类型”和“用户手动偏好”分开建模。
2. Vue 表单控件如果既要支持程序性同步，又要精确捕捉“只有用户操作才算手动选择”，优先用 `:model-value + @update:model-value`，不要直接依赖 `v-model` 推断来源。
3. 想和 Rust `base64` crate 保持一致时，前端至少要自己校验最后一个 sextet 的未使用 bits 是否为 0；否则像 `AB==`、`ABC=`、`AB`、`ABC` 这类输入会被前端误判为可解码。
4. Base64 这类共享字符集协议天生会和普通短文本重叠，像 `test` 这样的 4 字符共享字符集文本落入 `ambiguous` 是当前最小改动方案下的接受成本，必须用单测固定，避免后续“优化”把规则漂移掉。
5. 组件级持久化场景下，只恢复显示值不够；凡是会影响歧义分支决策的状态，都要和输入一起持久化，否则切换工具回来后行为会前后不一致。

**涉及文件**:
- `apps/desktop/src/components/EncodePanel.vue`
- `apps/desktop/src/utils/base64.ts`
- `apps/desktop/src/utils/base64.test.ts`
- `docs/superpowers/specs/2026-04-07-base64-auto-detect-design.md`
- `process.md`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/base64.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-07: 代理规范文档按检索场景重构并补 Agent 防错闸门

**场景**: 用户要求同时更新 `AGENTS.md` 和 `CLAUDE.md`，目标不是增加更多规则，而是提升 agent 的实际使用效果，尤其减少漏规则、漏同步、漏验证和误触高风险操作。

**问题**:
1. 原有两份规范虽然内容基本一致，但更像连续说明文；agent 在执行时需要自己从全文拼装“当前任务属于什么场景、该查哪节、该做哪些检查”。
2. 文档中混有不少容易过时的数量型描述，对执行帮助有限，却会增加规范失真概率。
3. 仅做“结构重排”还不够解决 agent 漏项问题；如果没有显式的执行入口，agent 仍会漏掉双文件同步、Element Plus 双文件覆盖、`process.md` 沉淀等约束。
4. 这类规范改动本身又受“双文件同步约束”约束，如果先改一份再手工复制另一份，最容易产生细微漂移。

**解决**:
1. 先走设计闭环：确认目标是“日常检索更快”，再追加“agent 专项优化，以更少犯错为第一优先级”，并将设计固化到 `docs/superpowers/specs/2026-04-06-agent-doc-structure-optimization-design.md`。
2. 正文结构统一重组为 `01-08` 八个一级章节，把原有规则重新收口到：
   - `02.1 问题导向索引`
   - `02.2 Agent 决策闸门`
   - `07.4-07.7` 四组场景化检查清单
3. 将“agent 更少犯错”具体化为两层机制：
   - 顶部 `Agent 决策闸门`：先判断任务场景、同步约束、确认要求、最低验证和经验沉淀
   - 底部场景化检查清单：文档规范改动、普通功能开发、UI / 样式改动、高风险改动
4. 对容易过时但不影响执行的数量型描述做降噪处理，保留路径、文件、机制与流程，不继续保留低收益统计数字。
5. 双文件一致性不靠肉眼，完成后用规范化文本比较：将 `AGENTS.md` / `CLAUDE.md` 的文件名差异抹平后做 `Compare-Object`，确保两份文件除标题与互引对象外完全同构。

**关键点**:
1. 规范文档优化如果目标是“提升 agent 效果”，不能只做目录美化；必须把执行前最常漏的判断前置成显式闸门或 checklist。
2. 对规范类文档，最危险的不是“写少了”，而是“改出语义漂移”；因此设计阶段要明确“只重组，不扩边界”，清单也只能映射已有规则，不能借机发明新规则。
3. 像模块数、通道数、组件数这类统计信息，如果对执行帮助不大又容易过时，宁可删掉数字保留路径与机制，这样 agent 命中率更高、维护成本也更低。
4. 双文件同步场景下，完成后的结构一致性校验最好走文本归一化后比对，不要只凭人工快速扫一遍。

**涉及文件**:
- `AGENTS.md`
- `CLAUDE.md`
- `docs/superpowers/specs/2026-04-06-agent-doc-structure-optimization-design.md`
- `process.md`

**验证**:
- `Compare-Object` 对 `AGENTS.md` / `CLAUDE.md` 做归一化比对，结果为 `IDENTICAL`
- 检查两份文件的 `##` / `###` 标题结构完全一致
- 人工校对关键规则、构建发布规则、`process.md` 规则和 agent 检查清单均已保留

**使用次数**: 0

## 2026-04-06: Windows 正式发版前先处理版本号与已存在 tag 冲突

**场景**: 用户要求直接开始打包编译新版本并推送到 GitHub Release；仓库已存在上一版 `v0.2.6` tag，本地 4 处版本文件仍停留在 `0.2.6`。

**问题**:
1. `scripts/release-all-win.ps1` 会强校验 `package.json`、`apps/desktop/package.json`、`apps/desktop/src-tauri/Cargo.toml`、`apps/desktop/src-tauri/tauri.conf.json` 四处版本完全一致，并要求传入 tag 必须等于 `v<version>`。
2. 远端如果已经存在同名 tag，继续沿用旧版本号会在发布阶段直接撞上旧 tag，无法当成“新版本”重新发。
3. 发布脚本在上传 GitHub Release 前会要求当前分支为 `main` 且工作区干净，因此版本更新、经验记录这类改动必须先提交，不能留到脚本执行中途。

**解决**:
1. 先用 `git tag --list` / `git ls-remote --tags origin` 确认目标版本 tag 是否已存在，再和用户确认新的发布版本号。
2. 把新版本统一写入上述 4 个文件后，先执行：
   - `pnpm typecheck`
   - `pnpm --filter @lazycat/desktop build:web`
   - `pnpm test`
3. 校验通过后，先提交版本变更，再执行 `pnpm release:all:win -- -Tag vX.Y.Z`，让脚本完成构建、推送 `main`、推送 tag 和 GitHub Release 上传。

**关键点**:
1. 发布前先查 tag 是否已存在，比等脚本跑到最后再失败更省时间。
2. 这个项目的正式发版版本源只有 4 处；只改其中一部分会被脚本直接拦下。
3. 只要要走 GitHub Release，发布脚本就要求干净工作区，因此“版本号修改”和“提交版本号”是正式发版链路的一部分。

**涉及文件**:
- `package.json`
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/src-tauri/tauri.conf.json`
- `scripts/release-all-win.ps1`
- `process.md`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`

**使用次数**: 0

## 2026-04-06: 项目管理甘特图周末日期坐标增加红色圆底

**场景**: 用户要求在项目管理甘特图的时间轴坐标里，为周末日期加一个红色圆底；同时明确不改已有周末整列淡色提示，只增强顶部日期数字本身。

**问题**:
1. `frappe-gantt` 的日期坐标不是 SVG 文本，而是渲染在 `.gantt-container` 内的绝对定位 `.lower-text` HTML 节点；如果按 SVG 选择器去做，会直接挂错层。
2. PM 甘特图存在 `render()`、`change_view_mode()`、`refresh()` 三条重建/重绘链路；如果只在首次渲染后补类，切视图或数据刷新后高亮会丢。
3. `Week` / `Month` 视图下的 `lower_text` 分别是周范围和月份文本，不能简单按类名里带日期就一律标红，否则会把“周起始日期”误当成周末日期。
4. `.lower-text` 默认宽度是列宽的 80%，直接给节点本身加背景会得到宽胶囊，而不是用户要的圆底。

**解决**:
1. 在 `pmGantt.ts` 新增 `shouldHighlightPmGanttWeekendLabel()` 纯函数，统一收口：
   - 仅 `viewMode === 'Day'` 时生效
   - 从类名中解析 `date_YYYY-MM-DD`
   - 非法日期或缺失日期类名时直接返回 `false`
2. `PmGanttView.vue` 新增 `syncGanttWeekendDateClasses()`，统一扫描 `.lower-text` 并切换 `pm-gantt-weekend-date` 类。
3. 将该同步逻辑接入三条链路：
   - 新实例 `renderGantt()` 后
   - `changeViewMode()` 后
   - `ganttInstance.refresh()` 后
4. 样式层使用 `::before` 伪元素在日期文本正中绘制固定 `24x24` 红色圆底，并通过 `isolation + z-index` 保证圆底在字后、不会扩成整格背景。

**关键点**:
1. `frappe-gantt` 头部日期坐标的可定制入口优先看 `.lower-text` / `.upper-text` 这类 HTML 节点，不要先入为主按 SVG 文本处理。
2. 时间轴头部装饰如果要跨 `render / refresh / change_view_mode` 稳定存在，最好抽成独立同步函数，和条目选中态一样走统一补丁链路。
3. 做“圆底数字”这类视觉增强时，不要直接给整块 header cell 上背景；更稳的是给文本节点本身加类，再用伪元素绘制固定尺寸圆底。
4. 周末识别规则最好放在纯函数里做单测，避免日期解析散在组件 DOM 代码中。

**涉及文件**:
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/utils/pmGantt.ts`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/pmGantt.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-06: 项目管理甘特图首次进入定位改为项目层无动画接管

**场景**: 用户要求按 `2026-04-06-pm-gantt-initial-scroll-design.md` 实现项目管理甘特图首次进入定位：保留“默认看今天附近”，去掉每次进入时从左向右的平滑滑动，并让 today 落在视口左侧约三分之一处。实现后又追加收口：虽然视口不再滚动，但任务条本身仍会从左往右长出来，也需要一起去掉。

**问题**:
1. `PmGanttView.vue` 每次进入都会新建 `frappe-gantt` 实例；若不覆盖配置，库会沿用默认 `scroll_to: 'today'`，重新触发一次 smooth scroll。
2. 现有组件已经在 `refresh()` 和 `change_view_mode(..., true)` 链路上做了“保持当前滚动位置”，如果把首次定位逻辑混进这些链路，容易误伤用户手动滚动后的视口。
3. 仅靠 PM 侧自己镜像日期差值算法并不稳，因为 `frappe-gantt` 在不同 view mode / infinite padding 下本身就有内部时间轴换算与 DOM 偏移。
4. 初次渲染时 `.gantt-container` 和 `.current-highlight` 可能晚一帧才稳定，若直接同步计算，容易出现“没定位到 today”或反复重试。
5. 即使关掉了视口滚动，`frappe-gantt` 仍会在 `bar.js` 里通过 SVG `<animate>` 把 `bar` / `bar-progress` 的宽度从 `0` 补到最终值，因此用户还会看到项目条目“从左往右出现”。

**解决**:
1. 新实例创建时显式传入 `scroll_to: 'start'` 和 `infinite_padding: true`，先确定性绕开库内部 `scrollTo({ behavior: 'smooth' })` 的 today 默认路径。
2. 在 `pmGantt.ts` 新增纯函数 `computePmGanttInitialScrollLeft`，只负责“today 左侧三分之一落点 + 0/max 边界钳制”，不感知 DOM。
3. `PmGanttView.vue` 中新增一次性初始定位状态：
   - 新实例创建后立即尝试读取 `.gantt-container` 与 `.current-highlight`
   - 若尺寸或高亮未就绪，仅补一次 `requestAnimationFrame`
   - 第二次仍不可用则直接放弃，保留 `scroll_to: 'start'` 的起始落点
4. `today` 坐标统一按真实 DOM 计算：
   - `currentX = (highlightRect.left - viewportRect.left) + viewport.scrollLeft`
   - 不再使用 `offsetLeft` 或手写日期差值镜像
5. 初始定位只在新实例创建后执行一次；`refresh()` 和 `change_view_mode(..., true)` 继续沿用现有滚动保持逻辑，不读取也不重置首次定位标记。
6. 对条目本身的默认 SVG 宽度动画，不去 patch 第三方包，而是在 `PmGanttView.vue` 渲染后统一移除 `.bar-group animate` 节点；新建实例、切视图和 `refresh()` 后都要执行一次，确保条目直接以最终宽度出现。

**关键点**:
1. 只要目标是“保留 today 语义但取消动画”，优先用 `scroll_to: 'start'` 把第三方默认滚动关掉，再由项目层直接写 `scrollLeft`，比 patch 依赖更稳。
2. 对第三方时间轴组件做定位时，优先读它已经渲染出来的真实锚点 DOM，而不是在业务层复制一套坐标算法。
3. “首次定位”必须和“同实例刷新/切视图的滚动保持”分开建模；前者是一次性初始化，后者是用户上下文保持，不能混在一个状态里。
4. 对首帧未稳定场景只补一帧重试即可，再多会引入闪动和状态复杂度；失败时安静降级到起始位置比持续纠偏更稳。
5. 如果用户反馈“已经不滚动了，但任务条还会自己展开”，优先检查第三方是否注入了 SVG `<animate>`，这和 CSS transition 不是一类问题。

**涉及文件**:
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/utils/pmGantt.ts`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/pmGantt.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-05: 项目管理状态筛选从甘特专用迁移为共享工具栏筛选

**场景**: 用户要求把项目管理里“当前甘特图至少筛选状态进行任务显示”的能力拓展到看板视图，同时把入口改成顶部工具栏里的多选下拉，并要求同步把“可视化辅助默认开启”写进 `AGENTS.md` / `CLAUDE.md`。

**问题**:
1. 仓库里已经存在一套 `pmGanttFilter.ts + PmGanttView.vue` 的甘特专用状态筛选实现，如果直接在看板里复制一套逻辑，会形成两份状态源和两套默认值。
2. `PmPanel.vue` 的看板列是按 `PM_STATUS_COLUMNS` 全量渲染，再靠列内数据为空显示空态；用户这次明确要求“未选中的状态列直接隐藏”，这会影响列渲染、`Sortable` 初始化和空态判断。
3. `PmGanttView.vue` 当前既负责甘特图自身交互，也负责状态筛选 UI；如果不把接口收口，后续看板和甘特会持续被“谁才是状态筛选入口”这个边界问题拖累。
4. `el-select multiple` 默认会在闭合态显示已选标签，但本次产品要求固定只显示“状态筛选”，不能把选中状态名露在工具栏里。

**解决**:
1. 将旧的 `pmGanttFilter.ts` 迁移为 `pmStatusFilter.ts`，把以下能力集中成 PM 共享 helper：
   - 默认状态集合
   - 多选切换 / 全选 / 清空
   - 稳定顺序数组输出
   - 未知状态按 `todo` 兜底
   - 看板可见列与状态分组
2. `PmPanel.vue` 中把 `ganttSelectedStatuses` 提升并改名为 `selectedStatuses`，然后拆出：
   - `baseFilteredItems`：搜索 / 类型 / 优先级
   - `statusFilteredItems`：在 `baseFilteredItems` 上叠加共享状态筛选
   - `visibleStatusColumns`：只渲染当前选中的状态列
3. `PmGanttView.vue` 移除内部状态 chip 与 `全选 / 清空`，只保留甘特自己的视图切换和统计信息；甘特图只接收父层已经筛好的 `items`。
4. 工具栏里的固定文案多选下拉通过“覆盖式标签”实现：
   - 外层放固定 `状态筛选` 文案
   - 内层 `el-select multiple` 继续负责真实选择
   - 用局部 CSS 隐掉默认标签和 placeholder
5. `Sortable` 初始化改为只遍历当前 `visibleStatusColumns`，并让 `setColumnRef()` 在列被隐藏时同步删除旧 ref，避免隐藏列残留拖拽实例。

**关键点**:
1. 这类“从单视图筛选升级为共享筛选”的任务，先迁 helper、再迁状态源、最后收口子组件接口，比直接在组件里局部打补丁稳得多。
2. 看板列一旦允许隐藏，`Sortable` 的初始化集合不能再硬编码为全部状态列，否则很容易绑定到已经卸载的 DOM。
3. 固定文案的多选下拉不一定要自定义整个浮层；如果只是想隐藏已选标签，给 `el-select` 做一层覆盖式标签和局部 CSS 就够了。
4. 未知状态兼容不能只做到“筛选命中”，还要做到“看板归列”；否则工作项会通过筛选但在列分组时消失。

**涉及文件**:
- `AGENTS.md`
- `CLAUDE.md`
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/utils/pmStatusFilter.ts`
- `apps/desktop/src/utils/pmStatusFilter.test.ts`
- `process.md`

**验证**:
- `pnpm test src/utils/pmStatusFilter.test.ts src/utils/pmGantt.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-04: 项目管理视觉统一规划的后半程收尾

**场景**: 用户要求继续执行 `2026-04-04-pm-visual-unification-design.md`，仓库里已经存在一半未提交实现：`PmPanel.vue` 模板和 `pmVisual.ts` / `pmVisual.test.ts` 已经开始改，但整体视觉还没有真正统一落地。

**问题**:
1. 当前断点不是“功能没接上”，而是“模板结构先改了，样式层还停留在旧时代”；`typecheck` 和 `build:web` 都能过，但新类名大量缺少配套 CSS，实际界面会呈现半成品状态。
2. `PmPanel.vue` 同时包含 scoped 样式和全局样式两段，侧栏/看板/详情在第一段，弹窗和 Teleport 相关样式在第二段；如果只在一处补样式，很容易出现“主页面好了，弹窗还是旧壳”。
3. 工作项编辑弹窗由 `el-dialog` Teleport 到 `body`，不能依赖只挂在 `.pm-panel` 上的局部 CSS 变量；弹窗样式若直接复用 scoped 区块里的变量，实际渲染时可能拿不到。
4. 这类 UI 收尾任务最容易被“编译通过”误导，必须额外检查“新模板类名是否真的有样式定义”，不能只看 TS 和构建结果。

**解决**:
1. 保留已有模板和前端派生逻辑，继续复用 `pmVisual.ts` 中的排序和标签摘要 helper，不回退已有中间成果。
2. 在 `PmPanel.vue` 的 scoped 样式中追加一整套视觉统一覆盖，集中补齐：
   - 左侧项目空间卡片化样式
   - 看板列与卡片的冷白蓝灰视觉
   - 详情侧栏的主信息卡、时间轨迹网格、资源关联行卡片
3. 在 `PmPanel.vue` 的全局样式块中补齐 Teleport 弹窗相关样式：
   - 顶部项目身份卡
   - 分区表单卡片
   - 三列核心信息栅格
   - 链接输入尾部动作区
   - 移动端收口
4. 验证时不能只跑新增 `pmVisual.test.ts`，还要把 PM 既有的日期、甘特、思源辅助测试一起跑一遍，避免 UI 收尾顺手打断现有 PM 派生逻辑。

**关键点**:
1. “规划执行一半”的 UI 任务，先看 `git diff` 和模板类名，再看编译结果；很多时候真正没完成的是样式层，而不是逻辑层。
2. 同一个组件里如果既有 scoped 样式又有 Teleport 场景，要先分清“谁负责页面内样式，谁负责弹窗/全局样式”，否则补丁会落在错误位置。
3. Teleport 内容不要依赖父容器上的局部 CSS 变量；更稳的做法是对弹窗区块使用硬编码设计色或全局变量。
4. 做视觉统一时，优先用“末尾覆盖样式”补齐新骨架，比大面积清洗旧 CSS 更稳，尤其适合已有一半中间改动的工作区。

**涉及文件**:
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/utils/pmVisual.ts`
- `apps/desktop/src/utils/pmVisual.test.ts`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop test src/utils/pmVisual.test.ts src/utils/pmDate.test.ts src/utils/pmGantt.test.ts src/utils/pmSiyuan.test.ts`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-04: 项目管理甘特图新增状态多选筛选

**场景**: 用户要求给项目管理甘特图增加状态筛选，状态可多选；最终确认不新增项目筛选，继续复用左侧项目 / 总览切换。

**问题**:
1. `PmPanel.vue` 里原本只有一层 `filteredItems`，同时服务看板和甘特；如果直接把状态筛选叠上去，会误伤看板四列数据。
2. `PmGanttView.vue` 之前把工具栏放在“有甘特条才显示”的分支里；一旦用户点击 `清空`，工具栏会跟空态一起消失，用户也就失去了恢复筛选的入口。
3. 状态筛选虽然只有 4 个按钮，但仍有三个容易被实现带偏的细节：
   - 未知状态运行时兼容
   - `selectedStatuses` 顺序稳定
   - 清空后保持空选择，而不是自动回填全选
4. 仓库当前没有现成的 `PmPanel` / `PmGanttView` 组件测试基础，如果把全部行为都压在组件内，回归只能靠手工点。

**解决**:
1. 新增 `pmGanttFilter.ts`，把甘特状态筛选规则抽成纯函数，集中处理：
   - 默认全选
   - 单个切换
   - 全选 / 清空
   - 未知状态按 `todo` 兜底
   - 输出稳定顺序数组
2. `PmPanel.vue` 将原 `filteredItems` 拆成：
   - `baseFilteredItems`：搜索 / 类型 / 优先级，继续供看板使用
   - `ganttFilteredItems`：在 `baseFilteredItems` 之上叠加状态筛选，只供甘特使用
3. `PmGanttView.vue` 新增 4 个状态按钮和 `全选 / 清空` 事件，并把工具栏从空态分支里拆出来，保证空态下仍可恢复筛选。
4. 测试策略优先纯函数：
   - `pmGanttFilter.test.ts` 负责筛选状态行为和未知状态兼容
   - `pmGantt.test.ts` 继续守住单边日期 / 非法日期的甘特排期语义

**关键点**:
1. 像“只作用于某个视图”的筛选，不要直接塞进所有视图共用的 `filteredItems`；最好先拆出共享基础筛选，再叠视图专用筛选层。
2. 允许用户 `清空` 筛选时，**工具栏绝不能挂在“有结果才显示”的分支里**，否则产品行为会自相矛盾。
3. 状态筛选如果对外用数组，必须稳定顺序、去重输出；否则 Vue diff、按钮选中态和测试断言都容易抖。
4. 在缺少组件测试基础时，优先把易错规则抽成纯函数单测，比直接堆在 SFC 内更稳。

**涉及文件**:
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/utils/pmGanttFilter.ts`
- `apps/desktop/src/utils/pmGanttFilter.test.ts`

**验证**:
- `pnpm test src/utils/pmGanttFilter.test.ts src/utils/pmGantt.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-04: 项目管理工作项新增外部链接字段与打开动作

**场景**: 用户要求在项目管理工作项中新增一个通用链接字段，可在编辑弹窗里维护，并支持从详情区与右键菜单直接打开。

**问题**:
1. `pm_items` 只有标题、描述、时间和思源页面缓存，没有独立的外部链接字段，无法承接 Jira、禅道、本地服务地址这类通用跳转目标。
2. 仓库里虽然已有 `vault:open-url` / `todo:open_link` 能力，但如果 PM 直接复用其他域的 action，会让领域边界变得模糊，后续维护时难以理解“为什么 PM 要走 Vault/Todo”。
3. 新增字段后，前端 `PmItem` 类型、弹窗草稿态、详情展示和测试 fixture 必须同步补齐；否则最容易在 `typecheck` 阶段被遗漏的测试桩卡住。
4. URL 输入允许用户只填 `localhost:3000` 这类地址，但又必须拒绝 `ftp://` 等非 `http/https` 协议，不能简单靠前端字符串拼接放过异常输入。

**解决**:
1. 在 `helpers.rs` 中为 `pm_items` 新增 `link_url TEXT DEFAULT NULL`，并追加 `ALTER TABLE pm_items ADD COLUMN link_url TEXT DEFAULT NULL` 迁移，兼容现有数据库。
2. 在 `pm.rs` 内新增 `normalize_item_link_url()`、`parse_item_link_url_value()` 和 `open_link()`：
   - 空值统一落库为 `NULL`
   - 未带协议时自动补 `http://`
   - 已带其他协议（如 `ftp://`）时直接拒绝
   - 打开动作仍由 PM 域自己暴露 `tool:pm:open-link`
3. `item_list` / `item_create` / `item_update` 同步接入 `link_url`，前端 `PmItem` 增加 `linkUrl`，`PmPanel.vue` 的工作项弹窗新增“链接”输入和“打开”按钮。
4. 右侧详情面板补“链接”展示行，右键菜单增加“打开链接”；这样用户既可以在编辑态测试，也可以在浏览态快速打开。
5. `pmGantt.test.ts` 的 `PmItem` fixture 需要同步补 `linkUrl: null`，否则 `PmItem` 新字段会直接打断全局 `typecheck`。

**关键点**:
1. 给已有业务表补字段时，前端类型、表单草稿、后端 CRUD、测试 fixture 要一口气补齐；否则 `build` 可能过了，`typecheck` 仍会被测试文件拦住。
2. “自动补协议”只能对无协议输入生效；如果用户已经输入 `xxx://`，必须先判断协议是否合法，不能盲目前缀成 `http://xxx://...`。
3. PM 这类业务域新增动作时，优先在本域增加 `open_link`，不要为了省几行代码直接跨域借用别的工具通道。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/pm.rs`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/types/pm.ts`
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path "E:/Projects/LazyCat/apps/desktop/src-tauri/Cargo.toml"`
- `cargo test --manifest-path "E:/Projects/LazyCat/apps/desktop/src-tauri/Cargo.toml" normalize_item_link_url -- --nocapture`
- `pnpm test src/utils/pmGantt.test.ts`

**使用次数**: 0

## 2026-04-02: 项目管理工作项弹窗切换为时间范围 + 思源紧凑列表

**场景**: 用户要求按 spec 落地项目管理工作项弹窗刷新，只改“新建/编辑工作项”弹窗，不扩散成整页 PM 重构，同时要把历史单边日期、带时间部分字符串和思源关联区的大卡片布局一起收敛。

**问题**:
1. `PmPanel.vue` 里时间输入还是两个独立 `el-date-picker`，并且直接用 `new Date('YYYY-MM-DD')` 做禁用与逾期判断，带来隐含时区解析和历史脏数据兼容问题。
2. PM 日期消费散落在看板卡片、详情区和甘特图，部分逻辑只会 `slice(0, 10)`，部分逻辑直接 `new Date(...)`，导致“表单、展示、逾期、甘特”难以保持同一套本地日期语义。
3. 思源关联区还是“主页面 / 附加页面”双分段卡片结构，纵向占位大；如果只在旧结构上删按钮，很难达到 spec 要求的“摘要头 + 统一紧凑列表”。
4. “关联页面”入口与“更换主页面 / 设为主页面”其实是两种不同意图：前者遇到已存在页面应提示并保持顺序不变，后者则允许页面提升为主页面；如果仍共用同一分支，很容易把重复选择错误地变成主页面提升。

**解决**:
1. 新增 `apps/desktop/src/utils/pmDate.ts`，集中提供：
   - `normalizePmDateString`
   - `parsePmDateAtLocalStart`
   - `parsePmDateAtLocalEnd`
   - `normalizePmDateRangeForDraft`
   - `formatPmDateRangeForDisplay`
   - `isPmItemOverdue`
2. `PmPanel.vue` 的工作项弹窗改成：
   - 标题单行
   - 类型 + 优先级同排
   - 状态单行
   - 单个 `daterange` 作为“时间安排”
   - 思源关联改为摘要头 + 统一行列表
3. 编辑历史数据时先在弹窗初始化层归一：
   - 单边日期映射为同日范围
   - 倒序日期自动升序
   - 非法日期按空处理
   保存时始终显式提交 `startAt/endAt`，避免继续把脏值带回后端。
4. 思源关联区引入“对话框意图”状态：
   - `link`：顶部 `关联页面`
   - `replace-primary`：主页面行的 `更换主页面`
   这样重复页面选择时才能正确区分“提示已存在”与“提升为主页面”。
5. 甘特图切到同一套 `pmDate` helper，确保未排期统计、历史带时间部分日期和逾期判断都走统一本地日期语义。
6. 新增 `pmDate.test.ts`，并补充 `pmGantt.test.ts` 场景，覆盖非法值、时间部分、单边日期、倒序日期与逾期边界。

**关键点**:
1. 只要输入语义是“本地日期”，就不要再写 `new Date('YYYY-MM-DD')`；必须先把值归一到 `YYYY-MM-DD`，再用 `new Date(year, monthIndex, day, ...)` 构造本地时间。
2. “关联页面”与“更换主页面”不能共用同一条选择后处理逻辑，否则重复选择时会把原本应该 no-op 的操作变成状态迁移。
3. 弹窗里的日期归一应只发生在草稿态；如果用户取消，不写回历史单边日期或脏数据的归一结果。
4. 统一列表想做到 44~52px 的紧凑高度，标题和路径都必须收敛成单行省略，不能继续沿用旧卡片的多行文本与分段标签。

**涉及文件**:
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/utils/pmDate.ts`
- `apps/desktop/src/utils/pmDate.test.ts`
- `apps/desktop/src/utils/pmGantt.ts`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/pmDate.test.ts src/utils/pmGantt.test.ts src/utils/pmSiyuan.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-04-02: Brainstorming 本地预览在 Windows 仓库内补齐桥接脚本

**场景**: brainstorming 过程中需要使用 visual companion 做本地浏览器预览，但仓库根目录没有 `scripts/start-server.sh` 入口，Windows 环境里也没有可直接用的 `bash`，导致技能文档里的默认启动方式无法在当前项目里直接落地。

**问题**:
1. 真正的预览服务脚本位于 `C:\\Users\\huahua\\.codex\\skills\\brainstorming\\scripts\\`，而不是仓库 `scripts/`；如果只按仓库相对路径调用，会误判成“能力缺失”。
2. Windows 环境里 `bash --version` 直接失败，不能假设 skill 里的 `.sh` 脚本能在本机无缝执行。
3. Node 预览服务默认随机高位端口时，Windows 可能命中不可绑定端口段，触发 `listen EACCES: permission denied 127.0.0.1:<port>`。
4. 仓库当前 `.gitignore` 忽略 `.superpowers/`，但历史上又有被跟踪的 brainstorming 产物；如果提交前不核对暂存区，容易把旧 session 删除误带进 commit。

**解决**:
1. 在仓库 `scripts/` 下补齐桥接入口：
   - `start-server.ps1`
   - `stop-server.ps1`
   - `start-server.sh`
   - `stop-server.sh`
2. `ps1` 桥接脚本负责：
   - 从 `$CODEX_HOME` 或 `~/.codex` 定位真正的 skill 脚本
   - 在项目 `.superpowers/brainstorm/` 下创建 session 目录
   - 先探测一个可实际监听的空闲端口，再把 `BRAINSTORM_PORT` 注入给 `server.js`
   - 读取 `.server-info` 作为启动成功判据
3. Windows 下优先使用 `powershell -ExecutionPolicy Bypass -File .\\scripts\\start-server.ps1 -ProjectDir .`；`.sh` 入口只保留给兼容 skill 约定的调用方。
4. 提交前必须显式执行：
   - `git status --short`
   - `git diff --cached --name-status`
   确认没有把 `.superpowers/` 里的历史跟踪文件删除误带进提交；若误删已进入提交，需要先从上一提交恢复，再用 `git add -f` 明确把这些跟踪文件补回。

**关键点**:
1. PowerShell 参数不能命名为 `Host`，否则会和只读的 `$Host` 变量冲突；桥接脚本里应使用 `BindHost` 之类的名字。
2. 预览服务是否真的可用，不能只看进程有没有启动；至少要同时检查 `.server-info` 和 `Invoke-WebRequest http://localhost:<port>` 是否返回 200。
3. 对依赖关系明确的 Git 操作（`add -> status -> commit`）不要并行执行，容易因为时序问题误判暂存区状态。

**涉及文件**:
- `scripts/start-server.ps1`
- `scripts/stop-server.ps1`
- `scripts/start-server.sh`
- `scripts/stop-server.sh`

**验证**:
- `powershell -ExecutionPolicy Bypass -File .\\scripts\\start-server.ps1 -ProjectDir .`
- `Invoke-WebRequest -UseBasicParsing http://localhost:<port>`

**使用次数**: 0

## 2026-04-02: 项目管理思源页面关联弹窗切换为默认位置列表优先

**场景**: 用户要求按新 spec 重做项目管理中的“关联思源页面”弹窗，默认打开就展示当前有效位置下的全部文档，输入改为本地过滤，只有手动点击“扩展到全库”时才发起远端搜索。

**问题**:
1. `PmPanel.vue` 之前把页面关联弹窗建模成“搜索框 + 搜索结果”，打开时会预填标题并自动走远端搜索，和当前位置列表优先的交互目标冲突。
2. 默认位置列表其实已经能从思源目录树缓存派生，但仓库里缺少“按位置定位目录子树并拍平成页面列表”的纯函数，导致组件层很难稳定处理根目录、父文档、位置失效和空目录等状态。
3. 目录刷新与全库搜索是两条不同链路，如果仍复用同一套 `searchResults` 状态，很容易出现“全库结果覆盖当前位置状态”或“输入变化后还停留在旧全库结果”的串态问题。

**解决**:
1. 在 `pmSiyuan.ts` 中新增 `collectPmSiyuanPagesForLocation()` 与 `filterPmSiyuanPages()`，把“位置定位 + 子树拍平 + 本地过滤 + 无效位置/空目录判定”都下沉到纯函数。
2. `PmPanel.vue` 的页面关联弹窗改成双数据源状态：
   - `location`：当前位置完整列表 + 本地过滤
   - `all`：一次性手动触发的全库搜索结果
3. 打开弹窗时固定重置为 `location` 模式并清空输入；若已有目录缓存则先直接派生默认列表，再后台静默刷新目录；若无缓存则先加载目录再展示。
4. 输入框不再触发远端请求；只有点击“扩展到全库”时才调用 `tool:pm:siyuan-search-pages`，且 `all` 视图下继续修改输入会立即退出回 `location`。
5. 增加“返回当前位置列表”入口、位置状态空态文案和目录刷新失败的轻量提示，同时在 `pmSiyuan.test.ts` 补齐根目录、父文档、回退位置、本地过滤、位置失效与空目录测试。

**关键点**:
1. 这种“默认列表 + 手动扩展搜索”的弹窗，不要再把当前位置列表和远端搜索结果塞进同一组状态；至少要明确区分当前展示源，否则输入、刷新和返回操作很容易互相污染。
2. 思源目录树到页面列表的派生必须走纯函数，组件只保存“最新位置状态 + 当前展示源”，这样目录静默刷新时才能在 `all` 模式下继续后台更新 `location` 快照。
3. 目录刷新失败时，如果已经有上一次成功解析的位置结果，应该保留旧列表并给轻量提示；只有首次加载且没有任何位置结果时，才进入主体错误空态。

**涉及文件**:
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/utils/pmSiyuan.ts`
- `apps/desktop/src/utils/pmSiyuan.test.ts`

**验证**:
- `pnpm test src/utils/pmSiyuan.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-30: 项目管理思源存储位置选择器树节点错乱与轻量目录选择器重构

**场景**: 用户反馈项目管理里“绑定项目专属存储位置”弹窗显示很乱，树节点标题与路径重叠、层级难看清，希望排查原因并把交互一起优化。

**问题**:
1. `PmPanel.vue` 的位置选择弹窗给 `el-tree` 自定义节点塞了“标题 + 路径”两行内容，但没有同步适配 `el-tree-node__content` 高度和对齐，导致长目录树下出现文本重叠和错位。
2. 目录树默认 `default-expand-all`，笔记本、文档、路径、数量一次性铺开，信息密度太高，导致即使没有 CSS bug 也很难快速定位目标。
3. “当前选择”只有一行文案，没有独立选中卡片和搜索辅助，用户点击树节点后仍然很难快速确认自己到底选中了哪里。

**解决**:
1. 在 `pmSiyuan.ts` 中补充位置选择器辅助纯函数：目录树过滤、搜索态展开 key 计算、位置目标文案和路径文案格式化。
2. `PmPanel.vue` 的位置选择弹窗重做为“搜索 + 当前选择卡片 + 默认折叠树”的轻量目录选择器：
   - 顶部增加搜索框和当前选择卡片
   - 树默认只展开笔记本一级
   - 搜索时仅保留命中的节点和祖先路径，并自动展开必要分支
   - 完整路径从树节点主体中移走，集中在当前选择卡片里展示
3. 树节点改为单行标题优先展示，并通过 `:deep(.el-tree-node__content)` 覆盖最小高度、hover、current 状态和标签区宽度，彻底消除原来的重叠。
4. 顺手补齐思源配置卡片、页面关联列表和详情区的样式，让 PM 内思源相关 UI 视觉上统一，不再是多块临时样式拼接。
5. 在 `pmSiyuan.test.ts` 中补充过滤树、展开 keys 与位置展示文案测试，避免后续再改树结构时回归。

**关键点**:
1. 给 `el-tree` 做自定义多行节点时，不能只改 slot 模板，必须同步检查 `el-tree-node__content` 的高度和对齐方式，否则很容易出现“节点内容高度没长，文本却长了”的重叠 bug。
2. 目录选择场景不要把路径信息塞到每个树节点里。更稳的做法是“树里看标题，选中区看完整路径”，让信息按任务拆层。
3. 搜索目录树时，前端直接派生一棵过滤树比依赖组件内部 filter 更容易控制“祖先保留 + 搜索态展开 + 清空后恢复默认折叠”这整套行为。

**涉及文件**:
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/utils/pmSiyuan.ts`
- `apps/desktop/src/utils/pmSiyuan.test.ts`

**验证**:
- `pnpm test src/utils/pmSiyuan.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-29: 项目管理接入思源配置与目录树预览首版

**场景**: 用户希望在项目管理面板内新增思源设置，先完成第一版：配置本地思源地址和 API Token，验证连接，并读取“笔记本 + 文档树”目录数据。

**问题**:
1. 项目管理当前只有本地 PM 数据，没有任何外部知识库/文档系统接入入口。
2. 配置层已有 `user_settings`，但缺少 PM 场景内的局部设置 UI 和对应后端 action。
3. 思源 API 虽有官方接口可查版本、列笔记本和执行 SQL，但需要在 Rust 侧统一处理地址归一化、鉴权失败、超时、标准响应解析和目录树构建，不能把这些细节丢给前端。

**解决**:
1. 在 `PmPanel.vue` 顶部工具栏新增“思源设置”按钮，使用 `el-drawer` 承载地址、Token、保存、测试连接、加载目录和树预览；配置通过 `useSettings` 持久化到 `user_settings`。
2. 在 `bridge/tauri.ts` 增加 `tool:pm:siyuan-test` 与 `tool:pm:siyuan-directory` 两条通道，在 `types/pm.ts` 中补齐目录树类型定义，前端只负责展示和局部状态管理。
3. 在 `pm.rs` 中新增思源 helper：地址归一化、设置兜底读取、HTTP POST 封装、思源标准响应解析、401/403 与业务错误分类、`lsNotebooks + query/sql` 查询，以及按 `hpath` 构造树。
4. 目录树第一版使用 `blocks` 表中 `box / path / hpath / content` 查询文档节点，并在 Rust 内存中按路径分段逐层插入，最终返回前端可直接渲染的树结构。
5. 补充 Rust 单测覆盖 URL 归一化与目录树嵌套构建，减少后续重构时的回归风险。

**关键点**:
1. 思源配置虽然属于“设置”，但第一版放在 PM 面板内更贴近业务场景；后端 action 仍应归在 `pm` 域，而不是塞进通用 `settings` 域。
2. `user_settings` 足够承接这类轻量配置；若只是地址和 Token，不需要为了第一版额外建表。
3. 思源标准响应的错误不能只看 HTTP 状态码，还要看 JSON 中的 `code/msg`；连接失败、鉴权失败和业务错误要分开提示。
4. 前端刷新目录失败时应保留上一次成功树，避免一次请求失败把整个预览区清空。
5. 第一版目录树基于 `query/sql` 的字段假设构建，后续若思源版本字段结构变化，优先在 Rust 侧兜底或报出明确兼容性错误。

**涉及文件**:
- `docs/superpowers/specs/2026-03-29-pm-siyuan-integration-v1-design.md`
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/types/pm.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src-tauri/src/tools/pm.rs`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path "E:/Projects/LazyCat/apps/desktop/src-tauri/Cargo.toml"`
- `cargo test --manifest-path "E:/Projects/LazyCat/apps/desktop/src-tauri/Cargo.toml" siyuan_`

**使用次数**: 0

## 2026-03-29: 项目管理甘特图悬浮卡越界与右键视口重置修复

**场景**: 用户反馈项目管理甘特图在底部任务上悬浮详情时会被容器裁切，且右键任务打开菜单后甘特图视口会跳回默认位置。

**问题**:
1. `frappe-gantt` 默认 popup 只按 `left = x + 10`、`top = y - 10` 粗放定位，不会根据容器当前滚动视口做边界翻转，底部任务的详情卡容易被 `gantt-container` 裁掉。
2. `PmGanttView.vue` 里 `ganttTasks` 之前把 `selectedItemId` 也作为依赖，右键选中任务时会触发一次整图 `refresh()`。
3. `frappe-gantt` 的实际滚动容器是内部 `.gantt-container`，不是外层 `ganttRef`；之前即便尝试保留滚动位置，也读写错了元素。

**解决**:
1. 在 `utils/pmGantt.ts` 新增 `clampPmGanttPopupPosition()`，统一计算 popup 在当前视口内的左右/上下翻转与边距钳制，并补充单测覆盖底部和右侧越界场景。
2. `PmGanttView.vue` 改为通过 `MutationObserver` 观察 `.popup-wrapper` 的显隐与内容变化，在每次显示后按内部 `.gantt-container` 的 `scrollLeft/scrollTop/clientWidth/clientHeight` 重算 popup 位置。
3. 将甘特条“选中态”从 `ganttTasks` 计算链路里剥离，改为单独 watch `selectedItemId` 并只同步 class，避免右键时无谓刷新。
4. 甘特图数据确实需要 `refresh()` 时，先读取内部 `.gantt-container` 的视口位置，并临时关闭 `frappe-gantt` 的 `scroll_to` 自动定位，再在下一帧恢复滚动条。
5. 内部滚动事件改为直接绑定 `.gantt-container`，确保滚动时能及时关闭 popup 和右键菜单。

**关键点**:
1. `frappe-gantt` 的外层宿主元素只是挂载点，真正滚动的是内部自建容器；凡是涉及视口恢复、滚动监听、popup 可见区判断，都必须基于 `.gantt-container`。
2. “选中态变化”不能混进“任务数据变化”刷新链路，否则右键、点击切换详情都会触发整图重绘，带来滚动抖动和定位回跳。
3. 第三方库的 popup 若无法直接配置碰撞检测，优先在项目层补一层纯函数定位和 DOM 观察，不要急着 fork 依赖。

**涉及文件**:
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/utils/pmGantt.ts`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/pmGantt.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-29: 项目管理甘特图交互增强与甘特条右键菜单

**场景**: 用户希望优化项目管理甘特图视图，并为甘特图条目补上右键菜单，让甘特图和现有看板卡片的快捷操作能力对齐。

**问题**:
1. `PmGanttView.vue` 之前只有 `点击选中 + 拖动改日期`，没有悬浮信息、双击编辑，也没有甘特条自己的右键菜单。
2. `frappe-gantt` 暴露给项目的类型声明很薄，缺少 `popup`、`on_double_click`、`change_view_mode(..., maintain_pos)` 等当前实现会用到的钩子描述。
3. `PmPanel.vue` 之前只给看板卡片和左侧项目列表做了右键菜单，菜单定位还是硬编码近似值；甘特图如果直接复用旧入口，目标选中态和菜单目标容易不同步。

**解决**:
1. 在 `PmGanttView.vue` 中增加悬浮详情卡、双击编辑、甘特条 `contextmenu` 事件代理、滚动时关闭浮层，并把总览模式下的项目元信息一并透给 popup。
2. 新增 `utils/pmGantt.ts` 与 `pmGantt.test.ts`，把甘特任务映射、未排期统计、逾期/置顶/选中 class 组装、悬浮卡 HTML 生成收敛成纯函数。
3. 将菜单坐标钳制能力抽为通用 `utils/contextMenu.ts`，`PmPanel.vue` 和 `TodoPanel.vue` 共用；`PmPanel.vue` 的项目管理右键菜单统一接入 `Esc / 外部点击 / scroll / resize / 再次右键` 关闭规则。
4. `PmPanel.vue` 中抽出统一的工作项菜单动作构造逻辑，甘特条和看板卡片共用 `编辑 / 置顶或取消置顶 / 推进状态 / 删除`，并在打开菜单前同步切换当前选中工作项。
5. 补齐 `frappe-gantt.d.ts` 的 popup / 双击 / 视图切换类型，避免甘特图实现继续依赖隐式 `any`。

**关键点**:
1. 甘特图库的右键菜单不要强依赖原生 `MouseEvent` 透传；对子组件发 `{ item, anchorX, anchorY }` 这类稳定 payload，更利于父层复用菜单定位逻辑。
2. 甘特图条目的“选中态”最好作为任务 class 的一部分统一进入 `refresh()` 链路，不要在父层和子层各维护一套视觉状态。
3. 菜单定位不要继续靠 `actions.length * 34` 这类散落硬编码；抽成纯函数后，Todo/PM 两边能共享同一组边界测试。

**涉及文件**:
- `apps/desktop/src/components/PmGanttView.vue`
- `apps/desktop/src/components/PmPanel.vue`
- `apps/desktop/src/types/frappe-gantt.d.ts`
- `apps/desktop/src/utils/contextMenu.ts`
- `apps/desktop/src/utils/contextMenu.test.ts`
- `apps/desktop/src/utils/pmGantt.ts`
- `apps/desktop/src/utils/pmGantt.test.ts`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop test src/utils/contextMenu.test.ts src/utils/pmGantt.test.ts`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-03-20: 密码库解锁顺滑度优化首轮落地

**场景**: 用户希望密码库在输入正确主密码后更快出现可用主界面，同时首轮只做前端体感优化与后端低风险性能优化，不调整 PBKDF2 参数、不改现有 vault IPC 协议。

**问题**:
1. `VaultLockScreen.vue` 之前使用 500ms debounce + 按密码长度去重，自动解锁会产生固定空等，且自动/手动请求状态彼此割裂。
2. `VaultPanel.vue` 之前要等 `loadEntries()` 和 `loadTagStats()` 串行完成才真正显示主界面，首屏还叠加 `out-in` 过渡和列表逐项延迟动画，放大了“解锁后还在等”的体感。
3. `helpers.rs` 每次开库都会重新解析 `config.json` 且执行完整 schema 初始化；`vault.rs` 的 `cmd_list()` 对 tags 存在 N+1 查询，`cmd_touch()` 在高频续活路径里重复开库读取配置。

**解决**:
1. 在 `VaultLockScreen.vue` 中保留 `onUnlock()` / `attemptAutoUnlock()` 入口，内部统一走 `runUnlockAttempt()`；自动解锁改为 150ms debounce + 按密码值去重，并允许真并发请求，只有最近一次手动失败才回写错误。
2. 在 `VaultPanel.vue` 中把“已解锁”和“数据已加载”拆成两个阶段：先切 unlocked 主界面，再后台 `loadEntries({ phase })`，列表成功后 fire-and-forget 触发 `loadTagStats({ phase })`；首屏补了轻量 loading / 最小重试 UI，并用 generation + request token 丢弃旧结果。
3. 去掉锁屏到主界面的 `mode="out-in"` 和列表逐项 `animationDelay`，保留必要但不阻塞首屏可用性的轻量动效。
4. 在 `helpers.rs` 中把 `get_data_dir()` 做进程级缓存，并把数据库初始化拆成“每连接执行 `PRAGMA foreign_keys = ON`”与“进程首次连接执行 schema/FTS/seed 初始化”。
5. 在 `vault.rs` 中为 `cmd_list()` 增加批量 tags 查询映射，避免逐条查 tags；`cmd_touch()` 只更新 session 的 `last_activity`，不再走热路径数据库 I/O。

**关键点**:
1. 真并发 unlock 的收敛规则要清晰：任意一次成功即可进入 unlocked，旧失败不能覆盖新状态，自动失败继续静默。
2. `VaultPanel.vue` 的首次加载和普通刷新必须分开：首次加载允许显示占位与最小错误 UI，普通刷新保留旧列表和旧标签，不回退首屏态。
3. 旧异步结果一定要做代际保护，否则锁定后晚到的 `list/tag-stats` 或连续刷新里的旧结果会回写新页面。
4. `PRAGMA foreign_keys = ON` 属于连接级设置，不能跟 schema 一次化一起粗暴搬走；schema 初始化失败也不能缓存失败结果。
5. 已解锁会话内的锁定策略要和后端 session 保持同一口径：当前会话冻结既有策略，下一次 setup/unlock/change_password 再刷新。

**涉及文件**:
- `apps/desktop/src/components/VaultLockScreen.vue`
- `apps/desktop/src/components/VaultPanel.vue`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/vault.rs`
- `docs/superpowers/specs/2026-03-20-vault-unlock-smoothness-design.md`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path "E:/Projects/LazyCat/apps/desktop/src-tauri/Cargo.toml"`


## 2026-03-20: 主呼出快捷键优先按剪贴板路径打开资源管理器

**场景**: 用户希望按主呼出快捷键后，如果当前剪贴板内容本身是本地文件/目录路径，则优先直接打开系统文件浏览器；文件要定位选中，目录要直接打开，且不能影响 snippets / vault / todo 等命名快捷键导航。

**问题**:
1. 现有主快捷键 `toggle` 只负责呼出窗口，没有给前端一个仅属于主呼出场景的处理入口。
2. 前端现有 `clipboard-detect.ts` 只做 JSON/JWT 等内容识别，没有独立的保守路径归一化能力。
3. `inbox.rs` 的 `action_open_path()` 在 `reveal=true` 时仅打开父目录，Windows 下不会在资源管理器中选中文件。

**解决**:
1. 在 `main.rs` 的主快捷键 Reveal 分支中，仅对 `toggle` 额外发出 `main-window-toggle` 事件；命名快捷键仍继续走 `hotkey-navigate`，避免语义串线。
2. 在 `clipboard-detect.ts` 新增 `detectClipboardPath()`，只接受单行绝对路径 / UNC / file URI，支持一层外层引号，拒绝环境变量、多行和明显命令片段。
3. 在 `App.vue` 监听 `main-window-toggle`，读取 `navigator.clipboard.readText()` 后优先调用 `invokeToolByChannel("tool:inbox:open-path", { path, reveal })`；命中成功即短路，失败则静默回退原行为。
4. 在 `inbox.rs` 中保留现有存在性校验与目录打开逻辑，仅对 Windows + `reveal=true` + 文件路径改为 `explorer.exe /select,...`，实现真正的文件定位。
5. 增加 `clipboard-detect.test.ts`，覆盖文件路径、目录路径、引号、file URI、UNC 与拒绝样例。

**关键点**:
1. 主呼出与命名快捷键必须分事件处理，不能把 `toggle` 混进现有导航语义，否则容易误触发工具切换。
2. 前端路径检测应保持保守，只做格式归一化；路径是否真实存在仍交给 Rust 侧统一校验，避免误判普通文本。
3. 判断 `reveal` 不应依赖“是否含扩展名”，否则像 `C:\Windows\System32\drivers` 这类无扩展名文件会被误当目录；更稳妥的做法是“非根路径且不以斜杠结尾则 reveal=true”，再由后端基于真实文件系统类型分流。
4. Windows 文件定位优先走资源管理器原生 `/select,` 语义，目录和非 Windows 平台继续复用 `open::that(...)`，改动最小且兼容性最好。

**涉及文件**:
- `apps/desktop/src/App.vue`
- `apps/desktop/src/utils/clipboard-detect.ts`
- `apps/desktop/src/utils/clipboard-detect.test.ts`
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src-tauri/src/tools/inbox.rs`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`

**使用次数**: 0

---

## 2026-03-20: 本地待办最近一周已办改为真实完成时间 + 过去 7 天口径

**场景**: 用户反馈“任务清单”里的“最近一周已办”不符合预期，要求按过去 7 天滚动窗口计算，且已办归类必须基于真实完成时间，不能被完成后的再次编辑影响。

**问题**:
1. `TodoPanel.vue` 当前走 `item_list -> groupTodoItemsByBucket` 的前端分桶链路，但 `todoBuckets.ts` 之前用 `updatedAt || createdAt` 近似完成时间，完成后再编辑会把事项错误挪进/挪出“最近一周已办”。
2. 旧“最近一周”口径是“从最近周五开始”，不是严格过去 7 天窗口，导致边界日期与用户直觉不一致。
3. `todo_items` 表没有独立 `completed_at` 字段，后端列表接口也就无法给前端提供稳定的真实完成时间。

**解决**:
1. 在 `helpers.rs` 增加 migration 27：为 `todo_items` 新增 `completed_at`，给历史 `status='completed'` 数据用当前 `updated_at` 回填一次，并创建索引。
2. `todo.rs` 的 `item_change_status()` 改为：状态切到 `completed` 时仅在 `completed_at` 为空时写入 `CURRENT_TIMESTAMP`；切回非完成态时清空 `completed_at`，同时保持原有 `updated_at` 刷新语义。
3. `todo.rs` 的 `item_list()` 新增返回 `completedAt`，前端 `TodoItem`、`normalizeTodoItem()`、`relativeDoneTimeLabel()` 全链路改为消费真实完成时间。
4. `todoBuckets.ts` 改为以 `completedAt` 做已办分桶与倒序排序，“最近一周”起点收口为“今天向前 6 天的零点”；缺少 `completedAt` 的旧数据统一落到较早已办，避免继续被 `updatedAt` 污染。
5. `todoBuckets.test.ts` 补充“编辑已完成事项不会改变分桶”“缺少 completedAt 的已办不进入最近一周”等回归用例。

**关键点**:
1. `updated_at` 仍然应该保留“最后编辑时间”语义，不要再把它混用成完成时间，否则后续任何已完成事项编辑都会再次破坏分桶稳定性。
2. 对历史数据无法恢复真实完成时刻时，宁可保守回填一次并让后续新数据准确，也不要继续在前端做 `updatedAt` fallback。
3. 这类时间口径修正优先沿用现有列表接口和前端分桶入口，只替换真源字段与边界规则，能显著降低无关回归。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/todoBuckets.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml todo:: -- --nocapture`（本次被与改动无关的既有 `vault.rs` 测试编译错误阻塞）

**使用次数**: 0

## 2026-03-18: 本地待办卡片右键菜单落地

**场景**: 用户要求给本地待办的待办卡片补上右键菜单，支持 `置顶/完成/删除/编辑任务时间`，并保持与现有右侧详情区行为一致。

**问题**:
1. `TodoPanel.vue` 原本只有点击选中、双击编辑，置顶/完成/删除都集中在右侧详情区，列表卡片没有任何上下文菜单状态。
2. 待办右键菜单如果直接写死坐标，靠近窗口右下角时会出屏；仓库里虽然有多处自绘菜单，但 Todo 没有复用层。
3. “编辑任务时间”不能新开一条临时保存链路，否则会和现有 `ensureDetailCanLeave()`、右栏编辑态、重复事项删除范围确认产生交叉行为。

**解决**:
1. 在 `TodoPanel.vue` 的待办卡片补 `@contextmenu.prevent`，右键时先走 `ensureDetailCanLeave()`，再切到目标事项的详情查看态并打开菜单。
2. 新增 `apps/desktop/src/utils/todoContextMenu.ts`，抽出纯函数 `clampContextMenuPosition()` 统一处理菜单坐标钳制；`todoContextMenu.test.ts` 覆盖正常位置、右下出屏、左上边距和菜单大于视口四个核心场景。
3. 菜单浮层通过 `Teleport to="body"` 渲染，统一支持点击外部关闭、再次右键关闭、`Esc` 关闭、列表滚动关闭和组件卸载清理监听。
4. 菜单动作全部复用现有链路：`toggleItemPin()`、`changeItemStatus()`、`deleteItem()`，`编辑任务时间` 新增 `enterEditTimeMode()`，内部继续复用 `enterEditMode()`，并强制展开“日期与时间”区域后滚动定位和聚焦首个输入框。

**关键点**:
1. 右键目标切换前必须先处理脏编辑态；否则从创建态/编辑态直接右键其它事项，会把旧草稿和新菜单操作混在一起。
2. “编辑任务时间”最好只是**进入现有编辑态并定位字段**，不要额外做轻量弹窗或独立保存逻辑，这样可以继续复用现有校验、5 分钟刻度和保存语义。
3. Todo 的右键菜单不值得先抽公共组件；先在面板内局部落地，真正抽象的只有坐标钳制纯函数，避免把 UI 生命周期和业务动作耦到一起。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoContextMenu.ts`
- `apps/desktop/src/utils/todoContextMenu.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/todoContextMenu.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-18: 本地待办 meta-time 跨自然周文案修复

**场景**: 用户反馈任务清单中 `meta-chip / meta-time` 的相对日期文案有误。对于跨到下一自然周的事项，当前仍显示为 `周X`，例如本周设置下周的事项时应显示 `下周X`。

**问题**:
1. `TodoPanel.vue` 的 `relativeDateTimeLabel()` 写在组件内部，只按 `diffDays` 判断，没有按“周一开始”的自然周边界判断。
2. 当前逻辑会把“下周但不足 7 天”的日期误显示为 `周X`，而“刚好 +7 天”的日期又会直接退回绝对日期，表现不稳定。
3. 相对日期逻辑没有独立测试，涉及 `今天 / 明天 / 昨天 / 周X` 的文案调整时容易回归。

**解决**:
1. 新增 `apps/desktop/src/utils/todoRelativeDate.ts`，把相对日期文案提炼为纯函数 `formatTodoRelativeDateTimeLabel()`，统一处理 `今天 / 明天 / 昨天 / 周X / 上周X / 下周X / 绝对日期`。
2. 周边界改为按周一开始的自然周判断；相邻日计算使用 `setDate()`，避免继续依赖固定 `86400000` 毫秒偏移。
3. 对纯日期字符串 `YYYY-MM-DD` 做本地日期解析，避免被 `new Date()` 当作 UTC 导致跨天、跨周误判。
4. `TodoPanel.vue` 改为复用新 util，并删除未使用的 `itemTimeLabel()` 死代码。
5. 新增 `apps/desktop/src/utils/todoRelativeDate.test.ts`，覆盖同周、上周、下周、跨年回退和纯日期字符串等边界。

**关键点**:
1. 这类文案问题本质不是“差几天”，而是“是否跨自然周”；只看 `diffDays` 很容易把周三到下周一这种场景判错。
2. `今天 / 明天 / 昨天` 要高于 `上周 / 下周`，否则周日看次日周一会被展示成 `下周一`，不符合直觉。
3. 相对日期逻辑适合抽成 Todo 专用 util，不要塞进 `todoSchedule.ts` 或 `todoBuckets.ts`，否则职责会混在一起。

**涉及文件**:
- `apps/desktop/src/utils/todoRelativeDate.ts`
- `apps/desktop/src/utils/todoRelativeDate.test.ts`
- `apps/desktop/src/components/TodoPanel.vue`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/todoRelativeDate.test.ts`
- `pnpm --filter @lazycat/desktop typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-17: 收纳箱图片预览、右键菜单与图片回采抑制

**场景**: 用户要求在收纳箱图片详情里支持点击放大预览，并在右键菜单中提供复制图像、打开图像、打开图像位置、复制图像路径等常用操作。

**问题**:
1. 收纳箱详情里的图片已有 `payloadDataUrl` 和 `openPath`，但正文区只有静态展示，没有预览层和局部菜单交互。
2. 复制图像如果直接写回系统剪贴板，后台采集线程会把这张图再次录回历史流，形成“自复制回采”。
3. 前端右键菜单需要挂在图片正文和预览图上，同时还不能破坏现有三栏布局、滚动和详情切换体验。

**解决**:
1. 在 `InboxPanel.vue` 内直接加 `Teleport to="body"` 的预览层与自定义右键菜单，正文图和预览图共用一套菜单状态；关闭规则统一为遮罩点击、`Esc`、点击空白、滚动和窗口 resize。
2. 菜单动作固定收敛为 4 项：`复制图像 / 打开图像 / 打开图像位置 / 复制图像路径`；文件类动作统一走现有 `tool:inbox:open-path`，路径复制先走 `suppressClipboardCapture` 再写文本剪贴板。
3. Rust 侧在 `inbox.rs` 新增 `copy_image` action，Windows 下把图片文件解码后按 `CF_DIB` 写入系统剪贴板；抑制逻辑从“只压文本”升级为“按内容哈希压制”，文本和图片都共用同一份一次性抑制队列。

**关键点**:
1. 收纳箱图片详情不需要额外改表或改详情接口，现有 `payloadDataUrl + openPath + canOpenPath + metaJson.width/height` 已经够支撑预览和右键操作。
2. 图片回采抑制不能直接拿原文件字节做哈希；要按写入剪贴板后的像素内容重新编码 PNG 再算哈希，才能和后台 `read_image()` 重新采样出的 `content_hash` 对上。
3. 自定义右键菜单最稳妥的复用方式是沿用 `TabBar/VaultPanel` 那套 `reactive({ visible, x, y }) + Teleport + document click` 模式，不要额外引入新菜单依赖。

**涉及文件**:
- `apps/desktop/src/components/InboxPanel.vue`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src-tauri/src/tools/inbox.rs`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`
- `cargo check`

**使用次数**: 0

## 2026-03-17: 收纳箱（Inbox Hub）首版打通与跨工具草稿联动

**场景**: 需要按设计稿一次性落地收纳箱首版，包括 Rust 端剪贴板采集、SQLite 落库/搜索、前端三栏面板，以及转 Todo / Vault 的跨工具草稿联动。

**问题**:
1. Windows 剪贴板读取链路涉及 `windows-sys` 的 `HANDLE/HGLOBAL`、GDI 位图和注册格式，常量和句柄类型一旦用错，`cargo check` 会直接卡死在编译层。
2. 收纳箱面板需要同时承载筛选、分页、详情、元数据编辑和跨工具转入，如果直接把全部列表节点渲染到 DOM，历史量一大就会拖垮前端滚动体验。
3. Todo / Vault 不能只接字符串草稿；Todo 需要“首行标题、剩余正文进描述”，Vault 需要保守启发式预填并在复制敏感内容前抑制剪贴板回流。

**解决**:
1. Rust 端新增 `inbox` 工具域、migration 26 和主线程剪贴板轮询；Windows 剪贴板层统一改用正确的 `HANDLE`/`HGLOBAL` 调用签名，并用固定 `CF_*` 值避开 `windows-sys` feature 差异。
2. 前端新增 `InboxPanel.vue`，采用左栏筛选 + 中栏虚拟滚动摘要列表 + 右栏详情/动作的三栏布局，列表分页 50 条并通过固定行高虚拟化控制渲染节点数。
3. `useClipboardSuggestion` 的结构化 `PendingToolInput` 被接到 Todo / Vault：Todo 复用显式 `todoDraft` 并兜底文本拆分；Vault 在面板层做 URL/地址/端口/数据库关键词/显式标签行的保守解析，原文完整保留到备注；Vault 所有复制动作在写入系统剪贴板前先调用 `suppressClipboardCapture`。
4. 为避免验证噪音，顺手清理了 `vault.rs` 里既有的未使用 `Duration` warning。

**关键点**:
1. Windows 剪贴板 API 的 `GetClipboardData` 返回 `HANDLE`，后续 `GlobalLock/GlobalSize/GlobalUnlock` 必须按 `HGLOBAL` 指针语义使用，不能再把它当 `isize`。
2. 虚拟列表不必一开始就引完整库；对固定高度摘要卡片，用 `scrollTop + slice + spacer` 就能把 DOM 节点稳定压在可控范围内。
3. Vault 预填要宁缺毋滥：账号/密码优先取显式标签行，URL/主机/IP/端口/数据库类型可以推断，但原始全文必须完整进 `notes`，避免信息损失。
4. 新增异步组件后，`components.d.ts` 会在构建时自动补全，属于预期变更，不要误删。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/inbox.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src/components/InboxPanel.vue`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/components/VaultPanel.vue`
- `apps/desktop/src/components/VaultEntryDialog.vue`
- `apps/desktop/src/composables/useClipboardSuggestion.ts`

**验证**:
- `cargo check`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-16: release-all-win 正式发版校验、恢复路径与兼容性补强

**场景**: 在发布 `v0.2.5` 时，需要把“版本统一、正式发版、失败补跑”三类动作整理成稳定流程，并修复脚本里暴露出的兼容问题。

**问题**:
1. 旧脚本会在完整构建之后才暴露版本号不一致、tag 不匹配、工作区未提交等问题，失败成本高。
2. 便携包阶段默认从 `target/release/lazycat_lib.dll` 取 DLL，但当前 Tauri/Rust 产物实际可能输出到 `target/release/deps/lazycat_lib.dll`。
3. 某些 PowerShell 环境没有 `Get-FileHash`，会导致四个产物已经构建完成，却在 `SHA256SUMS.txt` 阶段失败。
4. 文档里只有零散打包说明，缺少“正式 GitHub Release 只能从 main 干净工作区发布”以及“失败后用 `-SkipBuild` 恢复”的明确规则。

**解决**:
1. `release-all-win.ps1` 增加正式发版前置校验：统一读取根 `package.json`、桌面端 `package.json`、`Cargo.toml`、`tauri.conf.json` 的版本，要求完全一致，且 `Tag` 必须等于 `v<version>`。
2. 正式上传路径增加 Git 约束：仅允许从 `main` 的干净工作区发布，发 tag 前先执行 `git push origin main`，并校验已存在 tag 必须指向当前 `HEAD`。
3. 便携包复制逻辑改为同时兼容 `target/release/lazycat_lib.dll` 和 `target/release/deps/lazycat_lib.dll`；哈希生成优先用 `Get-FileHash`，缺失时回退到 .NET `SHA256`。
4. 在 `finally` 中统一清理临时 stage 目录和临时离线配置；文档同步补充正式发版命令、本地出包命令和 `-SkipBuild` 恢复命令。

**关键点**:
1. “正式发版”与“本地出包”是两条不同路径：`-SkipUpload` 可以放宽 GitHub 上传约束，但不能绕过版本一致性校验。
2. 发布 tag 本质上是版本号的一部分，必须和 `tauri.conf.json` 对应版本一致，否则 Release 名称、产物名和源码版本会错位。
3. 构建已成功时不要重跑 10 多分钟的完整流程，优先使用 `pnpm release:all:win -- -Tag vX.Y.Z -SkipBuild` 补哈希或补上传。
4. 对仓库规范做增量补强时，要同步更新 `AGENTS.md` 与 `CLAUDE.md`，避免两份规则再次分叉。

**涉及文件**:
- `scripts/release-all-win.ps1`
- `AGENTS.md`
- `CLAUDE.md`

**验证**:
- `pnpm release:all:win -- -Tag v0.2.5 -SkipBuild -SkipUpload`
- `pnpm release:all:win -- -Tag v0.2.5 -SkipBuild`

**使用次数**: 0

## 2026-03-16: Windows 发版前先统一多处版本号，再走 release 脚本

**场景**: 需要发布 `v0.2.5` 到 GitHub Release，并产出 Windows 安装包与绿色包。

**问题**:
1. 本仓库版本号分散在根 `package.json`、`apps/desktop/package.json`、`apps/desktop/src-tauri/Cargo.toml`、`apps/desktop/src-tauri/tauri.conf.json`，历史上可能出现不一致。
2. `scripts/release-all-win.ps1` 会读取 `tauri.conf.json` 的 `version` 作为产物命名基础，如果只改 tag 不改配置，最终 Release 包名和 Git tag 会错位。
3. release 脚本只负责推送 tag 和上传 Release 资产，不会替代版本提交本身；版本变更必须先进入 Git 提交，再发版。

**解决**:
1. 发版前先统一桌面应用相关版本号到目标版本，至少同步根 `package.json`、桌面端 `package.json`、`Cargo.toml`、`tauri.conf.json`。
2. 先执行 `pnpm typecheck`、`pnpm --filter @lazycat/desktop build:web`、`pnpm test` 做基础校验，再执行 `pnpm release:all:win -- -Tag vX.Y.Z` 产出安装包、绿色包和 `SHA256SUMS.txt`。
3. 先提交版本变更并推送分支，再运行 release 脚本，让 tag、Release 和源码提交保持一致。

**关键点**:
1. 产物文件名跟随 `tauri.conf.json` 的版本，而不是 Git tag。
2. 如果要让远端 `main` 与 Release 对齐，不能只推 tag，还要推送当前分支提交。
3. `release-all-win.ps1` 已内置 slim/full 安装包、portable zip、SHA256 和 `gh release create/upload` 流程，优先复用，不要手工拼装发布步骤。

**涉及文件**:
- `package.json`
- `apps/desktop/package.json`
- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/src-tauri/tauri.conf.json`
- `scripts/release-all-win.ps1`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`
- `pnpm release:all:win -- -Tag v0.2.5`

**使用次数**: 0

## 2026-03-08: 本地待办清空日期时间后仍显示时间的修复

**场景**: 用户反馈新建单次事项时已经手动清空日期和时间，但保存后列表中仍然显示时间，且时间相关统计也可能受影响。

**问题**:
1. Rust `row_to_task_json()` 之前会把任务的 `displayAt` 从 `eventAt` 回退到 `updatedAt`，导致“无日程事项”被伪装成有时间事项。
2. 前端 `TodoPanel.vue` 的时间列、今日到期和逾期判断都直接使用 `eventAt || displayAt`，一旦 `displayAt` 被污染，就会把更新时间当成日程时间展示。
3. `todoBuckets.ts` 的活跃项排序也依赖 `eventAt || displayAt`，会让无时间的单次事项因为伪 `displayAt` 提前排序。

**解决**:
1. Rust 侧收紧 `displayAt` 语义：普通任务/周期实例的 `displayAt` 只等于真实 `eventAt`，不再回退到 `updatedAt`；`item_sort_time()` 也移除 `updatedAt` fallback。
2. 前端新增统一 helper 区分真实日程时间：普通任务/周期实例只看 `eventAt`，周期根记录才看 `displayAt`，并让时间列、今日到期、逾期判断统一复用这层语义。
3. `todoBuckets.ts` 同步按同一规则排序：单次事项没有 `eventAt` 就按“无时间”处理排到末尾，周期根仍可按 `displayAt` 排序。
4. 为避免回归，补充 Rust 定向测试与 `todoBuckets.test.ts`，覆盖“无 `eventAt` 的单次事项不应产生伪 `displayAt`”和“周期根仍可按 `displayAt` 排序”。

**关键点**:
1. `displayAt` 应该只承担“真实可展示的日程时间”语义，不能混入 `updatedAt` 这类元数据字段。
2. 周期根记录是例外：它没有 `eventAt`，但需要用 `displayAt` 展示下一次发生时间；修复时要只收紧单次事项/实例语义，不能误伤周期根。
3. 一旦前后端都存在 `eventAt || displayAt` 这类松散回退，修复必须同时收口映射、展示和排序，否则很容易出现“显示改对了但排序还错”的半修状态。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`

**验证**:
- `pnpm test src/utils/todoBuckets.test.ts`
- `pnpm test`
- `cargo test item_sort_time_should_ignore_updated_at_fallback -- --nocapture`
- `cargo test task_row_without_event_at_should_not_emit_display_at -- --nocapture`
- `cargo check`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-08: 本地待办新增/编辑体验收口

**场景**: 用户要求按既定方案优化本地待办新增/编辑弹窗，包括默认日期时间、日期/时间清空联动、提醒排序、分类排序，以及放开内置分类删除限制。

**问题**:
1. `TodoPanel.vue` 里默认日期、默认时间、提醒状态和重复规则时间分别散落在 `resetItemDraft()`、时间拆分 computed 与保存校验中，直接局部改动很容易出现“UI 能清空、保存却报错”的状态撕裂。
2. 时间输入当前通过 hour/minute 双 `el-select` 拼接，`splitDraftEventTime()` 对空值默认回填 `09:00`，如果不先改这里，任何“清空时间”的交互都会被马上回弹。
3. 分类下拉排序之前已经有一份 `sortedTypes` 计算属性，新增同名实现会直接让 `build:web` 在 SFC 编译阶段报重复声明。
4. Rust `type_delete()` 之前把 builtin 限制和“类型是否存在”校验绑在一起，放开删除时要只移除 builtin 拦截，不能把不存在校验一并丢掉。

**解决**:
1. 在 `todoSchedule.ts` 新增 `getCreateDraftDefaultDateTime()`，统一返回“明天日期 + 下一档 5 分钟时间”；`TodoPanel.vue` 的 `itemDraft` 初始值和 `resetItemDraft()` 都只从这个 helper 取默认值。
2. `TodoPanel.vue` 中将日期清空和时间清空统一收口到 `clearEventSchedule()`：同时清空 `eventDate/eventTime`，并把提醒重置为 `none`；时间选择器保留双下拉，但改为显式“清空”按钮。
3. `splitDraftEventTime()` 先识别空字符串，再由 `composeDraftEventTime()` 负责重组 hour/minute，避免清空后被 `09:00` 兜底；保存时新增“日期和时间必须同时填写或同时清空”的校验。
4. 复用已有 `sortedTypes` 计算属性，只调整模板消费和排序规则；构建报重复声明时先全局搜同名符号，再删除重复块，比盲目重写安全。
5. `todo.rs` 的 `type_delete()` 改为先用 `SELECT 1` 校验类型存在，再继续执行解绑任务/模板与删除流程，从而放开 builtin 删除但保留原有错误语义。

**关键点**:
1. “默认时间”要先明确是“向上取整”还是“下一档 5 分钟”；这次按方案示例收口为“下一档”，因此 `14:55 -> 15:00`。
2. 清空日期/时间时必须同步处理提醒，否则保存校验会因为残留提醒而继续要求填写日程。
3. 前端 build 报 `Identifier ... has already been declared` 时，优先搜同名 computed / const，通常比模板本身更快定位。
4. Rust 定向测试失败时要区分“编译是否通过”和“现有红测是否无关本次改动”；本次通过 `cargo check` 单独确认了编译状态。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoSchedule.ts`
- `apps/desktop/src/utils/todoSchedule.test.ts`
- `apps/desktop/src-tauri/src/tools/todo.rs`

**验证**:
- `pnpm test src/utils/todoSchedule.test.ts`
- `pnpm test`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check`
- `cargo test todo:: -- --nocapture`（存在 1 个与本次改动无关的既有失败：`convert_one_off_task_to_recurring_should_bind_existing_task_without_duplicate`）

**使用次数**: 0

## 2026-03-08: 本地待办置顶排序与已办倒序收口

**场景**: 用户要求参照现有方案，给本地待办补上事项置顶能力，并同步收敛待办/已办列表的排序和操作列行为。

**问题**:
1. `TodoPanel.vue` 的待办列表里混有普通事项与周期根记录，若直接给所有行都接“置顶/取消”或“取消”动作，会把周期根记录误走到 `todo_tasks` 更新链路。
2. `groupTodoItemsByBucket` 之前只负责活跃项按时间升序分桶，已办项没有显式排序，状态切换后展示顺序容易和“最近完成优先”预期不一致。
3. 前后端都各自有一层排序：Rust `task_list/sort_item_rows` 与前端 `todoBuckets.ts`。如果只改一侧，刷新前后顺序会抖动。

**解决**:
1. `types/todo.ts` 为 `TodoItem` 新增 `pinned` 字段，`TodoPanel.vue` 的 `normalizeTodoItem` 接入该字段，待办标题补“置顶”标签，待办操作列新增“置顶/取消置顶”。
2. 前端只对可执行事项显示置顶与取消按钮；周期根记录继续保留编辑/删除路径，避免误调用任务状态或置顶接口。
3. `todoBuckets.ts` 将待办排序改为“`pinned` 优先，其次 `eventAt || displayAt` 升序”，已办改为按 `updatedAt` 倒序；`todoBuckets.test.ts` 补上置顶优先与已办倒序断言。
4. `helpers.rs` 增加 migration 19，为 `todo_tasks` 增加 `pinned` 列；`todo.rs` 新增 `item_toggle_pin` action，并让 `task_list` / `row_to_task_json` / `sort_item_rows` 全链路识别置顶状态。

**关键点**:
1. 周期根记录来自 `todo_templates` 映射，不应复用 `todo_tasks` 的置顶更新语义；前端按钮显隐要按“是否可执行”而不是只看是否在待办区。
2. 已办倒序直接复用 `updatedAt` 作为完成时间代理，前提是状态变更 SQL 始终同步刷新 `updated_at`。
3. 列表排序需要前后端同时收口：后端保证初始返回稳定，前端保证筛选/分桶后顺序仍符合产品规则。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/todo.rs`

**验证**:
- `pnpm test src/utils/todoBuckets.test.ts`
- `cargo test todo:: -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-08: 本地待办提醒改为独立弹窗窗口

**场景**: 用户要求把本地待办的系统通知替换为右下角置顶的自定义提醒弹窗，并支持完成、知道了、稍后提醒等直接操作。
**问题**:
1. 旧链路是 Rust 调度后直接发系统通知，再向主窗口发 `todo-reminder-fired`，主窗口里又会弹一次 `ElNotification`，提醒展示分散且无法直接操作。
2. 独立 Tauri 窗口首次创建时，如果只依赖 `emit("reminder-push")` 推送数据，前端监听尚未挂载时容易丢掉首屏提醒。
3. 生产态弹窗复用同一个前端入口时，不能只依赖 URL query 判断视图，需要给前端一个稳定的“当前就是 reminder-popup”信号。
**解决**:
1. 在 `main.rs` 新增 `show_reminder_popup`、`position_reminder_popup` 与 3 个 popup command，scheduler 改为复用/创建 `reminder-popup` 窗口，并继续向主窗口发 `todo-reminder-fired` 仅用于刷新。
2. `ReminderDispatch` 增加 `priority` 字段，前端弹窗直接消费完整提醒 payload，展示优先级徽章与稍后提醒菜单。
3. 前端 `main.ts` 按 `view=reminder-popup` 或 `window.__LAZYCAT_VIEW__` 切换到独立的 `ReminderPopupApp` 入口，主应用 `App.vue` 删除旧的 `ElNotification` 逻辑。
4. 为避免首屏事件抢跑，Rust 在创建弹窗时通过初始化脚本写入 `window.__LAZYCAT_REMINDER_BOOTSTRAP__`，弹窗组件挂载后先吃这份初始队列，再监听后续 `reminder-push` 事件。
5. capability 为 `reminder-popup` 开窗并补上 `core:window:allow-close`，最后用定向 Rust 单测加 `pnpm typecheck` / `build:web` 完成联调验证。
**关键点**:
1. “替换系统通知”不等于只删掉通知调用；还要把提醒展示、操作命令、首屏补偿和主窗口刷新链路一起收口。
2. 独立弹窗首次打开时，初始化脚本 + 队列去重比单纯依赖页面加载完成后的事件更稳妥。
3. 主窗口的 `todo-reminder-fired` 在这次改造后只承担刷新职责，前端如果还保留旧 toast 监听，就会和新弹窗形成重复提醒。
**涉及文件**:
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/capabilities/default.json`
- `apps/desktop/src/main.ts`
- `apps/desktop/src/ReminderPopupApp.ts`
- `apps/desktop/src/components/ReminderPopup.vue`
- `apps/desktop/src/App.vue`
- `apps/desktop/src/types/todo.ts`

**验证**:
- `cargo test dispatch_due_reminders_should_include_priority_in_payload -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-08: 本地待办改为双区块展示并前端判定逾期

**场景**: 用户要求把本地待办从“超期事项 / 待办事项 / 已办事项”三段简化为“待办事项 / 已办事项”两段，同时把逾期判断前移到前端，并用复选框替代“完成”按钮。

**问题**:
1. `TodoPanel.vue` 的列表模板最初依赖 `itemSections` 循环渲染三段，若只改文案不改分桶结构，很容易留下旧列、旧按钮和重复规则副文案等残留 UI。
2. 后端 `isOverdue` 的语义只覆盖 `pending / in_progress + eventAt < now`；如果前端直接按“任意早于当前时间”打标，已办项也会误显示“逾期”。
3. `todoBuckets.ts` 旧实现依赖 `item.isOverdue` 分桶，但已有单测又要求“周期根事项仍归待办”，改结构时若不顺手修契约，相关回归会继续红。

**解决**:
1. `TodoPanel.vue` 改为两个固定 section：待办区直接展示 `activeItems`，已办区默认折叠，点击标题切换展开/收起。
2. 列表统一改成 7 列：复选框、事项、时间、分类、优先级、执行人、操作；删除状态列、时间副文案和“完成”按钮，改用复选框切换 `completed/pending`。
3. 前端新增 `isItemOverdue()`，仅对 `pending / in_progress` 且 `eventAt || displayAt < now` 的事项显示“逾期”，避免已办项误标红。
4. `todoBuckets.ts` 改为返回 `{ activeItems, doneItems }`，`activeItems` 统一按 `eventAt || displayAt` 升序排序、无时间项排最后，并保留周期根事项进入 activeItems 的 helper 契约。
5. `todoBuckets.test.ts` 同步改为断言新返回结构，并补上时间排序、`displayAt` 回退、无时间排尾和周期根事项归 activeItems 的覆盖。

**关键点**:
1. “前端接管逾期判断”不等于扩大逾期语义；要先锁定状态口径，否则视觉标记会和已有业务含义脱节。
2. 如果某个 helper 已被单测和历史经验共同约束，哪怕当前 UI 层暂时过滤了相关数据，也更适合保留 helper 契约，再由 UI 自己决定是否展示。
3. 计划文案里出现“6 列”但实际列举出 7 列时，应以明确列清单为准，避免把笔误实现成产品行为。
4. 清理展示辅助函数前，先确认它们是否还被弹窗编辑逻辑等其他路径复用；这次 `getItemRecurrence` 仍需保留，不能按列表清理思路直接删除。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`

**验证**:
- `pnpm test src/utils/todoBuckets.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-16: Tauri 自定义 manifest 不要与 embed-resource 并用

**场景**: 用户执行 `pnpm dev`，Rust/Tauri 在 Windows 链接阶段报 `link.exe failed: exit code: 1123`。
**问题**:
1. 表面错误是 `LNK1123`，但真正的首个致命错误是 `CVTRES : fatal error CVT1100: duplicate resource. type:MANIFEST, name:1`。
2. `build.rs` 手工用 `embed-resource` 生成了 `embed_manifest.lib`，同时 `tauri_build::build()` 在 Windows 下也会生成包含 manifest 的 `resource.lib`。
3. 两份 `MANIFEST` 资源同时链接进 exe，会导致资源转换阶段失败，最终表现成 `LNK1123`，容易被误判为 `link.exe` 或 OpenSSL 警告问题。
**解决**:
1. 删除 `build.rs` 中手工生成 `.rc` / `embed_manifest.lib` 的逻辑，不再直接调用 `embed-resource`。
2. 保留自定义 `lazycat.manifest` 内容，但改为通过 `tauri_build::WindowsAttributes::app_manifest(...)` 注入，让 Tauri 成为唯一的 Windows 资源编译入口。
3. 同步移除 `Cargo.toml` 里的 `embed-resource` build dependency，避免后续再次走回旁路方案。
**关键点**:
1. 遇到 `LNK1123` 时，先往前找 `CVTRES` 的第一条 fatal error，不要只盯着最后一行。
2. Tauri 2 在 Windows 下默认就会通过 `tauri-build` 生成资源文件；要自定义 manifest，应扩展它，而不是额外再编一份资源库。
3. `LNK4099` 这类 OpenSSL PDB 警告通常不是主因，先区分 warning 和真正 fatal error。
**涉及文件**:
- `apps/desktop/src-tauri/build.rs`
- `apps/desktop/src-tauri/Cargo.toml`
- `apps/desktop/src-tauri/lazycat.manifest`

**验证**:
- `cargo build --manifest-path apps/desktop/src-tauri/Cargo.toml --no-default-features -vv`
- `pnpm dev`

**使用次数**: 0

## 2026-03-08: 本地待办移除提醒中心并改为超期/待办/已办三段

**场景**: 用户要求移除本地待办中的“提醒中心”功能，并将事项页面固定拆成“超期事项、待办事项、已办事项”三段展示。

**问题**:
1. `TodoPanel.vue` 之前通过“事项 / 提醒中心 / 基础数据”三个页签组织内容，提醒中心并不是独立工具，而是同一组件内的第二个视图，直接删除页签后需要同步清理前端状态和刷新链路。
2. 事项主列表原本是“单表 + 状态筛选”的模式，如果只是把表格视觉上复制三份，容易让筛选条件与三段口径互相打架，尤其是 `completed / canceled` 的归类口径。
3. 三段分组逻辑若继续写死在 `TodoPanel.vue` 模板内，后续很难单测，也容易在周期根事项是否算待办这类规则上再次回归。

**解决**:
1. 删除 `TodoPanel.vue` 中的提醒中心页签、未读提醒列表状态和已读操作，仅保留事项页与基础数据页；事项提醒、系统通知和“稍后10分钟”能力继续保留。
2. 事项页保留“事项视图”和关键词筛选，但去掉“状态筛选”，改为在过滤后的统一事项集合上固定分出三段：超期事项、待办事项、已办事项。
3. 新增 `src/utils/todoBuckets.ts`，将分段逻辑抽成纯函数 `groupTodoItemsByBucket`，明确规则：超期=`pending/in_progress + isOverdue`，待办=周期根事项或未超期的 `pending/in_progress`，已办=`completed + canceled`。
4. 新增 `src/utils/todoBuckets.test.ts` 覆盖超期、待办、已办、周期根归待办四个核心场景，保证 UI 重构后规则仍可独立验证。
5. `App.vue` 中同步调整本地待办入口文案，去掉“提醒中心”表述，避免功能描述与页面实际结构不一致。

**关键点**:
1. 这类“固定分段”页面最好采用“先统一过滤，再固定分桶”的顺序；如果把筛选与分桶交叉写，会很快出现某一段数据来源和用户预期不一致的问题。
2. 周期根事项虽然没有普通状态字段，但在产品语义上仍属于可管理的待办对象，应该显式纳入待办段，而不是依赖状态枚举自然落桶。
3. 移除提醒中心页签并不等于下线提醒能力；只删 UI、保留调度与系统通知，可以把这次改动控制在前端重排层，避免无谓扩大到后端存储退役。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`
- `apps/desktop/src/App.vue`

**验证**:
- `pnpm test src/utils/todoBuckets.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-08: 本地待办编辑态事项类型互转与 5 分钟步进

**场景**: 用户要求本地待办新增事项的时间选择按 5 分钟步进展示，并允许在编辑已有事项时切换“单次事项 / 周期事项”。

**问题**:
1. 前端 `TodoPanel.vue` 之前只是用 `ElTimePicker` 禁用非 5 分钟分钟项，交互上仍是完整 60 分钟列表，不符合“时间间隔为五分钟”的直观预期。
2. 编辑弹窗里“事项类型”单选在非创建态被禁用，已有事项无法直接在单次 / 周期之间切换。
3. 后端 `todo.rs` 的 `item_update` 同时承担单次事项、周期根记录、周期实例和 future scope 更新，若不先识别“当前持久化类型”和“本次目标类型”，很容易把跨类型变更误走成普通更新。

**解决**:
1. 前端把单次时间与周期时间输入切到 `ElTimeSelect`，统一使用 `00:05` 步进，让时间列表只展示 5 分钟粒度。
2. 编辑态放开“事项类型”单选；单次转周期时自动切到 `future_instances` 语义并展示周期规则区，周期转单次时根据编辑上下文回填单次时间锚点。
3. Rust 端为 `item_update` 增加“当前类型 vs 目标类型”识别；同类型继续走 `task_update` / `template_update`，跨类型则走专门转换分支。
4. 单次转周期时创建周期根记录，并把当前任务绑定成首个实例，同时把 `generated_count` 置为 1、`next_occurrence_at` 推到首个实例之后，避免保存后马上再生成一条重复实例。
5. 周期转单次时复用现有 `source_template_id` / `todo_templates` 结构：根记录或 future scope 场景创建/保留一条独立单次事项，再解绑或删除周期模板；当前实例直接转单次的老分支继续保留给后端兼容测试，但前端不默认暴露该入口。

**关键点**:
1. “5 分钟步进”最好直接落在控件层，而不是只靠保存前校验，否则体验上仍像 1 分钟粒度。
2. 单次转周期时如果当前任务直接保留为首个实例，模板的 `next_occurrence_at` 一定要从该实例之后开始算，否则调度器会补出一条同时刻重复实例。
3. 周期转单次在前端应优先走“根记录 / 此后未发生项”语义，避免用户误以为会重写整条周期历史。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/components.d.ts`
- `apps/desktop/src-tauri/src/tools/todo.rs`

**验证**:
- `cargo test todo:: -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`

**使用次数**: 0

## 2026-03-07: 本地待办多提醒与逐条稍后提醒改造

**场景**: 用户要求调整本地待办新增/编辑弹窗的字段顺序与创建态文案，同时把提醒从单选升级为多选，新增默认勾选的“准时提醒”，并让“稍后10分钟”只影响当前触发的那一条提醒。

**问题**:
1. 现有待办模型从前端类型到 Rust 调度都建立在“单事项仅一个提醒时间”的前提上，`todo_tasks.remind_at` 和 `todo_templates.reminder_offset_minutes` 都只能表达单提醒。
2. 提醒中心事件与系统通知只携带 `taskId`，没有具体提醒记录标识，多提醒下无法精确实现“逐条稍后”。
3. 前端 `TodoPanel.vue` 同时承担表单、列表展示和历史兼容解析，若不先统一提醒数组语义，`typecheck` 虽可能通过，但运行时容易出现“默认值/无提醒/旧数据回填”不一致。

**解决**:
1. 前端把 `reminderPreset` 全量切为 `reminderPresets`，增加 `0m` 准时提醒与互斥的 `none` 哨兵值；创建态默认 `['0m']`，编辑已有无提醒事项时显示为 `['none']`。
2. Rust 端在 `helpers.rs` 增加 migration 18，新增 `todo_task_reminders` 与 `todo_template_reminders` 两张子表，把旧单提醒数据迁移成单元素提醒集合，并为 `todo_reminder_events` 补 `task_reminder_id` 与 `reminder_preset`。
3. `todo.rs` 的任务创建、任务更新、周期模板创建、周期模板更新、周期实例生成、提醒派发与提醒中心列表统一改为围绕提醒子表工作；旧列保留但不再作为主真源。
4. “稍后10分钟”改为优先吃 `taskReminderId`；若列表按钮未显式传入，则后端自动选择该事项最近一条仍可触发的提醒。
5. 前端弹窗字段顺序调整为“提醒 → 事项类型 → 周期规则 → 描述”，创建态标题固定“新增事项”，提交按钮固定“创建事项”。

**关键点**:
1. `none` 只作为前端互斥选项存在，提交到后端时必须转为空数组，不能和真实提醒预设一起落库。
2. 多提醒的 `snooze_until` / `last_notified_at` 必须下沉到提醒子表；若继续复用任务表字段，会导致一条提醒被稍后后误伤同事项的其它提醒。
3. 周期实例生成时不要直接复制旧 `remind_at`，而是基于实例 `event_at` 和模板提醒偏移重新计算，才能保证多个提醒时间都正确。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/App.vue`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`

**验证**:
- `cargo test todo:: -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`

**使用次数**: 0

## 2026-03-07: 本地待办调度区重构为日期/时间/提醒/重复

**场景**: 用户要求把本地待办的新增/编辑弹窗重构为更接近日历应用的调度体验，核心围绕“日期、时间、提醒、重复”，并为周期事项补上显式开始日期。

**问题**:
1. 前端 `TodoPanel.vue` 已经拆出新草稿字段，但模板、调度工具和保存逻辑存在两套接口并存，容易出现导出名不一致与提交流程断链。
2. 后端周期模板此前没有 `start_at`，`next_occurrence_at` 默认从保存当下开始推算，无法表达“从指定日期开始重复”。
3. 工具函数与测试一度处于新旧命名混用状态，若不先收敛为单一真源，`typecheck` 与 `build:web` 很容易反复失败。

**解决**:
1. 前端把 `TodoPanel.vue` 的调度区统一为单次事项 `singleDate/singleTime` 与周期事项 `recurrenceStartDate/recurrenceTime/repeatPreset` 两套草稿，并在 `saveItem` 中映射为 `eventAt` 与 `recurrence.startAt`。
2. 新增 `src/utils/todoSchedule.ts` 作为调度规则单一真源，同时兼容旧测试接口和新面板接口，统一提供重复预设、日期时间拆装、规则摘要与结束条件格式化。
3. Rust 端在 `helpers.rs` 增加 migration 17，为 `todo_templates` 增加 `start_at` 并回填历史数据；`todo.rs` 的模板创建、更新、启停、实例生成全部改为尊重 `start_at`。
4. 周期规则继续沿用 simple/Cron 双轨，但简单月规则允许 31 号；前端对“每周自定义且间隔大于 1”直接提示改用高级 Cron，避免后端 silent ignore。

**关键点**:
1. `start_at` 是周期系列的生效下界，不等于 `next_occurrence_at`；创建时要按 `start_at` 首次计算，更新/启用时则按 `max(now, start_at)` 重算，避免重复补历史实例。
2. 若工具文件已演进过多轮，优先收敛为一个稳定导出面，再回头补 `TodoPanel.vue` 与测试，成本低于在两套接口之间硬凑兼容。
3. 图片里的“时间段”这轮不落数据模型，只保留具体时间；“不重复”在现有双模型里仍由单次事项承担，因此周期编辑态需要阻止直接改成不重复。

**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoSchedule.ts`
- `apps/desktop/src/utils/todoSchedule.test.ts`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`

**验证**:
- `pnpm typecheck`
- `pnpm test src/utils/todoSchedule.test.ts`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo test todo:: -- --nocapture`

**使用次数**: 0

## 2026-03-07: 本地待办工具（任务+周期+提醒）一体化落地

**场景**: 新增本地待办能力，要求支持任务类型、优先级、执行人、提醒、周期提醒与周期事件，并同时提供系统提醒与应用内提醒中心。

**问题**:
1. 现有仓库没有 `todo` 工具域，前后端通道、数据表、面板均为空白。
2. 需要兼顾单次任务与周期实例，且应用退出后重启要补偿错过提醒。
3. 系统提醒需与应用内提醒状态同步，避免重复提醒与丢提醒。

**解决**:
1. Rust 新增 `tools/todo.rs`，实现类型/执行人/任务/周期模板/提醒中心 action，及 `scheduler_tick` 调度入口。
2. `helpers.rs` 增加 migration 13，创建 `todo_*` 系列表并注入内置类型（待报事项、工作任务、会议安排、个人事项）。
3. `main.rs` 增加调度线程：每 30 秒执行周期实例生成 + 到期提醒派发；同时发送系统通知并 `emit(\"todo-reminder-fired\")` 给前端。
4. 前端新增 `TodoPanel.vue`，提供任务管理、周期管理、提醒中心与基础数据管理；`App.vue` 全局监听提醒事件并弹通知。
5. 通道映射与类型体系扩展：`bridge/tauri.ts` 新增 `tool:todo:*`，`types/todo.ts` 与 `types/index.ts` 新增导出，`tool-registry.ts` 注册 `todo` 面板。

**关键点**:
1. 周期模板统一存储 Cron 表达式，简单规则在保存时转换为 Cron，降低调度复杂度。
2. 提醒触发条件采用 `COALESCE(snooze_until, remind_at)` + `last_notified_at` 去重，支持“稍后提醒”复触发。
3. 为防止离线过久导致单轮阻塞，周期补偿每轮每模板最多生成 500 条实例，后续轮次继续补齐。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/main.rs`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/App.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/types/index.ts`

**使用次数**: 0

## 2026-03-07: 密码库移除软锁并改为失焦仅隐藏敏感信息

**场景**: 将密码库从“敏感信息隐藏 → 软锁 → 硬锁”收敛为“敏感信息隐藏 → 硬锁”，同时保留失焦时的安全保护体验。

**问题**:
1. 软锁引入了额外状态、IPC 和元数据列表链路，前后端实现复杂度偏高。
2. 失焦锁定会打断当前上下文，用户更需要的是立即恢复掩码显示，而不是直接改变会话状态。
3. `show-password` 输入框的显隐状态由组件内部维护，仅清理外层状态无法在失焦时自动恢复掩码。

**解决**:
1. 后端移除 `soft_lock`、`list_metadata`、`vault_soft_locked` 和 `SoftLocked` 状态，统一只保留 unlocked / locked 两态。
2. 前端空闲计时器改为“到期隐藏敏感信息 + 到期直接硬锁”，失焦事件只执行敏感信息隐藏，不再触发锁定。
3. 为 `VaultPanel`、`VaultEntryDialog`、`VaultLockScreen` 的密码输入引入 `maskVersion` 重挂载机制，失焦时可恢复掩码显示且不清空已输入内容。

**关键点**:
1. “隐藏敏感信息”与“锁定会话”需要明确分层：前者只影响 UI 展示，后者才影响后端解锁态。
2. 失焦隐藏要覆盖列表明文、复制反馈和 `show-password` 组件内部显隐状态，否则体验会出现保护不一致。
3. 锁定预设继续复用 `vault_lock_profile`，仅保留隐藏时长和硬锁时长，避免再引入新的配置分支。

**涉及文件**:
- `apps/desktop/src/components/VaultPanel.vue`
- `apps/desktop/src/components/VaultEntryDialog.vue`
- `apps/desktop/src/components/VaultLockScreen.vue`
- `apps/desktop/src/components/SettingsPanel.vue`
- `apps/desktop/src/composables/useSettings.ts`
- `apps/desktop/src/utils/vaultLock.ts`
- `apps/desktop/src-tauri/src/tools/vault.rs`
- `apps/desktop/src/bridge/tauri.ts`

**使用次数**: 0

## 2026-03-07: 密码库分级锁定优先复用现有会话与设置通道
**场景**: 为密码管理增加“敏感信息隐藏 → 软锁 → 硬锁”的平衡方案，同时保留主密码为唯一解锁凭据。
**问题**:
1. 原实现只有固定 5 分钟硬锁，前端只有布尔锁定态，缺少软锁与预设配置。
2. `vault` 已经具备通用设置持久化、状态查询和会话内存密钥，不适合再造一套存储模型。
3. 软锁需要保留列表上下文，但现有 `list` 接口会解密并返回账号/摘要，不能直接复用到软锁态。
**解决**:
1. 设置层继续走 `user_settings`，新增 `vault_lock_profile`，前端通过 `useSettings` 提供统一读取与策略换算。
2. 后端会话保持“内存密钥 + 状态枚举”，新增 `soft_lock` / `touch` / `list_metadata`，并让 `status` 返回 `lockState`。
3. 前端在 `VaultPanel` 本地做空闲计时与失焦软锁，后端负责硬锁兜底；软锁时改走 `list_metadata` 仅返回非敏感字段。
4. 关闭到托盘时在 `main.rs` 直接调用 `tools::vault::force_lock()`，避免窗口隐藏后仍保留解锁态。
**关键点**:
1. 分级锁定里，“软锁保留上下文”与“硬锁清空会话密钥”要明确分工：前端保留视图，后端控制密钥生命周期。
2. 锁定预设尽量收敛为 `strict / balanced / convenient`，不要把秒数配置直接暴露给用户。
3. 若前端测试在沙箱内出现 `spawn EPERM`，按规范提权重跑即可，不要因为单次 EPERM 放弃验证。
**涉及文件**:
- `apps/desktop/src/components/VaultPanel.vue`
- `apps/desktop/src/components/SettingsPanel.vue`
- `apps/desktop/src/composables/useSettings.ts`
- `apps/desktop/src/utils/vaultLock.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src-tauri/src/tools/vault.rs`
- `apps/desktop/src-tauri/src/main.rs`

**使用次数**: 0

## 2026-03-07: 命名快捷键二次触发隐藏失败根因为缺少 `core:window:allow-hide`
**场景**: `snippets`、`launcher`、`vault` 通过命名快捷键呼出后，再次按下同一快捷键没有隐藏主窗口。
**问题**:
1. 前端热键监听已经命中隐藏分支，但 `appWindow.hide()` 在 Tauri 权限层被拒绝。
2. 日志报错明确提示缺少 `core:window:allow-hide`，导致看起来像“逻辑无效”，实际是权限不足。
**解决**:
1. 在 `apps/desktop/src-tauri/capabilities/default.json` 为主窗口补充 `core:window:allow-hide`。
2. 保留命名快捷键使用结构化 payload 的隐藏判定逻辑，清理仅用于排查的调试日志和设置项。
**关键点**:
1. Tauri 2 的窗口 API 即使前端调用命中分支，也可能因 capability 缺失而在运行时失败。
2. 这类问题应先看权限报错，再决定是否继续扩大逻辑排查范围。
**涉及文件**:
- `apps/desktop/src-tauri/capabilities/default.json`
- `apps/desktop/src/App.vue`
- `apps/desktop/src-tauri/src/main.rs`

**使用次数**: 0

## 2026-02-21: 添加 MDN JavaScript 中文手册（Puppeteer 抓取方案）

**场景**: 将 MDN JS 中文手册（https://developer.mozilla.org/zh-CN/docs/Web/JavaScript）添加为离线手册

**问题**:
1. MDN 是 React SSR + 客户端水合的 SPA，没有静态构建产物可直接使用
2. Yari（MDN 官方构建系统）整站产物数 GB，不现实
3. 页面路径无 `.html` 扩展名（如 `/zh-CN/docs/Web/JavaScript/Reference/Array`）
4. Windows 文件系统不支持 `*` 字符，5 个路径含星号的页面（如 `async_function*`）无法保存

**解决**:
1. 用 Puppeteer + 系统 Edge（`C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe`）抓取
2. 抓取脚本：`scripts/scrape-mdn-js.mjs`，使用 `createRequire` 导入 pnpm 本地 puppeteer
3. 无扩展名 URL 路径一律保存为 `<path>/index.html`（避免同名文件与目录冲突，ENOTDIR 错误）
4. 注册到 `manuals.rs`：`("mdn-js", "MDN JavaScript 手册", "/zh-CN/docs/Web/JavaScript/")`
5. `tauri.conf.json` 的 `bundle.resources` 已有 `**/*` 通配符，自动覆盖新手册目录

**关键点**:
1. `createRequire(import.meta.url)` 以脚本所在目录为基准解析相对路径，ESM 脚本中导入 CJS 模块的正确方式
2. SPA 路由的无扩展名路径必须保存为目录下 `index.html`，否则子路径写入时报 ENOTDIR
3. HTTP 服务器已处理无扩展名路径（`file_path.extension().is_none()` → 尝试加 `.html` 或 `index.html`），MDN 内链接直接可用
4. 含 `*` 字符的页面在 Windows 下无法保存，属于不可绕过的 OS 限制，影响 5 个页面，可忽略

**涉及文件**:
- `scripts/scrape-mdn-js.mjs`（新建，抓取脚本）
- `apps/desktop/src-tauri/src/tools/manuals.rs`（注册新手册）
- `resources/manuals/mdn-js/`（新建，872 个文件，72.3 MB）

**使用次数**: 0

## 2026-02-20: 六方案全量重构（类型集中化 + Composables + App.vue 拆分 + Rust 模块化 + 构建优化 + CSS 分层）

**场景**: 项目存在巨型 App.vue (1538行)、巨型 main.rs (1341行)、重复接口定义、Element Plus 全量导入、CSS 单文件、Monaco 主题不联动等6个架构问题

**问题**:
1. App.vue 1538行 60+ ref 21个 v-else-if，不可维护
2. Rust main.rs 59分支 match，1341行单文件
3. 9处接口重复定义
4. Element Plus 全量导入导致 index.js 999KB
5. styles.css 1447行单文件
6. Monaco 编辑器硬编码 `theme: "vs"`，不跟随 Dark/Light 切换

**解决**:
1. **类型集中化**: 新建 `src/types/` (tools.ts, hosts.ts, ports.ts, calc.ts, index.ts)，所有组件 import from `../types`
2. **Composables**: 新建 `src/composables/` (useToolInvoke.ts, useLocalStorage.ts, useFavorites.ts)
3. **App.vue 拆分**:
   - 新建 `tool-registry.ts`，用 `defineAsyncComponent` 映射工具ID到组件
   - 模板用 `<component :is="currentComponent" :key="activeTool" v-bind="currentComponentProps" />` 替代 21 个 v-else-if
   - 新建 12 个胖组件: RsaPanel, AesPanel, JsonXmlPanel, JsonYamlPanel, TextProcessPanel, EnvPanel, SplitMergePanel, ImagePanel, TimestampPanel, UuidPanel, CronPanel, SettingsPanel
   - 重写已有薄壳组件 (FormatterPanel, RegexPanel, HostsPanel, PortsPanel, CalcDraftPanel) 为胖组件，内化状态和 IPC 调用
   - App.vue: 1538行 -> 190行
4. **Rust 模块化**: 新建 `src-tauri/src/tools/` (18个文件: mod.rs, helpers.rs, encode.rs, convert.rs 等)
   - main.rs: 1341行 -> 311行
5. **构建优化**: 安装 `unplugin-vue-components` + `unplugin-auto-import`，配置 ElementPlusResolver 按需导入；配置 `manualChunks` 拆分 element-plus 和 monaco-editor
   - index.js: 999KB -> 20KB (element-plus 独立 415KB chunk)
6. **CSS 分层**: 拆分 styles.css 为 9 个文件 (tokens, reset, layout, sidebar, home, panels, element-overrides, responsive, theme-light)
   - MonacoPane: MutationObserver 监听 `data-theme` 切换 `vs`/`vs-dark`
   - 修复硬编码 `#dce3ef` -> `var(--lc-border)`

**关键点**:
1. Vue SFC 中不能对普通对象使用 v-model（SettingsPanel 的 isDarkMode），需要用 `:model-value` + `@update:model-value` 模式
2. `<component :is>` 的 v-bind 中可以传递 `onUpdate:xxx` 事件处理器实现双向绑定
3. Rust 模块化后编译器自动捕获所有错误，风险极低

**涉及文件**: App.vue, main.ts, vite.config.ts, styles.css, MonacoPane.vue, tool-registry.ts, src/types/*, src/composables/*, src/components/*Panel.vue (12新建+5重写), src/styles/* (10文件), src-tauri/src/tools/* (18文件), src-tauri/src/main.rs

**使用次数**: 0

## 2026-02-21: 代码片段页三栏拥挤治理与检索管理迭代（批量能力）
**场景**: 代码片段页在三栏结构下信息密度过高，检索与管理动作分散，缺乏批量处理能力，导致日常整理效率低。
**问题**:
1. 中栏仅有搜索和排序，缺少结果反馈与快速筛选。
2. 列表无法多选，无法批量收藏/移动/打标签/删除。
3. 前后端缺少批量操作接口，管理动作需要逐条执行。
4. 布局拥挤，列表与管理动作缺乏分层。

**解决**:
1. 前端中栏改造：
   - 增加“无标签/最近7天”快速筛选。
   - 增加结果计数与“清空筛选”。
   - 列表支持多选（checkbox）并保留单项点击编辑。
   - 增加底部批量操作条（收藏/取消收藏/移动到当前文件夹/添加标签/删除/清空选择）。
2. 前端状态逻辑增强：
   - 增加 `selectedIds` 多选状态与派生计数。
   - 增加 `quickFilter` 快速筛选状态。
   - 在 `loadSnippets` 中统一应用快速筛选，并同步清理不可见选中项。
3. 后端新增批量接口（事务）：
   - `batch_update`: 支持批量收藏、移动文件夹、添加/移除标签。
   - `batch_delete`: 支持批量删除片段。
4. IPC 通道映射新增：
   - `tool:snippets:batch-update` -> `batch_update`
   - `tool:snippets:batch-delete` -> `batch_delete`

**关键点**:
1. 批量更新必须校验 `ids` 非空且去重，且至少包含一个操作字段。
2. 批量写入使用数据库事务，避免部分成功导致状态不一致。
3. 列表筛选后要同步修正多选状态，避免“不可见项仍被批量操作”。
4. 批量移动采用“移动到当前选中文件夹”，无目标文件夹时提示用户先选择。

**涉及文件**:
- apps/desktop/src/components/SnippetPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src-tauri/src/tools/snippets.rs

**使用次数**: 0

## 2026-02-21: 代码片段专属工作区 V2 重构（右键入口 + 新模型 + FTS 检索）
**场景**:
需要将左上角 Lazycat 的交互改为左键回首页、右键进入专属代码片段工作区，并对代码片段页面做结构级重构。

**问题**:
1. 现有 snippets 页面挂在通用工具壳层中，无法形成专注工作区。
2. 旧 snippets 数据模型和查询逻辑偏旧，缺少“最近使用优先”和结构化初始化流程。
3. 首次进入需要执行“清空旧数据并重建”的强制流程。

**解决**:
1. App 壳层增加 `viewMode`，支持 `main` 与 `snippet-workspace` 双模式切换。
2. `SidebarNav` 品牌按钮增加右键事件，右键进入专属工作区，左键行为保持回首页。
3. `SnippetPanel.vue` 重写为标签优先三栏布局，接入 `tool:snippets:v2:*` 通道。
4. Rust `snippets.rs` 重写 V2 逻辑，新增：
   - `v2_init`（首次确认后清空并重建）
   - `v2_list` / `v2_search` / `v2_get` / `v2_create` / `v2_update` / `v2_delete`
   - `v2_mark_used` / `v2_tag_stats` / `v2_folder_list` / `v2_folder_create` / `v2_folder_update` / `v2_folder_delete`
5. `helpers.rs` 增加 schema migration 8，创建 snippets v2 表结构与索引；FTS5 建表降级为可选，避免不支持 FTS 的环境直接失败。

**关键点**:
1. 首次初始化采用强确认输入 `DELETE`，降低误触导致的数据清空风险。
2. 排序默认切到 `last_used_at + use_count`，并在打开/复制时调用 `mark_used`。
3. FTS 不可用时自动退化到 LIKE 查询，不阻断可用性。

**涉及文件**:
- apps/desktop/src/App.vue
- apps/desktop/src/components/SidebarNav.vue
- apps/desktop/src/components/SnippetPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src/styles/layout.css
- apps/desktop/src/styles/responsive.css
- apps/desktop/src-tauri/src/tools/snippets.rs
- apps/desktop/src-tauri/src/tools/helpers.rs

**使用次数**: 0

## 2026-02-21: Cron 工具易用性 V2（Spring 6 字段标准 + 5 字段兼容 + 时区预览）
**场景**:
Cron 工具原先仅提供基础 6 字段输入与简单预览，缺少规范化、模板、规则描述与时区切换，易用性不足。

**问题**:
1. 用户输入 5 字段表达式时无兼容策略，容易报错。
2. 缺少“表达式含义”反馈，用户难以快速确认规则。
3. 预览结果固定本地时间，跨环境排查不便。
4. 前端与后端接口粒度较粗，不利于扩展。

**解决**:
1. Rust `cron` 工具新增 action：`normalize`、`preview_v2`、`describe`。
2. 标准化策略固定为 Spring 6 字段；兼容 5 字段时自动补秒 `0` 并返回 warnings。
3. 预览支持时区参数（local / UTC / IANA 时区），并返回结构化时间项（display/iso/epochMs）。
4. Cron 面板重构为四段式：表达式规范化、字段构建、模板应用、预览表格。
5. 新增前端 `types/cron.ts`，统一响应类型定义。
6. 增加 Rust 单元测试覆盖 normalize、时区回退、常见描述规则。

**关键点**:
1. 保留旧 `tool:cron:preview/parse`，新增 v2 能力，降低回归风险。
2. 7 字段（含 year）明确拒绝，避免隐式不兼容。
3. 时区解析失败回退 local 并给 warning，不中断主流程。

**涉及文件**:
- apps/desktop/src/components/CronPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src/types/cron.ts
- apps/desktop/src/types/index.ts
- apps/desktop/src-tauri/src/tools/cron.rs
- apps/desktop/src-tauri/Cargo.toml

**使用次数**: 0

## 2026-02-21: 文本处理工具重做（清洗 + 提取 + 双栏统计）
**场景**:
将“文本处理”从仅按行去重/排序升级为可配置的文本清洗与提取管线，并增强结果展示。

**问题**:
1. 旧能力过窄，仅 2 个后端 action，难以覆盖日志/配置清洗场景。
2. 前端缺少操作编排、统计反馈、差异预览，用户难以判断处理效果。
3. 文本面板存在文案乱码风险，影响可读性和可维护性。

**解决**:
1. Rust `text` 域替换为统一 `process` action，支持 trim/remove-empty/dedupe/sort/filter/replace/prefix/suffix/extract-column。
2. 新增 `presets` action，返回日志清洗、配置键提取、错误日志提取等预设。
3. 前端 `TextProcessPanel` 重写为双栏对照，新增操作区、统计卡片、变更样本表、自动执行与预设套用。
4. 通道映射改为 `tool:text:process` + `tool:text:presets`，移除旧 `unique-lines/sort-lines`。
5. 新增 `types/text.ts` 并统一导出，明确请求/响应与操作类型。

**关键点**:
1. 采用“前后端协同”：Rust 提供稳定算子，前端负责编排与展示。
2. 变更样本做数量上限控制（`previewLimit`），避免大文本导致前端卡顿。
3. 直接替换旧通道前先全仓检索调用点，确认仅单点使用后再切换。

**涉及文件**:
- apps/desktop/src/components/TextProcessPanel.vue
- apps/desktop/src/bridge/tauri.ts
- apps/desktop/src/types/text.ts
- apps/desktop/src/types/index.ts
- apps/desktop/src-tauri/src/tools/text.rs
- apps/desktop/src/App.vue

**使用次数**: 0

## 2026-02-21: Backend Unit Test Expansion for Critical Tool Domains
**场景**: 为 Rust 后端 tools 域补充单元测试，重点覆盖编码转换、加解密、模板渲染与高风险输入分支。
**问题**:
1. 现有测试主要集中在 cron/text，核心安全与转换能力覆盖不足。
2. 多个 action 缺少错误分支验证，回归时容易出现静默偏差。
3. 系统能力（network/dns/file/image/env/port 等）缺少稳定 smoke 测试。
**解决**:
1. 为 `encode/crypto/convert/jwt/schema/mybatis/nginx` 增加核心单测与错误分支。
2. 为 `network/dns/file/image/env/port/format/gen/time/regex/manuals/mod` 增加稳定测试。
3. 调整易波动断言（如 OpenSSL DES 可用性、resize 等比行为、url_decode 容错行为）以避免假阳性失败。
4. 统一执行 `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`，最终 76/76 通过。
**关键点**:
1. 避免新增测试依赖导致环境下载失败；优先使用标准库与现有依赖。
2. 对系统相关能力尽量使用本地回环与临时文件，避免依赖外网和真实系统状态。
3. 对第三方库行为差异（OpenSSL provider、urlencoding 容错）采用兼容断言。
**涉及文件**:
- apps/desktop/src-tauri/Cargo.toml
- apps/desktop/src-tauri/src/tools/encode.rs
- apps/desktop/src-tauri/src/tools/crypto.rs
- apps/desktop/src-tauri/src/tools/convert.rs
- apps/desktop/src-tauri/src/tools/jwt.rs
- apps/desktop/src-tauri/src/tools/schema.rs
- apps/desktop/src-tauri/src/tools/mybatis.rs
- apps/desktop/src-tauri/src/tools/nginx.rs
- apps/desktop/src-tauri/src/tools/network.rs
- apps/desktop/src-tauri/src/tools/dns.rs
- apps/desktop/src-tauri/src/tools/file.rs
- apps/desktop/src-tauri/src/tools/image.rs
- apps/desktop/src-tauri/src/tools/env.rs
- apps/desktop/src-tauri/src/tools/port.rs
- apps/desktop/src-tauri/src/tools/regex.rs
- apps/desktop/src-tauri/src/tools/manuals.rs
- apps/desktop/src-tauri/src/tools/settings.rs
- apps/desktop/src-tauri/src/tools/snippets.rs
- apps/desktop/src-tauri/src/tools/hotkey.rs
- apps/desktop/src-tauri/src/tools/format.rs
- apps/desktop/src-tauri/src/tools/gen.rs
- apps/desktop/src-tauri/src/tools/time.rs
- apps/desktop/src-tauri/src/tools/mod.rs

## 2026-02-27: release 脚本 Git link.exe 遮蔽 MSVC 链接器

**场景**: 执行 `release-all-win.ps1` 打包脚本，Rust 编译链接阶段失败
**问题**: `C:\Program Files\Git\usr\bin\link.exe`（GNU coreutils link）在 PATH 中优先于 MSVC 的 `link.exe`，导致 `linking with link.exe failed: exit code: 1`。即使 VsDevCmd.bat 已执行，Git 的 usr/bin 仍在 PATH 前面
**解决**: 在 `Invoke-InVsDevEnv` 函数中，调用 cmd /c 前在 PowerShell 层面过滤 PATH：`$env:Path = ($env:Path -split ';' | Where-Object { $_ -notmatch 'Git\\usr\\bin' }) -join ';'`，并在 finally 块中恢复原始 PATH
**关键点**:
1. cmd.exe 内的 `set "PATH=%PATH:old=new%"` 字符串替换对含空格路径不可靠，应在 PowerShell 层面处理
2. VsDevCmd.bat 虽然设置了 MSVC 工具路径，但不会移除已有的 Git 路径
**涉及文件**: scripts/release-all-win.ps1
**使用次数**: 0
**使用次数**: 0

## 2026-03-07: 本地待办统一为事项实例 + 周期系列

**场景**: 用户希望把原本分开的“任务”和“周期事件”整合成统一模型与统一维护入口，主列表以当前可执行事项为中心。

**问题**:
1. 旧实现虽然在同一个 `todo` 工具内，但前端仍按 `task/template` 两套对象分栏维护。
2. 后端缺少“单次事项也属于系列”的统一语义，周期规则与实例操作边界不清晰。
3. 前端编辑器没有统一承载“单次事项 / 周期事项 / 当前实例 / 后续系列”四种编辑上下文。

**解决**:
1. `helpers.rs` 新增 migration 14：为 `todo_templates` 增加 `series_kind`，并把历史孤立任务回填为 `one_off` 系列。
2. `todo.rs` 新增 unified actions：`item_*` 与 `series_*`，同时保留 `task_*` / `template_*` 兼容别名。
3. 主列表统一走 `item_list`，补充 `seriesId`、`seriesKind`、`isRecurring`、`canEditFuture`、`displayAt` 等字段。
4. `TodoPanel.vue` 重构为“事项 / 系列 / 提醒中心 / 基础数据”四视图，并用单一弹窗统一创建与编辑。
5. 周期实例编辑支持 `this_instance` 与 `future_instances` 两种作用域；后者由后端转为系列更新。

**关键点**:
1. 单次事项创建时也自动创建 `one_off` 系列，避免后续逻辑继续依赖空的 `source_template_id`。
2. 周期系列继续保留“生成实例”语义，调度器只处理 `recurring` 且启用中的系列。
3. 系列删除不会删除历史实例；已生成实例会退化为独立事项继续保留。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/types/index.ts`

**验证**:
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `cargo check`

**使用次数**: 0

## 2026-03-07: 本地待办自动收藏与命名快捷键接入

**场景**: 用户希望“本地待办”默认出现在首页常用工具中，并像代码片段/密码管理/快捷启动一样支持单独的全局快捷键呼出。
**问题**:
1. 首页“常用工具”当前完全依赖 `favorites` 与近 30 天点击历史，没有“默认收藏一次性补种”机制。
2. 现有命名快捷键链路已支持任意目标工具，但前端只暴露了 `snippets`、`vault`、`launcher` 三个配置入口。
3. 如果直接每次启动都强行把 `todo` 加回收藏，会覆盖用户手动取消收藏的意图。
**解决**:
1. 在 `useFavorites.ts` 中抽出 `normalizeFavoriteToolIds` 与 `bootstrapFavoriteToolIds`，统一做收藏去重、过滤与待办一次性补种。
2. 新增 `favorites_todo_seeded` 标记：首次启动时若收藏中没有 `todo`，自动插入收藏首位；一旦补种完成或用户原本已收藏，即写入标记，后续不再重复干预。
3. 在 `SettingsPanel.vue` 增加“本地待办”快捷键录入项，并纳入现有冲突检测、保存与清空流程。
4. 在 `App.vue` 启动阶段读取 `hotkey_todo`，通过现有 `registerNamedHotkey("todo", ...)` 注册；继续复用 `hotkey-navigate` 的显隐/跳转逻辑。
5. 新增 `useFavorites.test.ts` 覆盖补种规则，并补充 `hotkeyNavigate.test.ts` 的 `todo` 场景回归。
**关键点**:
1. “固定到常用工具”在本需求里等价于“走现有收藏模型的一次性自动收藏”，不是新增永久固定入口。
2. 对已手动收藏 `todo` 的用户也要写入补种完成标记，避免日后取消收藏后又被系统重新加回。
3. 复用现有命名快捷键协议即可，前后端无需新增 Tauri command 或事件结构。
**涉及文件**:
- `apps/desktop/src/composables/useFavorites.ts`
- `apps/desktop/src/composables/useFavorites.test.ts`
- `apps/desktop/src/App.vue`
- `apps/desktop/src/components/SettingsPanel.vue`
- `apps/desktop/src/utils/hotkeyNavigate.test.ts`

**验证**:
- `pnpm --filter @lazycat/desktop test src/utils/hotkeyNavigate.test.ts src/composables/useFavorites.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-07: 本地待办事件时间与提醒预设重构

**场景**: 用户要求把本地待办里的“截止时间 + 提醒时间”重构为“事件时间 + 提醒预设”，事件时间最小刻度统一为 5 分钟，并把周期系列从独立页签合并到事项页下方折叠区。
**问题**:
1. 旧模型同时维护 `due_at` / `remind_at`，单次事项与周期实例的含义不一致，前端也需要维护两个绝对时间输入。
2. 周期系列没有单独的提醒偏移字段，生成实例时只能把提醒时间直接写成触发时刻。
3. 现有文件里有历史乱码文案，重构过程中很容易把 Rust 字符串语法一并带坏，必须依赖编译器逐轮清理。
**解决**:
1. 前端类型与表单统一切到 `eventAt + reminderPreset`，提醒预设固定为 `none/5m/10m/30m/1h/1d/2d`，并在表单提交前校验 5 分钟刻度。
2. Rust 端新增 `event_at` 与 `reminder_offset_minutes` 模型：任务对外只暴露事件时间与提醒预设，内部继续复用 `remind_at + snooze_until` 做提醒调度。
3. migration 15 为历史数据回填 `event_at`，只保留能精确映射到新预设的旧提醒，其余旧 `remind_at` 直接清空；周期模板提醒偏移统一置空。
4. 事项页合并“系列”页签，在列表下方折叠展示周期系列，周期事项的规则区块移动到弹窗下半部分。
5. Rust 单测补充 5 分钟刻度与提醒预设换算，并用 `cargo test todo:: -- --nocapture` 做定向回归；前端用 `pnpm typecheck` 与 `pnpm --filter @lazycat/desktop build:web` 验证联调。
**关键点**:
1. 对任务编辑来说，`eventAt` 或 `reminderPreset` 任一变更都要重新计算 `remind_at`，同时清空 `snooze_until` 与 `last_notified_at`，避免旧稍后提醒污染新计划。
2. 对周期模板来说，只存提醒偏移，不存绝对提醒时间；实例生成时再根据发生时间反推 `remind_at`。
3. 处理历史乱码文件时，不要盲目整文件替换；先跑编译，再按报错行定点修复，成本最低、风险也最小。
**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/types/todo.ts`
- `apps/desktop/src/types/index.ts`
- `apps/desktop/src-tauri/src/tools/todo.rs`
- `apps/desktop/src-tauri/src/tools/helpers.rs`

**验证**:
- `cargo test todo:: -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-03-08: 本地待办合并待办列表并改为前端逾期判断

**场景**: 用户要求把“超期事项 + 待办事项”合并为一个待办列表，已办默认折叠，并用复选框替代“完成”按钮。
**问题**:
1. 前端 `TodoPanel.vue` 已经切到固定双区块展示，但底层 `groupTodoItemsByBucket` 仍返回 `overdueItems / pendingItems / doneItems`，与面板消费口径不一致。
2. 旧分桶逻辑依赖后端下发的 `isOverdue` 字段，无法满足“前端自行判断逾期”的新要求，也会让展示规则和排序规则分散在不同层。
3. `todoBuckets.test.ts` 里“周期根事项进入待办桶”的历史断言此前就是红的，如果不在这次一并修正，后续待办列表改造很难确认真实回归状态。
**解决**:
1. `todoBuckets.ts` 改为只返回 `activeItems + doneItems`，其中 `activeItems` 收口为“可执行项 + 周期根事项”，并按 `eventAt || displayAt` 升序排序、无时间排最后。
2. `TodoPanel.vue` 改为消费 `activeItems / doneItems`，事项列中的“逾期”标记统一由前端按 `pending/in_progress + (eventAt || displayAt) < now` 判断。
3. 待办与已办表格统一增加复选框列：待办勾选即切 `completed`，已办取消勾选回 `pending`；同时移除状态列、时间列副文本和操作列里的“完成”按钮。
4. `todoBuckets.test.ts` 同步改成新返回结构断言，并补充排序、`displayAt` 回退、无时间排最后与周期根事项归活跃桶场景，恢复该模块测试基线。
**关键点**:
1. 逾期展示应只面向待处理项；如果直接按“时间早于当前”判断，已办项也会被错误标成逾期。
2. 固定分段页面适合“先过滤，再统一分桶”，分桶 helper 只负责分组和排序，不承担 UI 折叠或标记文案逻辑。
3. 历史红测如果正好落在本次改造路径上，应顺手修复；否则即使构建通过，也很难判断新改动是否真的稳定。
**涉及文件**:
- `apps/desktop/src/components/TodoPanel.vue`
- `apps/desktop/src/utils/todoBuckets.ts`
- `apps/desktop/src/utils/todoBuckets.test.ts`

**验证**:
- `pnpm test src/utils/todoBuckets.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0


## 2026-07-07: 结构治理批次 0-1 行为保持拆分

**场景**: 执行结构治理路线图首批实施计划：先恢复 e2e 冒烟，再把 PM 前端域迁入子目录并拆分 `PmPanel.vue`。
**问题**:
1. `App.vue` 在纯 Web e2e 环境 setup 顶层调用 Tauri 窗口 API，导致主 App 无法挂载，后续重构缺少端到端安全网。
2. PM 组件平铺在 `components/` 根目录，`PmPanel.vue` 同时承担侧栏、工具栏、筛选、CRUD、右键菜单和视图编排，后续改动冲突面大。
**解决**:
1. `App.vue` 仅在检测到 `__TAURI_INTERNALS__` 时获取 `getCurrentWindow()`，非 Tauri 环境用可选链兜底。
2. 先用 `git mv` 把 18 个 `Pm*` 组件迁入 `components/pm/`，只更新外部动态 import 和跨目录相对路径，保留 `InlinePmSelector` / `InlineTodoList` 在根目录。
3. `PmPanel.vue` 按耦合度从低到高拆出 `usePmContextMenu`、`usePmItemFilters`、`usePmItemActions`、`PmSidebar`、`PmToolbar`，共享状态继续留在壳层，用 props down / events up 保持原行为。
**关键点**:
1. 行为保持拆分要先恢复最小 e2e 安全网；如果 e2e 本身已坏，先修阻断点，不把后续结构改动和测试基线问题混在同一提交。
2. Vue 壳层拆分优先抽 composable，再抽布局组件；筛选条件、选中项、拖拽状态、对话框编排等跨块共享状态留在壳层，避免引入新状态管理模式。
3. 行数目标只作为拆分反馈，不作为临时加拆分的理由。本批后 `PmPanel.vue` 为 1531 行，低于原 2643 行但未达 1200 行软目标；继续缩小需要另定接缝。
**涉及文件**:
- `apps/desktop/src/App.vue`
- `apps/desktop/src/components/pm/*`
- `apps/desktop/src/composables/usePmContextMenu.ts`
- `apps/desktop/src/composables/usePmItemFilters.ts`
- `apps/desktop/src/composables/usePmItemActions.ts`
- `apps/desktop/src/tool-registry.ts`
- `apps/desktop/src/composables/pmViewRegistry.ts`

**验证**:
- `pnpm test:e2e`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
- `pnpm test`
- PM 行为清单手工冒烟通过

**使用次数**: 0

## 2026-07-08: API Mock CORS 修复与增强

**场景**: 梳理 API Mock CORS 功能后集中修复发现的问题：项目级"新路由默认启用 CORS"配置不生效、`allowMethods` 无编辑入口、CORS 响应头不区分预检与实际响应、credentials 校验可被空 Origin 绕过；同时增强 Allow-Origin 多值回显能力。
**问题**:
1. `enabledCorsDefault` 有完整的存储、编辑 UI、序列化链路，但新建路由时 `assignRouteForm` 硬编码 `{ ...DEFAULT_API_MOCK_CORS }`，配置无消费方，是死配置。
2. `parse_cors_config` 中"空 Origin 兜底为 *"发生在 credentials 校验之后，credentials + 空 Origin 会绕过"credentials 不允许 *"校验。
3. 预检专用头（Allow-Methods/Allow-Headers/Max-Age）与实际响应专用头（Expose-Headers）混在同一个 `build_cors_headers` 中全量输出。
4. 共享常量 `DEFAULT_API_MOCK_CORS` 被 `{ ...spread }` 浅拷贝，其中 `allowMethods` 数组引用共享，加编辑 UI 后会污染常量。
**解决**:
1. 前端新增 `createDefaultApiMockCors(enabled)` 工厂函数（内部重建 `allowMethods` 数组），新建路由时传入 `selectedProject.enabledCorsDefault`。
2. 后端把空值兜底移到 credentials 校验之前；校验改为解析逗号分隔列表后检查是否含 `*`，前端 `validateMockCorsConfig` 同步一致。
3. `build_cors_headers` 增加 `preflight` 参数分流输出；`resolve_cors_allow_origin` 支持多值配置按请求 Origin 大小写不敏感回显，非通配时自动附带 `Vary: Origin`，未命中回退首个配置值。
4. 表单补 Allow-Methods 多选（留空自动使用路由方法）、CORS 折叠标题显示启用状态、关闭时其余字段禁用、面板内行为提示文案。
**关键点**:
1. 排查配置类功能时按"存储-编辑-消费"三环检查闭合性：字段有存储和 UI 不代表生效，必须找到消费方；grep 字段名后确认除表单读写外还有业务落点。
2. 校验与兜底的顺序敏感：兜底改变值域时（空 -> *），校验必须在兜底之后（或校验逻辑覆盖兜底前的等价输入），否则形成绕过路径。前后端双侧校验要同步同一语义。
3. 导出的共享默认值常量含数组/对象字段时，使用处 `{ ...spread }` 只有浅拷贝；提供工厂函数替代直接展开，避免消费方遗漏深拷贝。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/api_mock.rs`
- `apps/desktop/src/utils/apiMock.ts`
- `apps/desktop/src/utils/apiMock.test.ts`
- `apps/desktop/src/components/api-mock/ApiMockRouteForm.vue`
- `apps/desktop/src/components/ApiMockPanel.vue`

**验证**:
- `cargo test api_mock`（29 通过，含 3 个新增 CORS 用例）
- `pnpm test src/utils/apiMock.test.ts`（26 通过）
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-17: 请求转发三栏日志工作台改造

**场景**: 用户希望提高请求转发页面的数据密度，把规则维护收口到弹窗，并将日志浏览改造成左侧规则、中间日志、右侧详情的三栏工作台。
**问题**:
1. 规则编辑目标、当前观测规则和异步刷新共享选中状态时，右键编辑或删除容易误切日志上下文，较慢响应也可能覆盖用户的新操作。
2. 日志详情与列表行混排会显著降低单位屏幕信息量，后台刷新整批替换日志时还可能丢失当前详情选择。
3. 详情栏宽度既要适应当前窗口，又要记住用户偏好；如果把窗口钳制后的宽度直接持久化，窗口临时收窄会永久覆盖用户原有设置。
4. Element Plus 右键菜单组件 ref 若在模板渲染期间写入响应式集合，可能触发递归更新。
**解决**:
1. 新建与编辑共用规则弹窗，用独立 `editorRuleId` 和意图 token 绑定编辑目标；当前日志观测继续只由 `selectedId` 驱动。
2. 日志改为高密度表格行，HTTP 头与正文迁入独立详情检查器；后台刷新后只按稳定日志 id 保持选中，目标不存在时显式关闭详情。
3. splitter 分开维护“首选宽度”和“当前渲染宽度”：只在拖拽结束或键盘调整后保存首选宽度，窗口变化只通过纯函数临时钳制显示值。
4. 右键菜单 ref 使用普通 `Map` 缓存，左键选择、右键菜单和行内启动/停止保持独立事件路径。
5. 日志表的列降级使用命名容器查询，以实际日志区域宽度为断点，并取消会撑破中栏的固定 `min-width`。
6. 规则行操作按钮绝对定位在首行，摘要使用整行宽度并允许端点自然换行，同时保留完整内容的悬停提示。
**关键点**:
1. 配置弹窗的编辑目标不能复用观测选中项；异步保存还要携带意图快照，避免旧响应改变新选择。
2. 可拖拽面板持久化的是用户偏好，不是受当前视口约束后的派生值。
3. 后台列表替换只能用稳定 id 延续详情上下文，不能依赖对象引用或行号。
4. 模板函数 ref 只写非响应式缓存，避免在渲染期制造新的响应式更新。
5. 可拖拽分栏内部的响应式规则必须基于容器宽度；视口媒体查询无法感知 sibling 拖拽造成的局部区域变化。
6. 高密度导航可以让操作按钮只占用标题行，避免整列永久挤压需要完整展示的摘要信息。
**涉及文件**:
- `apps/desktop/src/components/RequestForwardPanel.vue`
- `apps/desktop/src/components/request-forward/RequestForwardRuleList.vue`
- `apps/desktop/src/components/request-forward/RequestForwardRuleDialog.vue`
- `apps/desktop/src/components/request-forward/RequestForwardLogList.vue`
- `apps/desktop/src/components/request-forward/RequestForwardLogInspector.vue`
- `apps/desktop/src/utils/requestForward.ts`

**验证**:
- `pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts`
- `pnpm test`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-18: 访问链路旧数据迁移与脱敏报告

**场景**: 在原位替换 network 面板后兼容旧收藏/历史，并为访问链路诊断提供受限持久化、复制和导出报告。
**解决**:
1. 新增 `network_diagnostics` 设置 envelope，固定 `schemaVersion: 1`；首次加载从 `network_test_favorites` / `network_test_history` 幂等构造新状态，旧 key 保留并继续同步写入。
2. 历史目标解析只对单冒号 `host:port` 拆分，`[IPv6]:port` 使用方括号规则，裸 IPv6 不被误切；详细报告最多保留 10 条并限制单条/总大小。
3. 报告复制与导出统一递归脱敏，清除代理凭据、Authorization、Cookie 和敏感查询参数；终态报告持久化同样使用脱敏结果。
**关键点**:
- 迁移必须读新 envelope 优先、旧 key 只作回退，重复执行不能生成随机 ID 或覆盖用户原始数据。
- 脱敏需同时覆盖可解析 URL 和 malformed 文本，查询参数按敏感名称匹配，不能只依赖 URL parser。
**涉及文件**:
- `apps/desktop/src/utils/networkDiagnosticsPersistence.ts`
- `apps/desktop/src/utils/accessPathReport.ts`
- `apps/desktop/src/utils/networkFavorites.ts`
- `apps/desktop/src/components/network/NetworkQuickProbe.vue`
- `apps/desktop/src/components/network/NetworkDiagnosisWorkspace.vue`
**验证**:
- `pnpm --filter @lazycat/desktop exec vitest run src/utils/networkFavorites.test.ts src/utils/networkDiagnosticsPersistence.test.ts src/utils/accessPathReport.test.ts src/components/NetworkPanel.test.ts`
- `pnpm typecheck`
- `git diff --check`

**使用次数**: 0
## 2026-07-18: 访问链路诊断集成 fixture 与终态结论

**场景**: 收口访问链路诊断的跨层验证，并让运行时终态报告产生可追溯结论和建议。
**解决**:
1. 增加本地 TCP/TLS/HTTP/代理 fixture 覆盖，DNS fixture 同时监听 UDP/TCP，验证 NXDOMAIN、SERVFAIL、REFUSED、无记录和超时分类；补取消、整体/单步超时、晚到结果和并发清理断言，并明确候选域名扩展为 unsupported。
2. 终态报告按真实 step lifecycle/outcome 生成 conclusions/recommendations，失败或 blocked 步骤补带错误证据的结论；全成功报告生成 info 结论，取消报告保留已完成步骤并生成可引用证据。
3. 保持 `network.rs` 的 `http_status_*` 与 `chmod_calc` action 不变，并用原有测试锁定兼容性。
**关键点**:
- 本地 DNS fixture 必须同时提供 UDP 与 TCP；Hickory 可能在两种传输间分配查询，单监听 UDP 会制造假超时。
- 结论和建议只能引用报告中的 evidenceId，不能从前端状态臆造成功；取消/超时也应保留真实错误阶段。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/adapters/dns.rs`
- `apps/desktop/src-tauri/src/tools/access_path_diagnostics/runtime.rs`
**验证**:
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml access_path_diagnostics -- --nocapture`（72 通过）
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml tools::network::tests -- --nocapture`（8 通过）

**使用次数**: 0
## 2026-07-18: 上线包打包采用构建与归档两阶段提交

**场景**: 外部项目需要在桌面工具中串行执行前后端构建，并把不同形态的产物合并归档到可编辑的周四日期目录。

**问题**:
1. 构建命令、产物复制和 UI 日志如果分散到前端，会形成多份运行状态并难以可靠终止子进程。
2. 直接向最终目录复制会在失败或取消后留下看似可用的不完整上线包。
3. 面板切换会卸载 Vue 组件，组件局部状态无法持续接收长任务事件。

**方案**:
1. Rust 统一编排 PowerShell、stdout/stderr、进程树取消和产物归档；所有事件绑定唯一 runId。
2. 前后端均成功后才写同卷临时目录，复制/ZIP 全部完成后原子重命名；失败由 staging guard 清理。
3. 前端运行态放在 module-level composable，日志有界保留，旧 runId 事件不能覆盖当前任务。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/release_package*.rs`
- `apps/desktop/src/components/ReleasePackagePanel.vue`
- `apps/desktop/src/composables/useReleasePackageRuntime.ts`

**验证**:
- `cargo test release_package --manifest-path apps/desktop/src-tauri/Cargo.toml -- --nocapture`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

## 2026-07-19: 上线包多命令编辑与紧凑工作台

**场景**: 编辑多行 PowerShell 构建脚本，同时对照前后端配置，并提升白底日志的可读性。

**解决**:
1. 保持后端单段脚本、同一 PowerShell 会话的执行语义，让多行命令共享环境变量和上下文。
2. 前端采用响应式双栏卡片，让前后端工程、命令与产物配置可以并排对照，窄窗口下再收为单栏。
3. 提供只读、可测试的 PowerShell 示例数据，通过带专属 `popper-class` 的 Popover 展示并支持一键复制。
4. 日志区域改为白色卡片，并分别提高状态标签、正文、meta 与 stderr 的对比度。

**关键点**:
1. 不能在前端逐行拆分脚本，否则会破坏环境变量复用和 PowerShell 语句块；多行内容必须作为单段交给后端执行。
2. 关键外部命令失败既要检查 `$?`，又要在可用时保留 `$LASTEXITCODE`；command-not-found 时 `$LASTEXITCODE` 可能为空，必须回退到明确的非零退出码。
3. Teleport 浮层使用专属 `popper-class` 和全局限定样式，避免 `scoped` 样式失效或污染其他 Popover。
4. 小字号状态和 meta 文本的颜色对比度至少达到 4.5:1，白底下不能只依赖低对比灰色区分层级。

**涉及文件**:
- `apps/desktop/src/utils/releasePackage.ts`
- `apps/desktop/src/utils/releasePackage.test.ts`
- `apps/desktop/src/components/ReleasePackagePanel.vue`
- `apps/desktop/src/components/ReleasePackagePanel.test.ts`

**验证**:
- `pnpm typecheck`：exit 0，2 个工作区项目完成 TypeScript 检查。
- `pnpm --filter @lazycat/desktop build:web`：exit 0，Vite 转换 3299 个模块并成功产出 `dist-renderer`；仅有现有 chunk 大小警告。
- `pnpm test`：exit 0，71 个测试文件、698 个测试通过；`data-dictionary` provider 的失败路径用例按设计主动写入 stderr（`Error: boom`），不影响通过结论。
- `git diff --check`：exit 0，无空白错误。

**使用次数**: 0

## 2026-07-19: 上线包归档目录改为项目级配置

**场景**: 不同项目需要归档到不同位置，同时修复上线包配置内容超出视口后无法继续向下浏览。

**解决**:
1. 将归档根目录纳入项目配置模型，创建、更新、准备和启动任务均只读取当前项目的目录。
2. 为已有项目新增字段迁移，并把旧的全局归档根目录一次性回填到尚未配置目录的项目。
3. 取消面板 flex 收缩和工作区内容裁剪，让外层内容容器按页面完整高度滚动。

**关键点**:
- 运行时路径只能来自已保存的项目快照，避免切换项目后使用另一项目的目录。
- 新增项目字段时需同时覆盖新库建表、旧库加列和历史配置回填三条路径。

**涉及文件**:
- `apps/desktop/src-tauri/src/tools/helpers.rs`
- `apps/desktop/src-tauri/src/tools/release_package.rs`
- `apps/desktop/src/components/ReleasePackagePanel.vue`
- `apps/desktop/src/types/release-package.ts`
- `apps/desktop/src/utils/releasePackage.ts`

**验证**:
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture`
- `pnpm --filter @lazycat/desktop exec vitest run src/utils/releasePackage.test.ts src/components/ReleasePackagePanel.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`

**使用次数**: 0

## 2026-07-20: 请求转发实时日志筛选、暂停与导出

**场景**: 请求转发日志需要在持续写入时支持精确筛选、暂停检查、键盘浏览、详情复制和可追溯导出，同时不能让旧响应或后台刷新破坏当前可见窗口。

**解决**:
1. 后端将 Method、状态码和 UTC 时间边界纳入同一 SQL 条件，先筛选并统计，再按 created_at DESC, id DESC 稳定排序和分页；HTTP 专属字段使用精确条件，使 TCP/UDP 日志在该条件下自然返回空集。
2. 前端把筛选值、规则 ID 和选择意图组成请求快照；本地 datetime-local 在请求前转换为 RFC3339 UTC，避免把本地时间直接当成数据库 UTC 文本比较。
3. 暂停状态只用 limit=1 探测筛选结果的 latestId/total，不替换可见日志；恢复实时后从 offset=0 重建连续窗口。total 差值只作为新增下界展示为“至少 N 条”，不把保留淘汰后的差值包装成精确数量。
4. 导出前按当前筛选重新查询最多 1000 条；JSON 带 total/exported/truncated/filters 元数据，CSV 防公式注入，文件名清理 Windows 非法字符并带本地时间。
5. 日志列表使用 roving tabindex 和上下/Home/End 导航；详情仅在 JSON Content-Type 且正文可解析时格式化，并为错误、headers、body 和完整日志提供显式复制反馈。

**关键点**:
- 实时刷新、暂停探测和用户筛选必须共用请求序号与参数快照，旧响应不能覆盖新规则或新筛选。
- 每规则日志存在 1000 条保留上限时，total 不足以单独判断新增；应同时比较最新稳定 ID，并避免伪造精确新增数。
- 时间筛选的 UI 语义是本地时间，数据库事实是 UTC 文本；边界转换只能集中在查询构造处完成。

**涉及文件**:
- apps/desktop/src-tauri/src/tools/request_forward/mod.rs
- apps/desktop/src-tauri/src/tools/request_forward/repository.rs
- apps/desktop/src/components/RequestForwardPanel.vue
- apps/desktop/src/components/request-forward/RequestForwardLogList.vue
- apps/desktop/src/components/request-forward/RequestForwardLogInspector.vue
- apps/desktop/src/types/request-forward.ts
- apps/desktop/src/utils/requestForward.ts

**验证**:
- cargo test --target-dir E:\tmp\lazycat-task1-target request_forward -- --nocapture（112 通过）
- pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts（94 通过）
- pnpm test（72 个测试文件、751 个测试通过）
- pnpm typecheck
- pnpm --filter @lazycat/desktop build:web

**使用次数**: 0

## 2026-07-21: 全局通知窗口统一任务提醒与长任务终态

**场景**: 在既有任务提醒独立窗口基础上增加上线包终态通知，并允许直接打开功能页和归档目录。

**解决**:
1. 将窗口生命周期和 FIFO 去重队列提升为全局通知能力，任务提醒与打包结果使用判别联合类型提供各自动作。
2. 打包运行时在真实 overall 终态落定后旁路发送通知；成功、部分成功、失败通知，主动取消不通知。
3. 打开工具页复用主窗口导航事件，打开目录复用 system 域绝对路径校验；操作失败保留通知供重试。

**关键点**:
- 长任务通知是结果旁路，窗口创建或事件发送失败不能改变真实任务终态或回滚产物。
- 通知唯一键来自稳定业务 ID；任务提醒使用 eventId，打包使用 runId，避免初始化与运行期事件重复入队。
- 类型专属动作留在展示层，窗口管理层只负责通知传输、聚焦、定位和生命周期。

**涉及文件**:
- apps/desktop/src-tauri/src/global_notification.rs
- apps/desktop/src-tauri/src/tools/release_package_runtime.rs
- apps/desktop/src/components/GlobalNotificationPopup.vue
- apps/desktop/src/utils/globalNotification.ts

**验证**:
- cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml release_package -- --nocapture
- pnpm --filter @lazycat/desktop exec vitest run src/utils/globalNotification.test.ts src/components/GlobalNotificationPopup.test.ts src/composables/useReleasePackageRuntime.test.ts src/components/ReleasePackagePanel.test.ts
- pnpm typecheck
- pnpm --filter @lazycat/desktop build:web

**使用次数**: 0
