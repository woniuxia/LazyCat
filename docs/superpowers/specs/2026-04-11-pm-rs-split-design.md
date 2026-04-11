# 项目管理后端 `pm.rs` 拆分设计

- 日期：2026-04-11
- 范围：`apps/desktop/src-tauri/src/tools/pm.rs` 及其同级子模块文件
- 状态：已形成实施方案，待用户确认

## 1. 背景

当前项目管理后端能力集中在单个文件 `apps/desktop/src-tauri/src/tools/pm.rs` 中。经代码核查，该文件当前约 `3717` 行，已经同时承载以下职责：

1. `pm` tool 的 action 分发入口。
2. 通用 payload 解析与时间 helper。
3. 思源相关数据模型、HTTP 调用、目录构建、搜索解析与页面创建逻辑。
4. 项目 CRUD。
5. 工作项 CRUD、状态推进、标签与页面关联处理。
6. 周工作汇总。
7. PM-Todo 关联能力。
8. 全部单元测试。

当前问题不在于功能不可用，而在于单文件边界已经明显过宽：

1. 日常修改任一 PM 子能力时，都需要在超大文件中跳转定位。
2. 思源、周工作、PM-Todo 关联这三块能力相对独立，但仍与主 CRUD 混放在一起。
3. 测试与实现代码全部堆在同一文件尾部，进一步拉高阅读成本。
4. 后续继续在 `pm.rs` 中叠加功能，会持续降低维护效率并提高误改风险。

本次用户要求是：在当前 PM 相关代码集中于一个文件的前提下，进行适当拆分。

## 2. 改动目标

本次拆分的明确目标如下：

1. 将 `pm.rs` 从超大单文件收敛为“父模块 + 若干领域子模块”的结构。
2. 优先拆出已经具备明显领域边界的能力块，降低后续查找与修改成本。
3. 保持现有 `pm` tool domain 不变，不改变前端 bridge、channel 名称与调用方式。
4. 保持现有业务行为、数据库结构、返回 JSON 结构与错误语义不变。
5. 保持最小必要重构，不借本次拆分顺带改业务逻辑或引入新的抽象层。
6. 为后续继续拆分 `PmPanel.vue` 或 PM 后端其他子域打下更清晰的模块边界。

## 3. 非目标

本次设计明确不包含以下内容：

1. 不修改 `tool:pm:*` 的 action 名称、参数结构或响应结构。
2. 不调整数据库 schema，不新增 migration，不修改现有表字段。
3. 不顺带重构前端 `PmPanel.vue`。
4. 不顺带重写项目 CRUD / item CRUD 的业务逻辑。
5. 不引入新的 service 层、trait 抽象或跨文件复杂框架。
6. 不追求一次性把 PM 后端完全细分到极小粒度模块。

也就是说，本次目标是“先把最重、最独立的几块从单文件中拆出去”，而不是过度模块化。

## 4. 已确认的拆分判断

### 4.1 保留 `pm.rs` 作为父模块

`apps/desktop/src-tauri/src/tools/mod.rs` 当前通过：

- `pub mod pm;`
- `"pm" => pm::execute(action, payload)`

完成 `pm` 域注册。

因此本次不调整 `tools/mod.rs` 的路由方式，而是继续保留 `apps/desktop/src-tauri/src/tools/pm.rs` 作为父模块入口文件，负责：

1. 保留 `pub fn execute(action, payload)` 作为唯一外部入口。
2. 在文件内部声明并聚合若干 PM 子模块。
3. 保留少量跨子模块共享的常量与基础 helper。
4. 继续承载项目 CRUD、工作项 CRUD 这些与主 PM 数据模型耦合最紧的逻辑。

这样可以在不改变外部调用链的前提下完成结构拆分。

### 4.2 本轮优先拆出的 3 个子域

经现有代码结构核查，本轮最适合优先拆出的子域如下：

1. 思源集成
2. 周工作汇总
3. PM-Todo 关联

原因：

