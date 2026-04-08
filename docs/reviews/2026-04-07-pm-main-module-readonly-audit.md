# PM 主模块常识性代码审查结论

- 日期：2026-04-07
- 性质：只读审查，不改代码
- 范围：`PM` 主模块
- 排除范围：`Todo / WeeklyWork`
- 输出方式：按“问题 + 整改顺序”整理，并区分 A / B 两类问题

## 1. 审查口径

本次审查重点判断两类问题：

- A 类：实现偏离现行 spec / 共享语义基线
- B 类：实现可能符合现行 spec，但规则本身明显反直觉，值得复核

审查主线按用户操作路径展开，而不是按前后端技术分层展开。

---

## 2. 建议整改顺序

### 第一批：先修会写错数据的问题
1. 同列重排误写状态流时间戳
2. 三条状态路径的时间戳副作用不一致
3. `siyuanExtraPages` 批量读取口径错误

### 第二批：再修多入口结果不一致的问题
4. 改项目入口不等价
5. 总览与单项目可见集合不一致
6. 删除当前项目后落到空选择态

### 第三批：最后复核展示与规则层问题
7. archived 项目的统计 / 排序 / 总览摘要口径分裂
8. 删除项目的真实阻断原因解释不足
9. archived 规则本身是否要继续维持“仅视觉归档”
10. 流转记录是否应该允许手工编辑

---

## 3. A 类问题：实现偏离现行规格 / 共享语义基线

### A1. 同列重排会误改 `startedAt / testingAt / completedAt`

**问题**

看板里同列重排，本应只改 `sortOrder`，当前却会把整列卡片统一带上 `status` 提交；后端只要收到 `status` 就进入状态时间戳逻辑，导致重排动作污染真实流转时间。

**证据**

- 前端同列 / 跨列都统一生成 `{ id, sortOrder, status }`：`apps/desktop/src/components/PmPanel.vue:2600`
- 前端调用 `tool:pm:item-reorder`：`apps/desktop/src/components/PmPanel.vue:2610`
- 后端只要 payload 带 `status` 就走 `reorder_with_timestamps()`：`apps/desktop/src-tauri/src/tools/pm.rs:1979`
- `reorder_with_timestamps()` 不判断状态是否真的变化：`apps/desktop/src-tauri/src/tools/pm.rs:1920`
- `done` 路径会直接写 `Some(now)` 到 `completed_at`：`apps/desktop/src-tauri/src/tools/pm.rs:1948`

**影响**

- `done` 列同列重排会刷新 `completedAt`
- `todo` 列同列重排会清空 `startedAt / testingAt / completedAt`
- 用户只是做排序，系统却改了“真实开始 / 测试 / 完成时间”

**整改顺序**

- 第 1 优先级

---

### A2. `item_update` / `item_change_status` / `item_reorder` 三条状态路径副作用不一致

**问题**

同样是状态变化，三条后端路径的时间戳策略不同，导致不同入口对同一工作项做相似动作时，写回结果不一致。

**证据**

- 编辑保存走 `item_update`：`apps/desktop/src/components/PmPanel.vue:2358`
- 快捷推进走 `item_change_status`：`apps/desktop/src/components/PmPanel.vue:2478`
- 看板拖拽走 `item_reorder`：`apps/desktop/src/components/PmPanel.vue:2610`
- `item_update` 只有 `status_changed` 才自动改时间戳：`apps/desktop/src-tauri/src/tools/pm.rs:1795`
- `item_change_status` 不判断旧状态：`apps/desktop/src-tauri/src/tools/pm.rs:1871`
- `item_reorder` 也不判断旧状态：`apps/desktop/src-tauri/src/tools/pm.rs:1964`

**影响**

- “编辑保存”“快捷推进”“拖拽改状态”三条路径结果不等价
- 用户难以建立稳定心智
- 后续任何依赖流转记录的展示或统计都可能漂移

**整改顺序**

- 第 2 优先级

---

### A3. `siyuanExtraPages` 批量读取列名与真实表结构不一致

**问题**

`pm_item_siyuan_links` 表实际使用 `doc_id / doc_title / doc_hpath / doc_path / notebook_id / notebook_name`，但批量读取函数查的是 `siyuan_doc_id / siyuan_doc_title / siyuan_doc_hpath / siyuan_doc_path / siyuan_notebook_id / siyuan_notebook_name`，两边口径不一致。

**证据**

- 表结构定义：`apps/desktop/src-tauri/src/tools/helpers.rs:489`
- 表真实列名 `doc_id / doc_title / doc_hpath / doc_path / notebook_id / notebook_name`：`apps/desktop/src-tauri/src/tools/helpers.rs:492`
- 单项读取用正确列名：`apps/desktop/src-tauri/src/tools/pm.rs:773`
- 保存也用正确列名：`apps/desktop/src-tauri/src/tools/pm.rs:814`
- 批量读取用错误列名：`apps/desktop/src-tauri/src/tools/pm.rs:1548`
- `item_list()` 用批量结果回填 `siyuanExtraPages`：`apps/desktop/src-tauri/src/tools/pm.rs:1502`

