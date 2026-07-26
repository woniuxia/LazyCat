# LazyCat 经验库

本目录保存可复用的工程经验。当前代码、脚本、配置和测试是事实源；经验与实现冲突时，以当前实现和日期更新且已验证的记录为准。

## 使用方式

1. 先从根目录 `AGENTS.md` 判断任务类型；Claude Code 通过 `CLAUDE.md` 自动导入同一份规则。
2. 通过 `process.md` 或下表进入领域文件。
3. 使用 `rg -n "关键词" docs/experience` 定位具体规则。
4. 新经验直接写入对应领域文件，不再把正文追加到 `process.md`。

## 领域索引

| 领域 | 文件 |
|------|------|
| 架构、IPC、Tauri、结构治理 | [architecture.md](./architecture.md) |
| UI、Element Plus、Teleport | [ui-and-styling.md](./ui-and-styling.md) |
| 数据字典 | [data-dictionary.md](./data-dictionary.md) |
| Todo | [todo.md](./todo.md) |
| PM | [pm.md](./pm.md) |
| Spotlight 与启动入口 | [spotlight-and-launcher.md](./spotlight-and-launcher.md) |
| API 与网络工具 | [api-and-network-tools.md](./api-and-network-tools.md) |
| 请求转发 | [request-forward.md](./request-forward.md) |
| 上线包 | [release-package.md](./release-package.md) |
| Windows 构建与发布 | [windows-build-and-release.md](./windows-build-and-release.md) |
| Vault 与 Inbox | [vault-and-inbox.md](./vault-and-inbox.md) |
| 手册与资源 | [manuals-and-resources.md](./manuals-and-resources.md) |
| Agent 协作 | [agent-workflow.md](./agent-workflow.md) |
| 其他工具 | [other-tools.md](./other-tools.md) |

## 迁移审计

迁移基线为 2026-07-21 当前工作区 `process.md` 中的 118 条记录，其中包含尚未提交的“上线包归档终态日志与目录快捷入口”。每条原记录只有一个处理结果：

- `kept`：原条目可不改语义直接保留。
- `merged`：并入当前有效的领域主题；允许合并演进链路和删除旧行为。
- `removed`：功能/结论失效、重复，或只是没有长期复用价值的交付流水。