1. 这三块都具备相对独立的业务主题。
2. 它们在 `pm.rs` 中占据大量篇幅，拆出后收益明显。
3. 它们对外仍通过 `pm.execute()` 暴露 action，不需要前端改协议。
4. 与项目 CRUD / item CRUD 相比，这三块更容易形成清晰边界。

### 4.3 `item CRUD` 暂不继续细分

虽然 `item CRUD` 仍然较大，但它与以下内容存在较紧耦合：

1. 标签保存。
2. 状态流转时间戳计算。
3. 页面主关联与附加关联的保存。
4. 项目归属校验。

其中一部分逻辑又直接依赖思源页面解析 helper。

因此本轮不继续把 `item CRUD` 再拆成更多子模块，而是优先完成高收益、低风险的第一阶段拆分。拆分完成后，`pm.rs` 体量预计将从约 `3717` 行下降到约 `1300-1500` 行，已经足以显著改善可维护性。

## 5. 目标文件结构

本次拆分后的目标结构如下：

```text
apps/desktop/src-tauri/src/tools/
  pm.rs
  pm_siyuan.rs
  pm_weekly.rs
  pm_todo_link.rs
```

### 5.1 `pm.rs`

父模块，保留内容：

1. `execute()` 分发入口。
2. 通用常量：`STATUSES`、`ITEM_TYPES`、`PRIORITIES` 等。
3. 基础 helper：如 `parse_i64`、`parse_string`、`parse_string_array`、`now_rfc3339`。
4. 项目 CRUD：`project_list/create/update/archive/restore/delete`。
5. item counts。
6. 工作项 CRUD：`item_list/create/update/change_status/reorder/toggle_pin/delete/move_project`。
7. 标签聚合与与 item CRUD 强耦合的 helper。
8. 父模块级测试，或仍适合保留在父模块的测试。

### 5.2 `pm_siyuan.rs`

负责思源集成相关类型、helper 与 action handler，包括：

1. 思源相关 struct：
   - `SiyuanConfig`
   - `SiyuanNotebook`
   - `SiyuanDocRow`
   - `SiyuanTreeNode`
   - `SiyuanNotebookDirectory`
   - `SiyuanDirectoryResult`
   - `SiyuanLocation`
   - `SiyuanPageRef`
   - `SiyuanSearchPagesResult`
2. 思源配置读取、URL 规范化、错误归一化。
3. HTTP 调用与响应 envelope 解析。
4. 目录树构建、查询、搜索、页面创建与打开逻辑。
5. 对 item CRUD 仍有复用价值的页面 helper，例如：
   - `parse_siyuan_location_value`
   - `parse_siyuan_page_ref_value`
   - `parse_siyuan_page_ref_array`
   - `build_siyuan_location_from_parts`
   - `build_siyuan_page_ref_from_parts`
   - `load_item_siyuan_links`
   - `save_item_siyuan_links`
6. PM action handler：
   - `siyuan_test`
   - `siyuan_directory`
   - `siyuan_search_pages`
   - `siyuan_create_page`
   - `siyuan_open_page`
   - `open_link`
7. 相关单元测试。

### 5.3 `pm_weekly.rs`

负责周工作汇总相关的日期与聚合逻辑，包括：

1. 日期格式化与解析 helper。
2. 周窗口计算 helper。
3. `weekly_work` action handler。
4. 周工作相关测试。

### 5.4 `pm_todo_link.rs`

负责 PM-Todo 关联子域，包括：

1. `item_todo_list`
2. `item_todo_link`
3. `item_todo_unlink`
4. `item_todo_create`
5. `item_todo_candidates`
6. 相关测试。

## 6. 模块边界与依赖规则

### 6.1 父模块到子模块的关系

`pm.rs` 作为父模块，负责统一 action 分发：

- 项目 / 工作项主 CRUD 仍在父模块内部直接处理。
- 思源、周工作、PM-Todo 关联 action 转发到对应子模块函数。

示意关系：

```text
pm::execute
  ├─ parent-local handlers
  ├─ pm_siyuan::*
  ├─ pm_weekly::*
  └─ pm_todo_link::*
```