**影响**

- 工作项列表里的 `siyuanExtraPages` 可能缺失或异常
- 编辑弹窗回显与详情区显示口径不稳定
- 同一条数据“单条读取正常，列表回显异常”

**整改顺序**

- 第 3 优先级

---

### A4. archived 项目统计口径偏离现行 spec

**问题**

现行 spec 明确：项目统一混排、排序按项目任务总数、archived 不影响排序口径；但后端当前只给 active 项目算统计，导致 archived 项目在侧栏排序和摘要里被错误压低。

**证据**

- spec 明确项目统一混排、按项目任务总数排序：`docs/superpowers/specs/2026-04-04-pm-visual-unification-design.md:131`
- spec 明确项目是否 archived 不影响排序口径：`docs/superpowers/specs/2026-04-04-pm-visual-unification-design.md:148`
- 前端排序 helper 依赖 `projectItemCounts.total`：`apps/desktop/src/utils/pmVisual.ts:32`
- 后端 `item_counts()` 只统计 active：`apps/desktop/src-tauri/src/tools/pm.rs:1386`
- active 过滤条件：`apps/desktop/src-tauri/src/tools/pm.rs:1392`

**影响**

- archived 项目侧栏排序失真
- 项目卡右上角数字口径失真
- 总览摘要和项目列表不是同一套语义

**整改顺序**

- 第 7 优先级

---

### A5. “改项目”各入口不等价

**问题**

用户从不同入口改项目，得到的规则不同：

- 编辑弹窗：基本只允许 active 项目，当前已归档项目只是“保留当前值”
- 侧栏拖拽：可以直接拖进 archived 项目
- 后端：`item_move_project` / `item_create` 都不校验目标项目状态

**证据**

- active 项目集合：`apps/desktop/src/components/PmPanel.vue:1172`
- 编辑弹窗项目选项：`apps/desktop/src/components/PmPanel.vue:1213`
- 编辑保存先 `item_move_project` 再 `item_update`：`apps/desktop/src/components/PmPanel.vue:2352`
- 侧栏拖拽仅 `item_move_project`：`apps/desktop/src/components/PmPanel.vue:2674`
- 后端项目移动不校验项目状态：`apps/desktop/src-tauri/src/tools/pm.rs:2031`
- 后端新建也不校验项目状态：`apps/desktop/src-tauri/src/tools/pm.rs:1603`

**影响**

- 同样是“改所属项目”，不同入口结果不同
- archived 项目的可操作边界混乱
- 用户会觉得系统前后说法不一致

**整改顺序**

- 第 4 优先级

---

### A6. 总览与单项目视图的可见集合不一致

**问题**

总览只展示 active 项目下的工作项，但点进具体 archived 项目又能看到它自己的工作项。这会造成“同一条数据在总览里消失、在单项目里出现”的展示分裂。

**证据**

- 总览 `item_list()` 只看 active：`apps/desktop/src-tauri/src/tools/pm.rs:1446`
- active 过滤条件：`apps/desktop/src-tauri/src/tools/pm.rs:1455`
- 单项目 `item_list(projectId)` 直接按 `project_id` 查：`apps/desktop/src-tauri/src/tools/pm.rs:1429`
- 单项目查询条件：`apps/desktop/src-tauri/src/tools/pm.rs:1439`

**影响**

- 总览不是“全部工作项总览”，而是“active-only 总览”
- 但 UI 没把这层语义讲清楚
- 容易让用户误判成数据丢失或筛选异常

**整改顺序**

- 第 5 优先级

---

### A7. 总览卡内部统计口径不一致

**问题**

总览卡同时显示：

- 项目数：全量 `projects.length`
- 待办总数：来自 `item_counts()` 聚合结果

但 `item_counts()` 只统计 active 项目，所以总览卡内部其实是“两套口径拼在一起”。

**证据**

- 总览卡展示：`apps/desktop/src/components/PmPanel.vue:28`
- 项目数来自 `projects.length`：`apps/desktop/src/components/PmPanel.vue:31`
- 待办总数来自 `overviewUndoneCount`：`apps/desktop/src/components/PmPanel.vue:35`
- `overviewUndoneCount` 基于 `projectItemCounts`：`apps/desktop/src/components/PmPanel.vue:1175`
- `projectItemCounts` 来源 `item_counts()`：`apps/desktop/src/components/PmPanel.vue:1887`
- 后端 `item_counts()` 只看 active：`apps/desktop/src-tauri/src/tools/pm.rs:1392`

**影响**

- 同一张总览卡里，“项目数”和“待办总数”不是同一层范围
- 用户会误以为总览反映的是全量 PM 空间，实际不是

**整改顺序**

- 第 8 优先级

---

### A8. 删除当前项目后进入空选择态，不回总览

**问题**

删除当前选中的项目后，前端把 `selectedProjectId` 直接置空，而不是切回总览。

**证据**

