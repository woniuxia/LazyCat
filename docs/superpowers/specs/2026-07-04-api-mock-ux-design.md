# API Mock 细节与交互优化设计

## 概述

对已上线的「API Mock」工具做一轮细节与交互优化，聚焦两个方向：联调反馈闭环（日志自动刷新、命中反馈、URL 复制）和编辑效率（Monaco 编辑器、行内启停、复制路由、拖拽排序、未保存拦截），并顺带补充路由级延迟模拟。不改动路由匹配语义、CORS 模型和文件响应机制。

本设计基于 `2026-07-02-api-mock-design.md`（首版设计），该文档中的数据模型、匹配优先级、安全边界继续有效；本文档只描述增量改动。

## 已确认决策

1. 本轮只覆盖「联调反馈闭环」和「编辑效率」两个方向，外加路由级延迟模拟；模板变量、条件匹配、Workbench 转 Mock、导入导出不做。
2. 切换路由/项目时若表单有未保存修改，弹窗三选拦截（保存并切换 / 放弃修改 / 留在当前）。
3. 请求日志做轻量增强：自动刷新、清空、未命中高亮、点击跳转命中路由；不扩展日志字段，不保存 query/header/body。
4. 路由列表和项目列表都做拖拽排序（后端 `route_reorder` / `project_reorder` 已存在，纯补前端）。
5. 延迟模拟为路由级 `delay_ms` 字段，0..=60000 毫秒；配套将请求处理从串行改为每连接一线程，避免延迟阻塞其他请求和停止操作。
6. 实现结构采用组件拆分方案：`ApiMockPanel.vue` 保留编排职责，列表、表单、日志拆到 `components/api-mock/` 子目录。

## 目标 / 非目标

### 目标

1. 服务运行中，请求日志无需手动刷新即可看到新请求。
2. 日志可清空，未命中/错误请求有视觉区分，命中日志可一键跳到对应路由。
3. 项目访问地址和路由完整 URL 可一键复制。
4. 启动服务成功后自动切到日志页。
5. 响应 Body 使用 Monaco 编辑（按 Content-Type 高亮），JSON 可一键格式化。
6. 路由列表行内直接启停，无需打开表单保存。
7. 支持复制路由为新草稿。
8. 路由/项目列表支持拖拽排序，路由排序体现匹配优先级语义。
9. 切换选中对象时未保存修改不再静默丢失。
10. 路由可配置响应延迟，且延迟不影响其他请求的并发处理和服务停止。
11. 保存项目时对端口冲突给出提醒。

### 非目标

1. 不实现模板变量、条件匹配、代理转发、录制回放。
2. 不实现 API Workbench 转 Mock、项目导入导出。
3. 不扩展日志字段（仍不保存 query、header、body），不做日志持久化。
4. 不改变「保存后需重启生效」模型，不做热更新。
5. 不做 HTTPS。

## 前端架构

### 组件拆分

`ApiMockPanel.vue`（当前 1006 行）拆为编排层 + 4 个子组件，子组件放 `components/api-mock/` 子目录（参照 `components/db/` 模式）：

```text
components/ApiMockPanel.vue          编排层：选中态、两个表单对象与 dirty 基线、
                                     IPC 调用、日志轮询 timer、未保存拦截弹窗
components/api-mock/
  ApiMockProjectList.vue             项目列表：拖拽排序、运行状态 tag
  ApiMockRouteList.vue               路由列表：拖拽排序、行内启停开关、复制按钮
  ApiMockRouteForm.vue               路由表单：Monaco、延迟输入、响应头、CORS
  ApiMockLogList.vue                 日志列表：高亮、跳转、清空、自动刷新指示
```

数据流：Panel 持有全部状态（projects / routes / logs / 选中 id / projectForm / routeForm），子组件 props 进、events 出，子组件不自行调用 IPC、不持有持久状态。

### 纯函数（`utils/apiMock.ts`，配单测）

1. `isMockRouteFormDirty(form, baseline)` / `isMockProjectFormDirty(form, baseline)`：表单与基线快照比较。基线在 Panel 每次 assign 表单时序列化记录。
2. `getMockBodyEditorLanguage(contentType)`：Content-Type 到 Monaco language 映射（json / html / xml / css / javascript，其余 plaintext）。
3. `findMockPortConflict(projects, currentId, port)`：返回与当前项目端口相同的其他项目。
4. `getMockLogRowTone(log)`：日志行视觉态派生（未命中/错误 → 警示态，命中 → 普通态）。

