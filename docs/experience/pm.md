# PM 经验

适用范围：PM 视图、共享筛选、甘特图、工作项、思源集成和列表性能。

关键词：`pmViewRegistry`、`pmStatusFilter`、`Gantt`、`Siyuan`、`sort_key`

## 视图通过注册表扩展

`pmViewRegistry` 统一注册 kanban/gantt/today/list/calendar/matrix；`PmPanel` 动态渲染当前视图。视图记忆按 overview / project 上下文保存。新增视图优先修改注册表与独立视图组件，不在主面板堆条件分支。

## 状态筛选是共享状态

状态筛选已从甘特专用迁移到 PM 顶部共享工具栏，同时服务看板和甘特。保持 `baseFilteredItems` 与 `statusFilteredItems` 分层；看板只渲染选中状态列，拖拽实例也只绑定可见列。旧 `pmGanttFilter` 方案已废弃。

## 不同统计口径独立建模

侧栏排序、项目计数、视图 badge 与业务查询不是同一问题，不共用一个宽松计数函数。时间字段也按计划、完成、更新时间分别处理，不松散回退。

## 甘特图 DOM 与状态同步

`frappe-gantt` 头部坐标包含 HTML 节点；装饰和选中态要覆盖 render、refresh、change_view_mode 三条重绘链路。初次定位与刷新保持滚动是两个状态，分别建模。悬浮卡、右键菜单按视口约束定位。

## 思源集成保持轻量边界

位置选择器、页面列表和关联弹窗使用明确的异步意图；优先展示默认位置内容，树选择不复用不稳定的临时索引。导入、导出、页面关联通过 `usePmSiyuan` 收口。

## 大列表渐进渲染

无分组且数据超过 500 行时初始渲染 200 行，滚动追加 200；筛选和排序仍对完整集合执行，不能只在已渲染切片上计算。

**使用次数**：0