| # | 日期 | 原标题 | 处理 | 目标 | 依据 |
|---:|------|--------|------|------|------|
| 001 | 2026-07-21 | 上线包归档终态日志与目录快捷入口 | merged | [`release-package.md`](./release-package.md#终态反馈在归档提交之后) | 并入当前有效主题；关键边界仍有效 |
| 002 | 2026-07-21 | Windows 本地打包使用唯一防呆入口 | merged | [`windows-build-and-release.md`](./windows-build-and-release.md#命令决策) | 并入当前有效主题；关键边界仍有效 |
| 003 | 2026-07-20 | 请求转发日志工作台布局与时间筛选优化 | merged | [`request-forward.md`](./request-forward.md#日志工作台) | 并入当前有效结论；旧行为不再作为建议 |
| 004 | 2026-07-20 | 上线包已有归档采用确认后完整替换 | merged | [`release-package.md`](./release-package.md#已有归档完整替换) | 并入当前有效主题；关键边界仍有效 |
| 005 | 2026-07-19 | 请求转发结构化错误与恢复动作 | merged | [`request-forward.md`](./request-forward.md#日志工作台) | 并入当前有效结论；旧行为不再作为建议 |
| 006 | 2026-07-19 | 上线包按目标并行与项目级日志 | merged | [`release-package.md`](./release-package.md#多目标并行但状态独立) | 并入当前有效主题；关键边界仍有效 |
| 007 | 2026-07-19 | 请求转发监听端点快捷操作 | merged | [`request-forward.md`](./request-forward.md#日志工作台) | 并入当前有效结论；旧行为不再作为建议 |
| 008 | 2026-07-19 | 高收益首版工具统一补齐工作流与正确性边界 | merged | [`other-tools.md`](./other-tools.md#首版工具补齐工作流) | 并入当前有效主题；关键边界仍有效 |
| 009 | 2026-07-19 | 请求转发预检状态机行为化测试 | merged | [`request-forward.md`](./request-forward.md#预检显式暴露风险) | 并入当前有效主题；关键边界仍有效 |
| 010 | 2026-07-19 | Cron 方言兼容与输出校验 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#cron-方言显式建模) | 并入当前有效主题；关键边界仍有效 |
| 011 | 2026-07-18 | 访问链路诊断高级参数持久化 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#访问链路诊断阶段化且保持只读) | 并入当前有效结论；旧行为不再作为建议 |
| 012 | 2026-07-18 | 请求转发可读性与双侧栏宽度偏好 | merged | [`request-forward.md`](./request-forward.md#日志工作台) | 并入当前有效结论；旧行为不再作为建议 |
| 013 | 2026-07-18 | 访问链路诊断分阶段引导 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#访问链路诊断阶段化且保持只读) | 并入当前有效结论；旧行为不再作为建议 |
| 014 | 2026-07-18 | 访问链路诊断前端原位替换 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#访问链路诊断阶段化且保持只读) | 并入当前有效结论；旧行为不再作为建议 |
| 015 | 2026-07-18 | 访问链路 TCP、TLS、HTTP 探测 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#访问链路诊断阶段化且保持只读) | 并入当前有效主题；关键边界仍有效 |
| 016 | 2026-07-16 | 高密度工作台分区与连续日志自动刷新 | merged | [`request-forward.md`](./request-forward.md#日志工作台) | 并入当前有效结论；旧行为不再作为建议 |
| 017 | 2026-07-18 | 访问链路诊断契约与输入归一化 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#访问链路诊断阶段化且保持只读) | 并入当前有效结论；旧行为不再作为建议 |
| 018 | 2026-07-18 | 访问链路诊断长任务运行时 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#访问链路诊断阶段化且保持只读) | 并入当前有效结论；旧行为不再作为建议 |
| 019 | 2026-07-18 | 访问链路诊断只读环境适配器 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#访问链路诊断阶段化且保持只读) | 并入当前有效结论；旧行为不再作为建议 |
| 020 | 2026-07-17 | Windows 发布脚本默认只构建 lite portable | merged | [`windows-build-and-release.md`](./windows-build-and-release.md#默认-lite-与完整四包) | 并入当前有效结论；旧行为不再作为建议 |
| 021 | 2026-07-16 | 长生命周期网络服务的运行态与观测一致性 | merged | [`request-forward.md`](./request-forward.md#运行态与持久化意图分离) | 并入当前有效结论；旧行为不再作为建议 |
| 022 | 2026-07-16 | 完整移除抓包工具 | merged | [`architecture.md`](./architecture.md#完整删除跨层功能) | 并入当前有效主题；关键边界仍有效 |
| 023 | 2026-07-14 | SQL 实体生成器基类字段排除 | merged | [`other-tools.md`](./other-tools.md#sql-实体字段排除) | 并入当前有效主题；关键边界仍有效 |
| 024 | 2026-07-11 | 结构治理批次 3（Todo 域）行为保持拆分 | merged | [`architecture.md`](./architecture.md#行为保持的结构拆分) | 并入当前有效主题；关键边界仍有效 |
| 025 | 2026-07-09 | API Mock 响应体格式化扩展到全部文本语言（复用 @lazycat/formatters） | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#api-mock持久配置与运行态分离) | 并入当前有效结论；旧行为不再作为建议 |
| 026 | 2026-07-08 | 契约测试专用接口未标 #[cfg(test)] 导致 44 个 dead_code 警告 | merged | [`architecture.md`](./architecture.md#测试专用接口必须隔离到测试编译) | 并入当前有效主题；关键边界仍有效 |
| 027 | 2026-07-08 | Tauri 窗口缺失 capabilities 白名单导致 Spotlight 事件静默失效 | merged | [`architecture.md`](./architecture.md#tauri-窗口必须同步声明-capability) | 并入当前有效主题；关键边界仍有效 |
| 028 | 2026-07-07 | IPC 契约对账与横切面治理 X1-X4 | merged | [`architecture.md`](./architecture.md#ipc-契约按唯一事实源治理) | 并入当前有效主题；关键边界仍有效 |
| 029 | 2026-07-07 | Spotlight 预取缓存变更用 provider 级事件失效 | merged | [`spotlight-and-launcher.md`](./spotlight-and-launcher.md#动态数据使用-query-time-provider) | 并入当前有效结论；旧行为不再作为建议 |
| 030 | 2026-07-04 | 任务清单快速添加栏（输入行 + 日期/优先级内联速选） | merged | [`todo.md`](./todo.md#表单与受控状态) | 并入当前有效主题；关键边界仍有效 |
| 031 | 2026-07-04 | JsonTreeViewer 查看+编辑扩展（搜索定位/复制菜单/树内编辑/撤销重做/三消费方接入） | merged | [`other-tools.md`](./other-tools.md#通用-json-树遍历留在纯函数层) | 并入当前有效主题；关键边界仍有效 |
| 032 | 2026-07-04 | API Mock 细节交互优化（组件拆分/未保存拦截/延迟模拟/并发改造） | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#api-mock持久配置与运行态分离) | 并入当前有效结论；旧行为不再作为建议 |
| 033 | 2026-07-02 | API Mock Content-Type 选择与响应内容校验前端闭环 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#api-mock持久配置与运行态分离) | 并入当前有效结论；旧行为不再作为建议 |
| 034 | 2026-07-02 | 连通性测试收藏夹用 settings JSON 做轻量持久化 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#轻量收藏使用-settings-json) | 并入当前有效主题；关键边界仍有效 |
| 035 | 2026-07-02 | API Mock 运行服务生命周期与用户反馈要闭环 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#api-mock持久配置与运行态分离) | 并入当前有效结论；旧行为不再作为建议 |
| 036 | 2026-07-02 | 浏览器身份搜索体验保持纯函数复用 | merged | [`spotlight-and-launcher.md`](./spotlight-and-launcher.md#浏览器身份搜索与启动参数分离) | 并入当前有效主题；关键边界仍有效 |
| 037 | 2026-07-02 | 浏览器身份启动器避免复用通用参数拆分 | merged | [`spotlight-and-launcher.md`](./spotlight-and-launcher.md#浏览器身份搜索与启动参数分离) | 并入当前有效主题；关键边界仍有效 |
| 038 | 2026-07-02 | API Mock 运行态与持久配置分离 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#api-mock持久配置与运行态分离) | 并入当前有效结论；旧行为不再作为建议 |
| 039 | 2026-07-01 | 通用 JSON 树视图要把遍历规则留在纯函数层 | merged | [`other-tools.md`](./other-tools.md#通用-json-树遍历留在纯函数层) | 并入当前有效主题；关键边界仍有效 |
| 040 | 2026-07-01 | 番茄钟后台触发与前端倒计时分离 | merged | [`other-tools.md`](./other-tools.md#番茄钟触发与倒计时分离) | 并入当前有效主题；关键边界仍有效 |
| 041 | 2026-06-28 | 数据字典体验状态优先显式建模 | merged | [`data-dictionary.md`](./data-dictionary.md#异步响应绑定当前意图) | 并入当前有效结论；旧行为不再作为建议 |
| 042 | 2026-06-27 | Spotlight 数据字典接入使用 query-time provider | merged | [`data-dictionary.md`](./data-dictionary.md#spotlight-使用查询时-provider) | 并入当前有效结论；旧行为不再作为建议 |
| 043 | 2026-06-27 | 数据字典查询排序使用记录级派生 sort_key | merged | [`data-dictionary.md`](./data-dictionary.md#排序使用记录级-sortkey) | 并入当前有效主题；关键边界仍有效 |
| 044 | 2026-06-27 | 数据字典大 JSON 导入绕开 IPC 文本负载 | merged | [`data-dictionary.md`](./data-dictionary.md#大-json-导入绕开通用-ipc-文本负载) | 并入当前有效主题；关键边界仍有效 |
| 045 | 2026-06-27 | 数据字典关系查询使用字段值派生索引 | merged | [`data-dictionary.md`](./data-dictionary.md#关系查询使用类型化字段值索引) | 并入当前有效主题；关键边界仍有效 |
| 046 | 2026-06-26 | 数据字典字段标题与字段配置排序 | merged | [`data-dictionary.md`](./data-dictionary.md#配置职责必须独立) | 并入当前有效结论；旧行为不再作为建议 |
| 047 | 2026-06-26 | 数据字典左侧导航排序独立持久化 | merged | [`data-dictionary.md`](./data-dictionary.md#配置职责必须独立) | 并入当前有效结论；旧行为不再作为建议 |
| 048 | 2026-06-25 | 数据字典排序配置应作为字典级单一真值 | merged | [`data-dictionary.md`](./data-dictionary.md#排序使用记录级-sortkey) | 并入当前有效结论；旧行为不再作为建议 |
| 049 | 2026-06-25 | 数据字典异步 IPC 结果必须绑定当前意图 | merged | [`data-dictionary.md`](./data-dictionary.md#异步响应绑定当前意图) | 并入当前有效结论；旧行为不再作为建议 |
| 050 | 2026-06-25 | Element Plus 右键菜单本地弹层与函数 ref 时序 | merged | [`ui-and-styling.md`](./ui-and-styling.md#dropdown-打开本地弹层的时序) | 并入当前有效主题；关键边界仍有效 |
| 051 | 2026-06-24 | 数据字典工具采用原始 JSON + 派生检索文本模型 | merged | [`data-dictionary.md`](./data-dictionary.md#原始-json-是唯一业务事实源) | 并入当前有效主题；关键边界仍有效 |
| 052 | 2026-06-24 | Todo 详情编辑标题聚焦失效 | merged | [`todo.md`](./todo.md#表单与受控状态) | 并入当前有效主题；关键边界仍有效 |
| 053 | 2026-06-12 | Vault 存储重构为仅密码加密，Spotlight 支持按账号搜索 | merged | [`vault-and-inbox.md`](./vault-and-inbox.md#仅加密敏感字段) | 并入当前有效主题；关键边界仍有效 |
| 054 | 2026-05-07 | Living Wallpaper（合成壁纸）端到端落地 | merged | [`manuals-and-resources.md`](./manuals-and-resources.md#合成壁纸) | 并入当前有效主题；关键边界仍有效 |
| 055 | 2026-04-19 | PM 视图扩展与列表渐进式渲染 | merged | [`pm.md`](./pm.md#视图通过注册表扩展) | 并入当前有效结论；旧行为不再作为建议 |
| 056 | 2026-04-08 | PM 侧栏排序口径与项目计数口径必须拆开建模 | merged | [`pm.md`](./pm.md#不同统计口径独立建模) | 并入当前有效主题；关键边界仍有效 |
| 057 | 2026-04-08 | 本周工作面板改为按 PM 计划时间命中本周统计 | merged | [`pm.md`](./pm.md#不同统计口径独立建模) | 并入当前有效结论；旧行为不再作为建议 |
| 058 | 2026-04-08 | Base64 面板自动识别前端收口为纯函数校验 + 手动选择持久化 | merged | [`other-tools.md`](./other-tools.md#base64-自动识别与手动偏好分离) | 并入当前有效主题；关键边界仍有效 |
| 059 | 2026-04-07 | 代理规范文档按检索场景重构并补 Agent 防错闸门 | merged | [`agent-workflow.md`](./agent-workflow.md#根规范是执行入口不是知识仓库) | 并入当前有效结论；旧行为不再作为建议 |
| 060 | 2026-04-06 | Windows 正式发版前先处理版本号与已存在 tag 冲突 | merged | [`windows-build-and-release.md`](./windows-build-and-release.md#正式发版前置条件) | 并入当前有效结论；旧行为不再作为建议 |
| 061 | 2026-04-06 | 项目管理甘特图周末日期坐标增加红色圆底 | removed | — | 一次性视觉装饰，当前通用重绘边界已纳入 PM 经验 |
| 062 | 2026-04-06 | 项目管理甘特图首次进入定位改为项目层无动画接管 | merged | [`pm.md`](./pm.md#甘特图-dom-与状态同步) | 并入当前有效结论；旧行为不再作为建议 |
| 063 | 2026-04-05 | 项目管理状态筛选从甘特专用迁移为共享工具栏筛选 | merged | [`pm.md`](./pm.md#状态筛选是共享状态) | 并入当前有效主题；关键边界仍有效 |
| 064 | 2026-04-04 | 项目管理视觉统一规划的后半程收尾 | removed | — | 一次性视觉收尾流水，保留 Teleport 与中间态通用结论 |
| 065 | 2026-04-04 | 项目管理甘特图新增状态多选筛选 | merged | [`pm.md`](./pm.md#状态筛选是共享状态) | 并入当前有效结论；旧行为不再作为建议 |
| 066 | 2026-04-04 | 项目管理工作项新增外部链接字段与打开动作 | merged | [`pm.md`](./pm.md#视图通过注册表扩展) | 并入当前有效主题；关键边界仍有效 |
| 067 | 2026-04-02 | 项目管理工作项弹窗切换为时间范围 + 思源紧凑列表 | merged | [`pm.md`](./pm.md#思源集成保持轻量边界) | 并入当前有效结论；旧行为不再作为建议 |
| 068 | 2026-04-02 | Brainstorming 本地预览在 Windows 仓库内补齐桥接脚本 | merged | [`manuals-and-resources.md`](./manuals-and-resources.md#本地视觉预览) | 并入当前有效主题；关键边界仍有效 |
| 069 | 2026-04-02 | 项目管理思源页面关联弹窗切换为默认位置列表优先 | merged | [`pm.md`](./pm.md#思源集成保持轻量边界) | 并入当前有效结论；旧行为不再作为建议 |
| 070 | 2026-03-30 | 项目管理思源存储位置选择器树节点错乱与轻量目录选择器重构 | merged | [`pm.md`](./pm.md#思源集成保持轻量边界) | 并入当前有效结论；旧行为不再作为建议 |
| 071 | 2026-03-29 | 项目管理接入思源配置与目录树预览首版 | merged | [`pm.md`](./pm.md#思源集成保持轻量边界) | 并入当前有效结论；旧行为不再作为建议 |
| 072 | 2026-03-29 | 项目管理甘特图悬浮卡越界与右键视口重置修复 | merged | [`pm.md`](./pm.md#甘特图-dom-与状态同步) | 并入当前有效结论；旧行为不再作为建议 |
| 073 | 2026-03-29 | 项目管理甘特图交互增强与甘特条右键菜单 | merged | [`pm.md`](./pm.md#甘特图-dom-与状态同步) | 并入当前有效结论；旧行为不再作为建议 |
| 074 | 2026-03-20 | 密码库解锁顺滑度优化首轮落地 | merged | [`vault-and-inbox.md`](./vault-and-inbox.md#vault-会话与显示分离) | 并入当前有效主题；关键边界仍有效 |
| 075 | 2026-03-20 | 主呼出快捷键优先按剪贴板路径打开资源管理器 | merged | [`spotlight-and-launcher.md`](./spotlight-and-launcher.md#剪贴板路径动作) | 并入当前有效主题；关键边界仍有效 |
| 076 | 2026-03-20 | 本地待办最近一周已办改为真实完成时间 + 过去 7 天口径 | merged | [`todo.md`](./todo.md#时间字段语义不可回退混用) | 并入当前有效结论；旧行为不再作为建议 |
| 077 | 2026-03-18 | 本地待办卡片右键菜单落地 | removed | — | 一次性右键交互落地，当前列表规则已由后续记录覆盖 |
| 078 | 2026-03-18 | 本地待办 meta-time 跨自然周文案修复 | merged | [`todo.md`](./todo.md#时间字段语义不可回退混用) | 并入当前有效结论；旧行为不再作为建议 |
| 079 | 2026-03-17 | 收纳箱图片预览、右键菜单与图片回采抑制 | merged | [`vault-and-inbox.md`](./vault-and-inbox.md#inbox-采集抑制) | 并入当前有效主题；关键边界仍有效 |
| 080 | 2026-03-17 | 收纳箱（Inbox Hub）首版打通与跨工具草稿联动 | merged | [`vault-and-inbox.md`](./vault-and-inbox.md#跨工具草稿) | 并入当前有效主题；关键边界仍有效 |
| 081 | 2026-03-16 | release-all-win 正式发版校验、恢复路径与兼容性补强 | merged | [`windows-build-and-release.md`](./windows-build-and-release.md#中断恢复) | 并入当前有效结论；旧行为不再作为建议 |
| 082 | 2026-03-16 | Windows 发版前先统一多处版本号，再走 release 脚本 | merged | [`windows-build-and-release.md`](./windows-build-and-release.md#正式发版前置条件) | 并入当前有效结论；旧行为不再作为建议 |
| 083 | 2026-03-08 | 本地待办清空日期时间后仍显示时间的修复 | merged | [`todo.md`](./todo.md#时间字段语义不可回退混用) | 并入当前有效结论；旧行为不再作为建议 |
| 084 | 2026-03-08 | 本地待办新增/编辑体验收口 | removed | — | 一次性表单体验收口，当前受控表单规则已保留 |
| 085 | 2026-03-08 | 本地待办置顶排序与已办倒序收口 | merged | [`todo.md`](./todo.md#列表分层与逾期判断) | 并入当前有效结论；旧行为不再作为建议 |
| 086 | 2026-03-08 | 本地待办提醒改为独立弹窗窗口 | merged | [`todo.md`](./todo.md#提醒与事件时间分离) | 并入当前有效结论；旧行为不再作为建议 |
| 087 | 2026-03-08 | 本地待办改为双区块展示并前端判定逾期 | merged | [`todo.md`](./todo.md#列表分层与逾期判断) | 并入当前有效结论；旧行为不再作为建议 |
| 088 | 2026-03-16 | Tauri 自定义 manifest 不要与 embed-resource 并用 | merged | [`windows-build-and-release.md`](./windows-build-and-release.md#windows-特有问题) | 并入当前有效主题；关键边界仍有效 |
| 089 | 2026-03-08 | 本地待办移除提醒中心并改为超期/待办/已办三段 | merged | [`todo.md`](./todo.md#提醒与事件时间分离) | 并入当前有效结论；旧行为不再作为建议 |
| 090 | 2026-03-08 | 本地待办编辑态事项类型互转与 5 分钟步进 | merged | [`todo.md`](./todo.md#表单与受控状态) | 并入当前有效结论；旧行为不再作为建议 |
| 091 | 2026-03-07 | 本地待办多提醒与逐条稍后提醒改造 | merged | [`todo.md`](./todo.md#提醒与事件时间分离) | 并入当前有效结论；旧行为不再作为建议 |
| 092 | 2026-03-07 | 本地待办调度区重构为日期/时间/提醒/重复 | merged | [`todo.md`](./todo.md#提醒与事件时间分离) | 并入当前有效结论；旧行为不再作为建议 |
| 093 | 2026-03-07 | 本地待办工具（任务+周期+提醒）一体化落地 | merged | [`todo.md`](./todo.md#当前模型事项实例与周期系列) | 并入当前有效结论；旧行为不再作为建议 |
| 094 | 2026-03-07 | 密码库移除软锁并改为失焦仅隐藏敏感信息 | merged | [`vault-and-inbox.md`](./vault-and-inbox.md#vault-会话与显示分离) | 并入当前有效结论；旧行为不再作为建议 |
| 095 | 2026-03-07 | 密码库分级锁定优先复用现有会话与设置通道 | merged | [`vault-and-inbox.md`](./vault-and-inbox.md#vault-会话与显示分离) | 并入当前有效结论；旧行为不再作为建议 |
| 096 | 2026-03-07 | 命名快捷键二次触发隐藏失败根因为缺少 `core:window:allow-hide` | merged | [`spotlight-and-launcher.md`](./spotlight-and-launcher.md#命名快捷键复用统一协议) | 并入当前有效主题；关键边界仍有效 |
| 097 | 2026-02-21 | 添加 MDN JavaScript 中文手册（Puppeteer 抓取方案） | merged | [`manuals-and-resources.md`](./manuals-and-resources.md#离线手册接入) | 并入当前有效主题；关键边界仍有效 |
| 098 | 2026-02-20 | 六方案全量重构（类型集中化 + Composables + App.vue 拆分 + Rust 模块化 + 构建优化 + CSS 分层） | merged | [`architecture.md`](./architecture.md#行为保持的结构拆分) | 并入当前有效结论；旧行为不再作为建议 |
| 099 | 2026-02-21 | 代码片段页三栏拥挤治理与检索管理迭代（批量能力） | removed | — | 一次性三栏视觉迭代，当前片段状态边界由后续记录覆盖 |
| 100 | 2026-02-21 | 代码片段专属工作区 V2 重构（右键入口 + 新模型 + FTS 检索） | merged | [`other-tools.md`](./other-tools.md#代码片段检索与三栏状态) | 并入当前有效结论；旧行为不再作为建议 |
| 101 | 2026-02-21 | Cron 工具易用性 V2（Spring 6 字段标准 + 5 字段兼容 + 时区预览） | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#cron-方言显式建模) | 并入当前有效结论；旧行为不再作为建议 |
| 102 | 2026-02-21 | 文本处理工具重做（清洗 + 提取 + 双栏统计） | removed | — | 历史功能交付流水，没有保留到当前的非显而易见边界 |
| 103 | 2026-02-21 | Backend Unit Test Expansion for Critical Tool Domains | removed | — | 历史测试扩容报告，验证原则已在当前 Agent 经验统一维护 |
| 104 | 2026-02-27 | release 脚本 Git link.exe 遮蔽 MSVC 链接器 | merged | [`windows-build-and-release.md`](./windows-build-and-release.md#windows-特有问题) | 并入当前有效主题；关键边界仍有效 |
| 105 | 2026-03-07 | 本地待办统一为事项实例 + 周期系列 | merged | [`todo.md`](./todo.md#当前模型事项实例与周期系列) | 并入当前有效结论；旧行为不再作为建议 |
| 106 | 2026-03-07 | 本地待办自动收藏与命名快捷键接入 | merged | [`spotlight-and-launcher.md`](./spotlight-and-launcher.md#命名快捷键复用统一协议) | 并入当前有效结论；旧行为不再作为建议 |
| 107 | 2026-03-07 | 本地待办事件时间与提醒预设重构 | merged | [`todo.md`](./todo.md#提醒与事件时间分离) | 并入当前有效结论；旧行为不再作为建议 |
| 108 | 2026-03-08 | 本地待办合并待办列表并改为前端逾期判断 | merged | [`todo.md`](./todo.md#列表分层与逾期判断) | 并入当前有效结论；旧行为不再作为建议 |
| 109 | 2026-07-07 | 结构治理批次 0-1 行为保持拆分 | merged | [`architecture.md`](./architecture.md#行为保持的结构拆分) | 并入当前有效结论；旧行为不再作为建议 |
| 110 | 2026-07-08 | API Mock CORS 修复与增强 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#api-mock持久配置与运行态分离) | 并入当前有效结论；旧行为不再作为建议 |
| 111 | 2026-07-17 | 请求转发三栏日志工作台改造 | merged | [`request-forward.md`](./request-forward.md#日志工作台) | 并入当前有效结论；旧行为不再作为建议 |
| 112 | 2026-07-18 | 访问链路旧数据迁移与脱敏报告 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#访问链路诊断阶段化且保持只读) | 并入当前有效结论；旧行为不再作为建议 |
| 113 | 2026-07-18 | 访问链路诊断集成 fixture 与终态结论 | merged | [`api-and-network-tools.md`](./api-and-network-tools.md#访问链路诊断阶段化且保持只读) | 并入当前有效结论；旧行为不再作为建议 |
| 114 | 2026-07-18 | 上线包打包采用构建与归档两阶段提交 | merged | [`release-package.md`](./release-package.md#构建与归档是两阶段提交) | 并入当前有效结论；旧行为不再作为建议 |
| 115 | 2026-07-19 | 上线包多命令编辑与紧凑工作台 | merged | [`release-package.md`](./release-package.md#多目标并行但状态独立) | 并入当前有效结论；旧行为不再作为建议 |
| 116 | 2026-07-19 | 上线包归档目录改为项目级配置 | merged | [`release-package.md`](./release-package.md#归档目录是项目配置) | 并入当前有效结论；旧行为不再作为建议 |
| 117 | 2026-07-20 | 请求转发实时日志筛选、暂停与导出 | merged | [`request-forward.md`](./request-forward.md#日志工作台) | 并入当前有效结论；旧行为不再作为建议 |
| 118 | 2026-07-21 | 全局通知窗口统一任务提醒与长任务终态 | merged | [`release-package.md`](./release-package.md#终态反馈在归档提交之后) | 并入当前有效结论；旧行为不再作为建议 |

## 对账结果

- 基线：118
- kept：0
- merged：111
- removed：7
- 合计：118