### 复用清单

- `MonacoPane.vue`：现成 `modelValue` / `language` / `readOnly` 接口。
- `sortablejs`：数据字典面板同款拖拽实现。
- `navigator.clipboard.writeText`：全项目统一复制模式。
- `ElMessageBox` `distinguishCancelAndClose`：三按钮拦截弹窗。

## 交互设计

### 未保存修改拦截

- 拦截点四处：切换路由、切换项目、新建路由草稿、新建项目草稿。点击日志行跳转路由、复制路由载入草稿同样经过拦截。
- dirty 时弹 `ElMessageBox`（`distinguishCancelAndClose: true`）：确认按钮「保存并切换」、取消按钮「放弃修改」、关闭（X / Esc）为留在当前。
- 「保存并切换」复用现有保存流程；校验失败时留在原地并展示错误，不切换。
- 项目 tab 与路由 tab 之间的切换不拦截：两个表单状态独立共存，切 tab 不重置表单。

### 路由列表

- 行内启停：行右侧 `el-switch`，`click.stop` 防止触发行选中；调用新增 `route_toggle`；成功后刷新项目摘要（启用计数、需重启状态联动）。失败时 toast 错误并刷新列表，开关不留假态。
- 复制路由：行 hover 显示复制图标，点击后将该路由完整配置载入表单并转为草稿（id 置空、名称加「副本」后缀），提示「已创建副本，保存后生效」。
- 拖拽排序：sortablejs 整行拖拽，`filter` 排除开关和按钮元素；落点后提交完整 id 顺序到 `route_reorder`；失败刷新列表恢复真实顺序。列表底部固定说明：「同等级路由按列表顺序优先匹配」。
- 运行中拖拽/启停触发「需重启」提示由现有 signature 机制自动覆盖（signature 已含 `sort_order`，启停改变启用路由集合）。

### 项目列表

- sortablejs 拖拽，提交到 `project_reorder`。纯视觉组织，无匹配语义。

### URL 复制

- 详情区 endpoint 行加复制按钮，复制访问地址（`0.0.0.0` 项目复制 `http://127.0.0.1:<port>`，与现有 `getMockProjectAccessUrl` 一致）。
- 路由表单顶部显示完整 URL 预览（访问地址 + 路径模式，`:param`、`*` 原样保留）并可复制。

### Monaco Body 编辑器

- `MonacoPane` 替换 textarea，高度 280px，language 由 `getMockBodyEditorLanguage` 按 Content-Type 派生。
- Content-Type 为 JSON 时编辑器上方显示「格式化」按钮：`JSON.parse` + 两空格缩进 `JSON.stringify`；解析失败 toast 报错、不改动内容。

### 延迟输入

- 路由表单状态码旁加「延迟 (ms)」`el-input-number`，范围 0..=60000，默认 0（立即返回）。

### 日志页

- 自动刷新：仅当「请求日志 tab 激活 且 当前项目运行中」时每 2 秒轮询 `request_logs`；切走 tab、切换项目、服务停止、组件卸载均清除 timer；轮询连续失败 3 次自动停止并提示。
- 顶部显示「自动刷新中」状态点与「清空」按钮；清空调 `request_logs_clear`，无二次确认。
- 未命中（`routeId` 为空）或有 `error` 的行：左侧红色描边 + 浅红背景。
- 命中行可点击：选中对应路由并切到路由 tab；路由已删除时提示「该路由已删除」。
- 启动服务成功后自动切到日志 tab。

### 端口冲突提醒

- 保存项目时用 `findMockPortConflict` 检查其他项目是否配置相同端口；命中时 warning 提示「端口与项目 X 相同，二者不能同时运行」，不阻断保存（不同时运行则不冲突）。

## 后端改动

### 新增 action

| Channel | Action | 入参 | 行为 |
|---|---|---|---|
| `tool:api-mock:route-toggle` | `route_toggle` | `{id, enabled}` | 单条 UPDATE `enabled` 与 `updated_at`，返回 `{ok: true}` |
| `tool:api-mock:request-logs-clear` | `request_logs_clear` | `{projectId}` | 清空运行态内存日志队列；服务未运行时也返回成功 |

