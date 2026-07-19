# Request Forward Productivity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 完成请求转发建议 1～8，使 HTTPS 下游、配置预检、快捷操作、复制规则、可恢复错误、自动恢复控制、批量操作和实时日志形成完整且可验证的日常调试工作流。

**Architecture:** 保留 `RequestForwardPanel.vue` 的编排职责，但把纯格式化、筛选、错误映射和导出逻辑放入 `utils/requestForward.ts`；Rust `request_forward` 域继续作为运行真值，新增小而明确的 preflight/lifecycle API。前后端契约集中在 `types/request-forward.ts` 和 `bridge/tauri.ts`，所有行为按 TDD 逐项落地。

**Tech Stack:** Vue 3、TypeScript、Element Plus、Tauri 2、Rust、Tokio、Hyper、hyper-rustls、Vitest、Rust unit/integration tests。

---

## File map

- `apps/desktop/src-tauri/src/tools/request_forward/http.rs`: HTTP/HTTPS 下游连接器与协议测试。
- `apps/desktop/src-tauri/src/tools/request_forward/preflight.rs`: 无副作用预检、空闲端口建议和结构化检查结果。
- `apps/desktop/src-tauri/src/tools/request_forward/runtime.rs`: 运行状态与 `auto_start` 持久化期望解耦。
- `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`: action 分发、结构化错误和日志筛选参数。
- `apps/desktop/src-tauri/src/tools/request_forward/repository.rs`: 扩展日志筛选与稳定分页。
- `apps/desktop/src/types/request-forward.ts`: 所有新增 IPC 类型。
- `apps/desktop/src/utils/requestForward.ts`: 复制规则、端点命令、错误恢复、批量范围、日志格式化/导出纯函数。
- `apps/desktop/src/components/RequestForwardPanel.vue`: 页面编排、preflight、快捷操作、批量结果、日志实时状态。
- `apps/desktop/src/components/request-forward/RequestForwardRuleList.vue`: 规则筛选、多选、复制、批量范围和自动恢复标记。
- `apps/desktop/src/components/request-forward/RequestForwardRuleDialog.vue`: 检测、仅本次启动、启动并自动恢复。
- `apps/desktop/src/components/request-forward/RequestForwardEndpointActions.vue`: 监听/目标复制、打开与命令示例入口。
- `apps/desktop/src/components/request-forward/RequestForwardLogList.vue`: 键盘选择和实时暂停反馈。
- `apps/desktop/src/components/request-forward/RequestForwardLogInspector.vue`: JSON 格式化和复制动作。
- `apps/desktop/src/components/request-forward/RequestForwardPreflightResult.vue`: 预检分阶段结果。
- `apps/desktop/src/components/request-forward/RequestForwardBatchResultDialog.vue`: 批量逐条结果。
- `apps/desktop/src/utils/requestForward.test.ts`、`apps/desktop/src/components/RequestForwardPanel.test.ts`: 前端回归测试。

