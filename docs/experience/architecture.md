# 架构与工程治理经验

适用范围：跨前后端结构、IPC、Tauri 窗口能力、工具接入、富文本、数据目录、模块拆分和删除功能。

关键词：`IPC`、`Tauri capabilities`、`结构治理`、`动作中心`、`dispatch`、`attachments`、`数据目录`

## 目录

- [2026-07-26：跨工具动作使用注册表、适配器与派发状态机](#2026-07-26跨工具动作使用注册表适配器与派发状态机)
- [Tauri 窗口必须同步声明 capability](#tauri-窗口必须同步声明-capability)
- [IPC 契约按唯一事实源治理](#ipc-契约按唯一事实源治理)
- [新增工具沿既有注册链路接入](#新增工具沿既有注册链路接入)
- [富文本编辑与附件共享同一模型](#富文本编辑与附件共享同一模型)
- [数据目录迁移保持可回退](#数据目录迁移保持可回退)
- [行为保持的结构拆分](#行为保持的结构拆分)
- [完整删除跨层功能](#完整删除跨层功能)
- [测试专用接口必须隔离到测试编译](#测试专用接口必须隔离到测试编译)

## 2026-07-26：跨工具动作使用注册表、适配器与派发状态机

**场景**：Todo、提醒等触发源需要启动上线包、开发环境或浏览器身份等其他工具能力。

**问题**：若触发源直接保存目标工具配置、拼装执行参数或调用内部实现，会形成配置双重真值，绕过目标工具已有的确认、安全和运行态约束。

**解决**：动作中心只注册可信动作定义，保存 `trigger + actionType + targetId` 通用绑定，并用 dispatch 状态机跟踪一次执行；组合动作使用代码注册的原子动作和持久化目标引用，配置、步骤快照与运行事实分离。前端通过独立 intent 导航到目标工具，不复用剪贴板或页面草稿状态。

**关键点**：动作定义来自代码注册表，不允许数据库注入任意命令；串行和并行只作为组合级模式，单步失败独立收口；数据库活动运行唯一索引是全局单运行真值，事件只用于刷新。目标适配器必须复用 Hosts、浏览器身份和请求转发的真实状态与执行链，不能复制配置或通过前端 IPC 自调用；动作中心只持有目标引用、运行关联和结果，不复制目标配置或秘密。

**涉及文件**：

- `apps/desktop/src-tauri/src/tools/action_center/`
- `apps/desktop/src/composables/useActionDispatchIntent.ts`
- `apps/desktop/src/types/action-center.ts`

**验证**：

- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml action_center -- --nocapture`
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml contract_tests -- --nocapture`
- `pnpm --filter @lazycat/desktop test -- src/composables/useActionDispatchIntent.test.ts`

**使用次数**：1

## Tauri 窗口必须同步声明 capability

**场景**：新增独立窗口后，自定义 command 正常，但 event、dialog、notification 等插件调用静默失败。

**根因**：窗口 label 没有加入 `apps/desktop/src-tauri/capabilities/default.json` 的 `windows` 白名单。自定义 command 与插件权限走不同链路，容易造成“部分功能正常”的假象。

**处理**：新增窗口时同步检查窗口创建、capability 白名单、前端监听和插件权限；至少做一次插件调用冒烟。

**使用次数**：2

## IPC 契约按唯一事实源治理

**场景**：channel、前端类型、mock、Rust action 和实际返回结构分别演进，导致参数名、可空性或 action 漂移。

**处理**：以 `bridge/tauri.ts`、集中类型、Rust dispatch 和契约测试形成闭环；跨域修改先列 action 清单，再同步实现与 fixture。不要在组件里复制另一套隐式 payload 定义。

**关键点**：契约守卫负责暴露漂移，不负责用默认值掩盖错误；后端不认识的 action 应显式失败。

**使用次数**：0

## 新增工具沿既有注册链路接入

前端工具通过 `App.vue` 的入口、`tool-registry.ts` 的异步组件和面板组件接入；需要后端时再同步 `bridge/tauri.ts` 的 channel 映射与 `src-tauri/src/tools/` 的模块注册。常规链路是 `invokeToolByChannel` → `tool_execute` → `execute_tool`，不要另起旁路协议。

**使用次数**：0

## 富文本编辑与附件共享同一模型

编辑器和只读 Viewer 必须调用 `buildExtensions()` 使用一致的 TipTap/ProseMirror schema。持久化图片路径保持相对路径，渲染时才转换为资产 URL；文件引用区分复制进附件目录的 `attachment` 与仅保存原路径的 `path`。

附件生命周期由 `useRichDescriptionLifecycle` 统一处理临时绑定、取消时孤儿清理和保存前整理；后端 `attachments.rs` 使用内容寻址存储。修改扩展、节点或附件行为时同步检查编辑器、Viewer 和所有消费方。

**使用次数**：0

## 数据目录迁移保持可回退

- 指针配置固定在 `%USERPROFILE%\.lazycat\config.json`。
- 数据库位于 `<数据目录>\lazycat.sqlite`，Hosts 备份位于 `<数据目录>\hosts-backups`。
- 自定义目录不可达时回退默认目录，不让应用崩溃。
- 迁移复制数据库和 Hosts 备份；目标已存在数据库时拒绝覆盖。

涉及真实用户数据的迁移或覆盖必须先确认影响范围与回退方式。

**使用次数**：0

## 行为保持的结构拆分

**场景**：大型 Vue/Rust 文件职责过多，需要拆分但不能改变行为。

**处理**：先恢复测试基线，再按稳定职责搬迁；每批只做一种结构变化，保持公共入口与数据流不变。组件负责状态编排，筛选、排序、归一化优先放纯函数；Rust 主模块保留 dispatch，业务子域下沉到独立模块。

**验证**：定向测试 → `pnpm typecheck` → `pnpm --filter @lazycat/desktop build:web`；Rust 域补相应 `cargo test`。

**使用次数**：0

## 完整删除跨层功能

**场景**：退役跨越菜单、异步组件、IPC、Rust 模块、依赖和文档的工具。

**处理**：按“入口 → 注册 → IPC/command → 后端模块 → 依赖与锁文件 → 当前文档”闭环删除，并先建立精确标识集合，避免误删同名但不同域的能力。

**现状**：旧 Npcap/PCAP 抓包工具已移除；QuickCapture、Inbox 剪贴板采集、请求转发 HTTP 内容采集等名称相近的能力仍有效，不能按 `capture` 关键词批量清理。

**使用次数**：0

## 测试专用接口必须隔离到测试编译

**场景**：为契约测试暴露的 helper 在正常构建中触发大量 `dead_code` 警告。

**处理**：测试专用函数、模块或 re-export 使用 `#[cfg(test)]` 限定；若生产代码也使用，则提升为真实公共接口并补清晰调用方，不用全局 `allow(dead_code)` 压制。

**使用次数**：0
