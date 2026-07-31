# 结构治理路线图首批实施计划（批次 0-1）

- 日期：2026-07-04
- 依据：`docs/superpowers/specs/2026-07-04-structure-refactor-roadmap-design.md`
- 范围：App e2e 安全网与 PM 前端域行为保持拆分
- 约束：每阶段验证通过后独立提交，不混入业务改良

## 总览

| 阶段 | 交付                                     | 验证                            |
| ---- | ---------------------------------------- | ------------------------------- |
| 0    | App.vue Tauri 环境守卫                   | typecheck、build:web、test、e2e |
| 1a   | 18 个 PM 组件迁入 `components/pm/`       | typecheck、build:web、test、e2e |
| 1b   | PmPanel 拆出 3 个 composable 和 2 个组件 | 同上，加 PM 行为清单冒烟        |
| 收尾 | 记录通用结构治理经验                     | 文档检查                        |

## 阶段 0：App.vue Tauri 环境守卫

1. 在 `App.vue` 中使用 `'__TAURI_INTERNALS__' in window` 判断运行环境。
2. 将 setup 顶层 `getCurrentWindow()` 改为守卫后的惰性获取。
3. 非 Tauri 环境的窗口操作使用可选链降级。
4. 执行：
   - `pnpm typecheck`
   - `pnpm --filter @lazycat/desktop build:web`
   - `pnpm test`
   - `pnpm test:e2e`
5. 提交：`fix(app): 修复纯 Web 环境主应用挂载`

## 阶段 1a：PM 域目录搬迁

1. 使用 `git mv` 将 18 个 `Pm*` 组件迁入 `apps/desktop/src/components/pm/`。
2. 保留跨域的 `InlinePmSelector.vue` 和 `InlineTodoList.vue` 在组件根目录。
3. 更新：
   - `apps/desktop/src/tool-registry.ts`
   - `apps/desktop/src/composables/pmViewRegistry.ts`
   - 其他静态 import
4. 执行 typecheck、build:web、test 和 e2e。
5. 提交：`refactor(pm): 组件迁入业务域目录`

## 阶段 1b：PmPanel 拆分

按以下顺序逐项抽取，每一步保持 props down / events up：

1. `composables/usePmContextMenu.ts`
2. `composables/usePmItemFilters.ts`
3. `composables/usePmItemActions.ts`
4. `components/pm/PmSidebar.vue`
5. `components/pm/PmToolbar.vue`

共享状态继续留在 `PmPanel.vue`：项目选择、工作项集合、当前详情、拖拽状态、今日刷新信号、弹窗和抽屉编排。

每一步执行 typecheck、build:web、test 和 e2e。全部完成后冒烟：

- 六个 PM 视图切换
- 工作项创建、编辑、删除和状态推进
- 跨项目拖拽
- 右键菜单
- 思源抽屉
- 今日 badge 刷新
- 详情面板开合

发现行为差异时立即修复或回退对应提交，不继续下一步。

## 收尾

1. 在 `process.md` 记录 e2e 安全网恢复和 Vue 壳层拆分经验。
2. 核对 `PmPanel.vue` 行数变化只作为反馈，不为行数目标追加无职责拆分。
3. 执行完整前端验证。
4. 提交：`docs(process): 记录结构治理批次 0-1 经验`