### Task 1: 支持 HTTPS 下游并消除协议误导

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/http.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/validation.rs`
- Modify: `apps/desktop/src/components/request-forward/RequestForwardRuleForm.vue`
- Test: `apps/desktop/src-tauri/src/tools/request_forward/http.rs`
- Test: `apps/desktop/src-tauri/src/tools/request_forward/runtime.rs`
- Test: `apps/desktop/src/components/RequestForwardPanel.test.ts`

- [ ] 先新增失败测试：本地测试 CA/证书启动 HTTPS 下游，规则能转发 method/path/query/header/body，TLS 验证失败返回明确错误；表单协议标签为“HTTP”，目标 URL 说明支持 HTTP/HTTPS 下游。
- [ ] 运行 `cargo test request_forward::http -- --nocapture`，确认新增 HTTPS 用例因连接器缺失失败。
- [ ] 使用 `hyper-rustls` 的 native roots 构造支持 HTTP/HTTPS 的 Hyper connector；系统根证书正常校验，不提供忽略证书开关；删除 `当前版本暂不支持 HTTPS 下游` 的提前拒绝。测试通过注入只信任本地 fixture CA 的 TLS config 完成，不修改系统证书库。
- [ ] 保持本地监听仍为明文 HTTP，修正文案，避免让用户误认为监听端支持 HTTPS。
- [ ] 运行 `cargo test request_forward -- --nocapture` 和 `pnpm test src/components/RequestForwardPanel.test.ts`。

### Task 2: 增加配置预检、空闲端口建议和“检测并启动”

**Files:**
- Create: `apps/desktop/src-tauri/src/tools/request_forward/preflight.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/types/request-forward.ts`
- Modify: `apps/desktop/src/components/RequestForwardPanel.vue`
- Modify: `apps/desktop/src/components/request-forward/RequestForwardRuleDialog.vue`
- Create: `apps/desktop/src/components/request-forward/RequestForwardPreflightResult.vue`
- Test: `apps/desktop/src-tauri/src/tools/request_forward/preflight.rs`
- Test: `apps/desktop/src/components/RequestForwardPanel.test.ts`

- [ ] 定义契约：`preflight` 接受与 create 相同的规则 payload，返回 `checks: { kind: "listener" | "dns" | "connect"; state: "passed" | "failed" | "warning"; message: string }[]`、`suggestedListenPort: number | null`、`ready: boolean`。预检只能表示检查时刻结果，实际启动仍以 bind/connect 结果为准。
- [ ] 先写失败测试：空闲端口通过、占用端口失败并建议同地址空闲端口、DNS 失败、TCP 连接失败、HTTP/HTTPS 只做 DNS/TCP/TLS 握手而不发送业务请求、UDP 目标只做 DNS 解析并给出“无法证明服务可用”的 warning。
- [ ] 在 `ACTIONS` 和 `CHANNEL_MAP` 注册 `preflight`，使用独立 Tokio runtime/受控超时，不静默吞掉阶段错误。
- [ ] 弹窗增加“检测配置”“检测并启动”和预检结果；端口占用时用户显式点击“使用建议端口”，不自动改表单。
- [ ] “检测并启动”只在本次预检 ready 后进入既有保存/启动链；若实际启动仍失败，按真实错误反馈。
- [ ] 运行针对性 Rust 与前端测试。

### Task 3: 增加监听端点快捷操作

**Files:**
- Modify: `apps/desktop/src/utils/requestForward.ts`
- Modify: `apps/desktop/src/utils/requestForward.test.ts`
- Modify: `apps/desktop/src/components/RequestForwardPanel.vue`
- Create: `apps/desktop/src/components/request-forward/RequestForwardEndpointActions.vue`
- Test: `apps/desktop/src/components/RequestForwardPanel.test.ts`

- [ ] 先写纯函数失败测试：HTTP 本地 URL、IPv6 URL、TCP/UDP `host:port`、PowerShell `Invoke-WebRequest` 与 curl 示例均正确转义；非 HTTP 不生成浏览器 URL。
- [ ] 实现 `getRequestForwardLocalEndpoint()`、`getRequestForwardLocalUrl()`、`getRequestForwardCommandExamples()`。
- [ ] 工作台标题区加入“复制监听地址”“复制目标地址”；HTTP 增加“浏览器打开”和命令下拉。剪贴板/打开失败必须显示错误，不吞异常。
- [ ] HTTP 访问通配监听时把 `0.0.0.0` 映射为 `127.0.0.1`、`::` 映射为 `::1`；TCP/UDP 不显示浏览器和 HTTP 命令操作。
- [ ] 运行 utils 与组件测试。

### Task 4: 增加复制规则

**Files:**
- Modify: `apps/desktop/src/utils/requestForward.ts`
- Modify: `apps/desktop/src/utils/requestForward.test.ts`
- Modify: `apps/desktop/src/components/request-forward/RequestForwardRuleList.vue`
- Modify: `apps/desktop/src/components/RequestForwardPanel.vue`
- Test: `apps/desktop/src/components/RequestForwardPanel.test.ts`

- [ ] 先写失败测试：`duplicateRequestForwardRuleForm(rule)` 保留协议/目标/采集配置，去除 id/autoStart/时间字段，名称追加“副本”，端口由调用方提供的建议值替换。
- [ ] 规则菜单增加“复制规则”，用独立 editor intent 打开 create 弹窗，不能切换当前观测规则。
- [ ] 打开复制弹窗后自动调用 Task 2 的 preflight；若端口冲突只展示建议端口，不静默改值。
- [ ] 保留“仅保存/保存并启动”现有行为并增加组件结构测试。

### Task 5: 结构化运行错误与可执行恢复入口

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/model.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/runtime.rs`
- Modify: `apps/desktop/src/types/request-forward.ts`
- Modify: `apps/desktop/src/utils/requestForward.ts`
- Modify: `apps/desktop/src/utils/requestForward.test.ts`
- Modify: `apps/desktop/src/components/RequestForwardPanel.vue`

- [ ] 定义稳定错误码：`listener_in_use`、`dns_failed`、`target_unreachable`、`tls_failed`、`self_forward`、`invalid_config`、`lifecycle_conflict`、`persistence_failed`、`unknown`；错误对象同时保留 `message` 和实际 `state`。
- [ ] 先写失败测试覆盖常见错误分类，未知底层错误不得丢失原文。
- [ ] IPC 错误仍通过现有 Result 失败通道暴露，但字符串载荷采用可识别 JSON envelope；前端解析失败时回退原始字符串，避免破坏其他工具通用 bridge。
- [ ] 状态区按错误码提供“重新启动”“编辑规则”“检测目标”“使用建议端口”中的适用动作，并提供“查看技术详情”。
- [ ] 批量结果复用同一错误分类，不重复字符串匹配。