`bridge/tauri.ts` 的 `CHANNEL_MAP` 同步登记两个通道。

### 延迟字段 `delay_ms`

1. 迁移：`api_mock_routes` 加列 `delay_ms INTEGER NOT NULL DEFAULT 0`，使用 helpers.rs 现有「ALTER TABLE 报错即忽略」兼容模式；`CREATE TABLE` 语句同步加列，保证新库旧库结构一致。
2. `route_save` 校验 `0..=60000`，越界拒绝；`route_get` / `route_list` 返回 `delayMs`。
3. `MockRouteSnapshot` 增加 `delay_ms`，并加入 `build_route_signature` 拼接串——修改延迟自动触发「需重启」。
4. 前端 `types/api-mock.ts` 的 `ApiMockRouteSummary` 加 `delayMs`，`RouteForm` 同步。

### 并发模型改造（延迟可用的前提）

现状：accept 循环内串行调用 `handle_http_stream`，一条延迟路由会让同项目其他请求排队，`service_stop` 的 join 也会被 sleep 卡住。改造为：

1. 每个连接 `thread::spawn` 处理：`route_snapshot` 传入服务线程的副本从 `Vec` 改为 `Arc<Vec<MockRouteSnapshot>>`；logs、stop、last_error 本就是 `Arc`。
2. 并发上限 64：原子计数器守卫；超限直接返回 `503` 并写请求日志（本地开发工具，触顶即异常信号）。
3. 延迟 sleep 按 100ms 分片，每片检查 stop 信号；服务停止时中断响应、直接断开连接，不再返回旧配置的响应。
4. `service_stop` 仍只 join accept 线程；连接线程通过 stop 分片检查自行退出，停止延迟上界约一个分片周期。

## 错误处理

1. 行内启停失败：toast 错误 + 刷新路由列表与项目摘要，开关回真实状态。
2. 拖拽 reorder 提交失败：toast 错误 + 刷新列表恢复后端真实顺序。
3. 日志轮询失败：连续 3 次后停止轮询并提示，避免后台无限报错。
4. 「保存并切换」保存失败：留在当前编辑对象，展示具体校验/IPC 错误。
5. JSON 格式化失败：toast 报错，编辑器内容不变。
6. 日志跳转的路由已删除：toast 提示，不切换。
7. 复制路由生成的草稿走完整现有保存校验，无特殊路径。

## 测试计划

### Rust（`cargo test api_mock -- --nocapture`）

1. `route_toggle`：启停写库、返回值、无效 id 报错。
2. `request_logs_clear`：运行中清空生效、未运行返回成功。
3. `delay_ms`：保存/读取、越界拒绝、默认 0。
4. signature：`delay_ms` 变化导致 signature 变化。
5. 延迟冒烟：命中延迟路由的请求耗时 ≥ delay。
6. 并发冒烟：延迟请求进行中，并行快速请求立即返回。
7. 停止及时性：延迟请求进行中执行 stop，服务在分片周期内停止。
8. 既有 api_mock 测试全量保持通过。

### 前端（`pnpm test`）

1. 新增纯函数单测：dirty 比较、language 映射、端口冲突、日志行视觉态。
2. 既有 `utils/apiMock.test.ts` 全量保持通过。

### 集成

```bash
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

实现完成后本地冒烟：启动带延迟路由的项目，验证日志自动刷新、行内启停触发需重启、拖拽排序改变同级匹配结果、停止服务不被延迟卡住。

## 风险与边界

1. 并发模型改造是本轮唯一动响应主链路的改动，靠测试 5/6/7 兜底；其余改动均为增量。
2. 每连接一线程在本地 mock 场景成本可忽略，64 上限防失控；`0.0.0.0` 暴露场景下该上限同时是简单的资源保护。
3. Monaco 替换 textarea 后表单高度增加，`route-grid` 布局在窄窗口（<1180px）下的表现需在实现时检查既有响应式断点。
4. 拖拽与行内开关、行选中在同一行上共存，需通过 sortablejs `filter` 和 `click.stop` 保证互不误触。