### 6.2 子模块对父模块共享能力的使用规则

子模块允许复用父模块中稳定、基础的通用能力，例如：

1. `parse_i64`
2. `parse_string`
3. `parse_string_array`（如需要）
4. `now_rfc3339`
5. 常量：`PRIORITIES`、思源 setting key、timeout 等

这类能力属于“PM 域基础设施”，保留在父模块即可，不需要为了拆分再强行抽出一个新 shared 文件。

### 6.3 思源子模块的公开边界

由于 item CRUD 仍需要使用部分思源页面 helper，因此 `pm_siyuan.rs` 中部分函数需要对父模块可见。

可见性原则：

1. 默认使用 `pub(super)`，仅对 `pm` 父模块暴露。
2. 不为了测试或方便搬运而一律改成 `pub`。
3. 只有 `pm.rs` 明确需要调用的 helper 才公开给父模块。

这样可以避免拆分后出现“所有 helper 全局裸露”的问题。

### 6.4 避免循环依赖

本次拆分要避免以下结构：

1. 父模块依赖子模块 helper。
2. 子模块又反向依赖父模块中的业务 handler。

因此约束如下：

1. 父模块可以调用子模块公开的 helper。
2. 子模块只依赖父模块中的基础 helper / 常量，不依赖父模块中的业务 action handler。
3. 若某个 helper 同时被多个子模块和父模块复用，但又不属于某个具体业务子域，才考虑继续上提到父模块。

## 7. `execute()` 的收口方式

`execute()` 继续保留在 `pm.rs`，但分发逻辑调整为“父模块内分发 + 子模块转发”的混合模式。

分发原则：

1. 不修改现有 action 名称。
2. 不修改前端 channel map。
3. 不引入二次分发注册表或动态映射，继续使用显式 `match`。

示意：

- `project_*`、`item_*`、`tag_list` 继续在父模块直接处理。
- `weekly_work` 转发到 `pm_weekly::weekly_work`。
- `siyuan_*` 与 `open_link` 转发到 `pm_siyuan::*`。
- `item_todo_*` 转发到 `pm_todo_link::*`。

这样既保留当前代码的直观性，也能把实现细节下沉到对应文件。

## 8. 测试迁移策略

当前 `pm.rs` 文件尾部的 `#[cfg(test)] mod tests` 已覆盖以下几类测试：

1. 周工作日期窗口。
2. 状态流转时间戳。
3. reorder 逻辑。
4. 思源目录与搜索 helper。
5. 思源链接构建。
6. 项目状态校验。
7. SQL 构建等。

本次拆分后的测试策略如下：

### 8.1 按领域迁移测试

优先按所属领域将测试迁移到对应文件：

1. 思源相关测试迁移到 `pm_siyuan.rs`。
2. 周工作相关测试迁移到 `pm_weekly.rs`。
3. PM-Todo 关联测试迁移到 `pm_todo_link.rs`。
4. 项目 / item 主流程相关测试保留在 `pm.rs`。

### 8.2 测试不为迁移牺牲边界

不应为了让旧测试最小改动运行，而把大量内部 helper 提升为 `pub`。

优先顺序应为：

1. 调整测试位置。
2. 使用 `pub(super)` 暴露必要 helper。
3. 仅在确有需要时才扩大可见性。

### 8.3 测试目标保持不变

本次是结构性拆分，不是行为重构，因此测试目标应保持：

1. 行为语义不变。
2. 错误信息口径不变。
3. 关键 helper 的既有边界条件不变。

## 9. 实施步骤

建议按以下顺序实施：

### 9.1 第一步：建立模块骨架

1. 在 `apps/desktop/src-tauri/src/tools/` 下新增：
   - `pm_siyuan.rs`
   - `pm_weekly.rs`
   - `pm_todo_link.rs`
2. 在 `pm.rs` 顶部声明对应子模块。
3. 先完成编译层级的模块可见性打通。

### 9.2 第二步：优先迁移思源子域