### Task 6: 显式控制本次运行与自动恢复

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/runtime.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Modify: `apps/desktop/src/types/request-forward.ts`
- Modify: `apps/desktop/src/components/request-forward/RequestForwardRuleList.vue`
- Modify: `apps/desktop/src/components/request-forward/RequestForwardRuleDialog.vue`
- Modify: `apps/desktop/src/components/RequestForwardPanel.vue`
- Test: Rust runtime/mod tests and front-end tests

- [ ] 先写失败测试：`start/stop/start_all/stop_all` 只改变本次 runtime，不再隐式写 `auto_start`；`auto-start-update { id, enabled }` 只改变下次启动期望；应用退出仍不改变期望。
- [ ] 从 runtime 启停路径移除 `AutoStartPersistence` 和持久化失败补偿分支；保留恢复启动、停止清理、状态串行化和实际状态真值。
- [ ] 列表展示“随应用启动”标记；弹窗和单条启动入口提供“仅本次启动”（先设 false 再 start）与“启动并自动恢复”（先设 true 再 start）；运行中允许切换下次启动自动恢复；停止默认保留期望，并提供“停止并取消自动恢复”。
- [ ] 自动恢复失败继续保留 `autoStart=true` 和 failed 状态。
- [ ] 运行完整 request_forward Rust 测试与前端测试。

### Task 7: 改进批量范围与逐条结果

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Modify: `apps/desktop/src/types/request-forward.ts`
- Modify: `apps/desktop/src/utils/requestForward.ts`
- Modify: `apps/desktop/src/utils/requestForward.test.ts`
- Modify: `apps/desktop/src/components/request-forward/RequestForwardRuleList.vue`
- Modify: `apps/desktop/src/components/RequestForwardPanel.vue`
- Create: `apps/desktop/src/components/request-forward/RequestForwardBatchResultDialog.vue`
- Test: Rust mod tests and front-end tests

- [ ] 先写失败测试：显式 ID 列表批量启停只作用于指定规则；非法/重复 ID 归一化；筛选结果和多选范围计算稳定；失败结果包含规则名、错误码、原文与状态。
- [ ] `start_all/stop_all` 接受可选 `ids`，缺失时保持旧版全量语义；前端始终显式提交当前范围。
- [ ] 规则列表增加状态筛选与多选；批量按钮文案明确“选中 N 条”“当前筛选 N 条”或“全部 N 条”。
- [ ] 停止批量操作前确认影响数量；结果组件逐条展示成功/失败，失败行支持定位、重试和编辑。
- [ ] 运行针对性测试。

### Task 8: 完善实时日志调试

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/repository.rs`
- Modify: `apps/desktop/src-tauri/src/tools/request_forward/mod.rs`
- Modify: `apps/desktop/src/types/request-forward.ts`
- Modify: `apps/desktop/src/utils/requestForward.ts`
- Modify: `apps/desktop/src/utils/requestForward.test.ts`
- Modify: `apps/desktop/src/components/RequestForwardPanel.vue`
- Modify: `apps/desktop/src/components/request-forward/RequestForwardLogList.vue`
- Modify: `apps/desktop/src/components/request-forward/RequestForwardLogInspector.vue`
- Test: Rust repository/mod tests and front-end tests

- [ ] 扩展日志查询契约：`method`、`statusCode`、`startedAt`、`endedAt`，先筛选、按 `created_at DESC,id DESC` 稳定排序，再分页；TCP/UDP method/status 条件返回空结果而不是忽略条件。
- [ ] 先写失败测试覆盖所有新筛选、组合筛选、边界时间和分页 total。
- [ ] 增加实时/暂停状态；暂停时后台只探测最新 total 并累计新增数量，不替换可见窗口，恢复后从 offset 0 重建连续窗口。
- [ ] 工具栏增加清空筛选、Method、状态码、时间范围；规则切换后按当前规则重新查询，不让旧响应覆盖。
- [ ] 日志列表支持上下方向键移动选择并保持焦点语义。
- [ ] 详情对 JSON Content-Type 正文做安全 pretty print，原始非 JSON 文本不变；增加复制错误、headers、body、完整日志。
- [ ] 使用现有 Tauri dialog/save 模式按当前筛选重新查询并导出 JSON 或 CSV，最多导出后端保留上限 1000 条；文件名带规则名和本地时间，JSON 元数据包含 `total/exported/truncated/filters`，界面明确提示截断。
- [ ] 运行 `cargo test request_forward -- --nocapture`、前端针对性测试、`pnpm typecheck` 和 `pnpm --filter @lazycat/desktop build:web`。

## Final verification

- [ ] `cargo test request_forward -- --nocapture`
- [ ] `cargo test contract_tests -- --nocapture`
- [ ] `pnpm test src/utils/requestForward.test.ts src/components/RequestForwardPanel.test.ts`
- [ ] `pnpm test`
- [ ] `pnpm typecheck`
- [ ] `pnpm --filter @lazycat/desktop build:web`
- [ ] 检查 `process.md` 并记录本次 3+ 文件的稳定经验。