- 删除项目后置空：`apps/desktop/src/components/PmPanel.vue:2237`
- `loadItems()` 对空 projectId 直接清空：`apps/desktop/src/components/PmPanel.vue:1874`
- 主区域显示“选择一个项目查看看板”：`apps/desktop/src/components/PmPanel.vue:258`
- spec 已把总览定义为全局入口语义：`docs/superpowers/specs/2026-04-04-pm-visual-unification-design.md:102`

**影响**

- 删除当前项目后界面会掉到空白上下文
- 与“总览是稳定入口”的现行设计不一致
- 体验上像是删完后主面板失去落点

**整改顺序**

- 第 6 优先级

---

### A9. 删除项目的真实阻断原因解释不足

**问题**

前端确认框强调的是“会同时删除所有工作项”，但后端真正阻断删除的条件其实是：该项目仍被 Todo 引用。PM 自己的工作项不会阻断删除，只会跟随级联删除。

**证据**

- 前端确认框：`apps/desktop/src/components/PmPanel.vue:2233`
- 后端只检查 `todo_items WHERE project_id = ?1`：`apps/desktop/src-tauri/src/tools/pm.rs:1364`
- Todo 引用阻断提示：`apps/desktop/src-tauri/src/tools/pm.rs:1372`
- PM 工作项通过外键级联删除：`apps/desktop/src-tauri/src/tools/helpers.rs:447`

**影响**

- 用户不知道为什么有些项目删不掉
- 表面看像是 PM 删除，真实阻塞却来自 Todo 关联
- 错误提示虽有说明，但确认框没提前暴露这层风险边界

**整改顺序**

- 第 9 优先级

---

## 4. B 类问题：实现可能符合现行 spec，但规则本身明显反直觉

### B1. archived 项目仍可继续承载工作项，本身反直觉

**现行规格**

- archived 项目仍可点击选中、右键、作为跨项目拖拽目标：`docs/superpowers/specs/2026-04-04-pm-visual-unification-design.md:150`
- 保留交互包括“看板卡片拖拽到项目卡完成跨项目移动”：`docs/superpowers/specs/2026-04-04-pm-visual-unification-design.md:167`

**为什么反直觉**

多数用户会把“归档”理解为：

- 不再参与日常流转
- 不应再接收新的工作项
- 更接近封存 / 历史态

当前规则却更像“只是灰一点的 active 项目”，语义边界很弱。

**影响**

- archived / active 差异感不足
- 用户很难理解 archived 的真实业务含义
- 也正是这条规则，放大了总览、拖拽、编辑口径分叉

**整改顺序**

- 第 10 优先级

---

### B2. 流转记录允许手工编辑，产品语义值得复核

**现状**

编辑弹窗允许直接手改：

- `startedAt`
- `testingAt`
- `completedAt`

**证据**

- 编辑态流转记录区：`apps/desktop/src/components/PmPanel.vue:635`
- 手工编辑 `startedAt`：`apps/desktop/src/components/PmPanel.vue:638`
- 手工编辑 `testingAt`：`apps/desktop/src/components/PmPanel.vue:641`
- 手工编辑 `completedAt`：`apps/desktop/src/components/PmPanel.vue:644`
- spec 明确这些字段放进编辑态“流转记录”区域：`docs/superpowers/specs/2026-04-04-pm-visual-unification-design.md:408`

**为什么反直觉**

如果这些字段要表达“系统真实流转时间”，那允许手工修改会削弱其可信度。如果它们只是“可编辑业务字段”，那又不应被理解成客观时间轨迹。

**影响**

- 后续如果拿它们做回顾、统计、审计，会混淆“系统记录”与“人工修订”
- 也会放大 A1 / A2 中的语义漂移问题

**整改顺序**

- 第 11 优先级

---

## 5. 路径化总览

### 5.1 项目路径
- A4 archived 项目统计 / 排序口径偏离 spec
- A7 总览卡摘要统计口径不一致
- A8 删除当前项目后不回总览
- B1 archived 本身是否还应继续承载工作项

### 5.2 工作项编辑路径
- A3 `siyuanExtraPages` 回显口径错误
- A5 编辑改项目与侧栏拖拽改项目不等价
- B2 流转记录是否应允许手工编辑

### 5.3 状态推进路径
- A2 三条状态路径副作用不一致

### 5.4 拖拽路径
- A1 同列重排误写时间戳
- A2 拖拽与其他状态入口不一致
- A5 跨项目拖拽与编辑改项目不一致

### 5.5 列表展示路径
- A4 archived 项目排序 / 计数口径分裂
- A6 总览与单项目集合不一致
- A7 总览卡内部统计口径不一致

### 5.6 删除与边界路径
- A8 删除当前项目后空选择态
- A9 删除项目阻断原因解释不足
- 删除工作项后详情回收正确，本轮未发现异常：`apps/desktop/src/components/PmPanel.vue:2485`

---

## 6. 一句话总评

这轮 PM 主模块里，最危险的不是 UI 表达，而是“排序动作误写业务时间戳”；其次是 `siyuanExtraPages` 列表读取口径错误；再往后，是 archived 项目的产品语义和统计 / 总览实现长期没完全收口，已经开始在多个入口上表现为前后不一致。