1. 搬运思源 struct、helper、action handler。
2. 在 `pm.rs` 中改为从 `pm_siyuan` 调用。
3. 收敛 `pub(super)` 可见性。
4. 迁移思源相关测试。

优先迁移思源块的原因是：

1. 体量最大。
2. 对 item CRUD 仍有少量反向复用。
3. 先收口该块后，其他拆分会更清晰。

### 9.3 第三步：迁移周工作与 PM-Todo 关联

1. 将 `weekly_work` 及日期 helper 迁移到 `pm_weekly.rs`。
2. 将 `item_todo_*` 相关逻辑迁移到 `pm_todo_link.rs`。
3. 同步迁移对应测试。

### 9.4 第四步：整理父模块

1. 清理已迁移的旧实现。
2. 收敛 `use` 列表。
3. 清理不再使用的 helper、导入与测试支撑函数。
4. 确认 `execute()` 中 action 分发路径清晰可读。

### 9.5 第五步：验证

至少执行：

1. Rust 编译 / 项目级类型检查所覆盖到的相关验证。
2. 与 PM 后端相关的测试。
3. `pnpm typecheck`
4. 必要时 `pnpm --filter @lazycat/desktop build:web`

## 10. 涉及文件

本次设计预期涉及以下文件：

1. `apps/desktop/src-tauri/src/tools/pm.rs`
2. `apps/desktop/src-tauri/src/tools/pm_siyuan.rs`
3. `apps/desktop/src-tauri/src/tools/pm_weekly.rs`
4. `apps/desktop/src-tauri/src/tools/pm_todo_link.rs`

本次原则上不需要修改：

1. `apps/desktop/src-tauri/src/tools/mod.rs`
2. `apps/desktop/src/bridge/tauri.ts`
3. `apps/desktop/src/components/PmPanel.vue`
4. `apps/desktop/src/types/pm.ts`

若实施中发现仅因 import/格式或测试辅助需要做极小联动，应控制在最小必要范围。

## 11. 风险与控制

### 11.1 可见性收口不当

风险：

为了快速拆分，把大量原本只在 `pm.rs` 内部使用的 helper 全部放大为公开函数，导致边界变松。

控制方式：

1. 默认私有。
2. 仅对父模块使用 `pub(super)`。
3. 不把“测试方便”作为扩大可见性的默认理由。

### 11.2 搬运后漏改 action 分发

风险：

代码已搬到子模块，但 `execute()` 仍指向旧实现或遗漏某个 action，导致运行时行为异常。

控制方式：

1. 逐项对照现有 `execute()` 的 action 列表迁移。
2. 搬运后重新检查 28 个 action 是否全部有落点。

### 11.3 思源 helper 迁移引发 item CRUD 编译错误

风险：

`item_create` / `item_update` 等仍依赖页面 helper，拆分思源子模块时容易遗漏导入或可见性设置。

控制方式：

1. 先识别 item CRUD 对思源 helper 的所有依赖。
2. 思源块迁移完成后先确保父模块编译通过，再继续后续拆分。

### 11.4 结构拆分演变成业务重构

风险：

在搬运过程中顺手重写 SQL、调整错误信息或改动返回结构，扩大任务范围。

控制方式：

1. 本次以“行为不变”为第一原则。
2. 除非发现明确 bug，否则不借拆分顺手改业务语义。
3. 若发现值得修复的独立问题，单独记录，不与本次结构拆分混做。

## 12. 预期结果

拆分完成后，PM 后端结构将从“单一超大文件”收敛为“父模块聚合 + 领域子模块实现”的形式，达到以下结果：

1. `pm.rs` 不再同时承载全部 PM 后端实现。
2. 思源、周工作、PM-Todo 关联均拥有独立代码落点。
3. 现有外部调用链保持不变，前端无需感知此次拆分。
4. 后续继续维护 PM 功能时，可以按领域直接定位目标文件。
5. 后续若仍需要继续拆分 item CRUD 或前端 `PmPanel.vue`，已有更清晰的参考边界。
