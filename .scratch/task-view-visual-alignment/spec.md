Type: task
Labels: ready-for-agent

# 任务清单双视图展示对齐

## Problem Statement

用户在“任务清单”工具中切换“我的任务”和“关注事项”时，两边虽然属于同一个工具，但使用了不同的三栏比例、栏间距、面板边界、工具栏高度、内容起点和响应式规则。切换会让侧栏、列表和详情区域明显移动，形成页面整体跳动，削弱同一工作区应有的连续感。

这种差异在窄窗口下更加明显：“我的任务”和“关注事项”会在不同宽度进入不同布局模式，切换入口还可能随侧栏移动到底部或被隐藏。用户需要两个视图保持各自领域语义和操作能力，同时在视觉上表现为同一套稳定的任务工作区。

## Solution

以现有“我的任务”页面作为视觉、密度和面板样式基准，让“关注事项”复用同一套任务工作区骨架。共享骨架统一视图切换区、侧栏、工具栏、列表和详情区域的轨道、间距、表面层级、滚动边界及响应式行为；两个视图继续渲染各自的筛选、列表卡片、详情内容和领域操作。

切换继续保留两个页面实例的局部状态和滚动位置，不通过重新挂载或大范围动画掩盖布局差异。宽屏、中等宽度和窄窗都使用一致的结构规则，切换控件始终可见、可操作，并在待复查数量出现或变化时保持自身位置稳定。

## User Stories

1. As a LazyCat user, I want “我的任务” and “关注事项” to look like two views of the same task workspace, so that switching does not feel like opening an unrelated page.
2. As a LazyCat user, I want “我的任务” to remain the visual baseline, so that the established task-management appearance is preserved.
3. As a LazyCat user, I want the view switch to stay in the same position between views, so that I do not need to reacquire the control after every switch.
4. As a LazyCat user, I want the sidebar boundary to remain stable between views, so that the primary content does not move horizontally.
5. As a LazyCat user, I want the list region to begin at the same horizontal position between views, so that list scanning retains spatial continuity.
6. As a LazyCat user, I want the detail region to begin at the same horizontal position between views, so that selecting and comparing items feels consistent.
7. As a LazyCat user, I want the toolbar region to have a stable vertical footprint, so that list content does not jump up or down when I switch views.
8. As a LazyCat user, I want view-specific toolbar actions to fit inside the shared toolbar region, so that each workflow keeps its necessary commands without changing the page skeleton.
9. As a LazyCat user, I want the two sidebars to use the same spacing rhythm and surface treatment, so that filters and summaries belong to one visual system.
10. As a LazyCat user, I want list and detail panes to use the same border, radius and background hierarchy, so that their structural roles are immediately recognizable.
11. As a LazyCat user, I want关注事项 to use the existing LazyCat semantic theme colors, so that its selected, focused, muted and danger states match “我的任务”.
12. As a LazyCat user, I want关注事项 cards to preserve their responsibility,复查时间 and external-progress information, so that visual alignment does not erase domain meaning.
13. As a LazyCat user, I want Todo cards to preserve their task status, schedule, project and assignment information, so that aligning the workspace does not reduce task functionality.
14. As a LazyCat user, I want the current view to remain clearly indicated, so that the shared visual treatment does not make navigation ambiguous.
15. As a LazyCat user, I want the待复查 badge to remain visible when needed, so that visual stability does not hide required attention.
16. As a LazyCat user, I want the待复查 badge to appear and change count without moving the view label, so that asynchronous data does not introduce a small but repeated jump.
17. As a LazyCat user, I want switching views to preserve my filters, selected item and local view mode, so that visual alignment does not reset my work context.
18. As a LazyCat user, I want switching views to preserve each view's internal scroll position, so that I can resume scanning where I left off.
19. As a LazyCat user, I want empty list and empty detail states to occupy the normal pane structure, so that missing content does not collapse the layout.
20. As a LazyCat user, I want loading and failure feedback to appear without changing the workspace geometry, so that asynchronous operations do not destabilize navigation.
21. As a LazyCat user, I want both views to enter the same responsive mode at the same available content width, so that switching cannot change the number or role of visible columns.
22. As a LazyCat user, I want responsive behavior to follow the actual task workspace width, so that application chrome and window size are accounted for correctly.
23. As a LazyCat user, I want a medium-width workspace to use one consistent list/detail navigation pattern, so that one view does not stack while the other overlays.
24. As a LazyCat user, I want the view switch to remain directly reachable in a narrow window, so that I can always move between “我的任务” and “关注事项”.
25. As a LazyCat user, I want narrow-window details to provide a predictable path back to the list, so that opening an item never traps me away from navigation.
26. As a LazyCat user, I want long titles, descriptions and progress content to wrap or truncate within their panes, so that they do not widen the shared layout.
27. As a LazyCat user, I want each pane to have one clear scroll owner, so that wheel and keyboard scrolling remain predictable.
28. As a keyboard user, I want the view switch and pane actions to retain visible focus states, so that layout alignment does not reduce accessibility.
29. As a user who requests reduced motion, I want view switching to avoid unnecessary movement, so that the interface remains comfortable and immediately usable.
30. As a LazyCat user, I want view switching to remain responsive during rapid repeated changes, so that transitions never block interaction.
31. As a LazyCat user, I want Todo list and calendar modes to remain available inside the aligned workspace, so that the change does not narrow existing task workflows.
32. As a LazyCat user, I want关注事项 groups, filters and lifecycle actions to remain available inside the aligned workspace, so that this visual work does not alter关注事项 behavior.
33. As a LazyCat user, I want existing Todo and关注事项 persistence and reminders to remain unchanged, so that a renderer layout change cannot affect stored data or notification semantics.
34. As a LazyCat user, I want the default clean light appearance to remain intact, so that alignment does not introduce an unrelated visual redesign.
35. As a LazyCat user, I want the aligned workspace to remain fully usable offline, so that the task tool retains LazyCat's offline core path.

## Implementation Decisions

- Use the existing “我的任务” page as the source of truth for visual density, spacing rhythm, panel borders, radii, backgrounds, typography hierarchy and interaction-state treatment.
- Introduce one renderer-local task workspace layout boundary with slots or similarly explicit regions for the view switch, sidebar content, toolbar, list content and detail content. The boundary owns geometry and responsive behavior; it must not own Todo or关注事项 domain state.
- Reuse the shared layout boundary from both views instead of maintaining separate grid and breakpoint definitions. This abstraction is justified by the cross-view invariant that is currently duplicated and drifting.
- Preserve the existing top-level “任务清单” tool and its two views. “我的任务” remains the default view.
- Preserve the current mounted-view continuity used inside the task tool. Switching views must not replace the retained instances with conditional remounting, a second cache or persisted UI snapshots.
- Keep Todo and关注事项 as independent domains in accordance with ADR 0006. Shared UI structure does not permit sharing status machines, persistence, commands, list derivation or lifecycle actions.
- Define the shared workspace against its actual available inline size using the repository's existing container-query approach. Do not derive internal pane behavior only from the application viewport.
- Provide three coherent workspace modes: a wide three-pane mode, a medium sidebar-plus-list mode with detail presented predictably above the list, and a narrow single-content mode with a persistent top-level view switch and a full-content detail presentation.
- Use one set of responsive thresholds for both views. Select final thresholds from the content-fit constraints of the “我的任务” baseline, then validate both sides around the existing risk widths near 760, 900, 1024, 1050 and 1280 pixels.
- Keep the view switch in a stable, reserved layout region in every workspace mode. Hiding a sidebar must not hide the switch or make it reachable only after scrolling through content.
- Reserve a stable badge region inside the “关注事项” switch option. Counts of zero, one and `99+` must not change the outer control size or move the label's visual anchor.
- Give the toolbar region a shared minimum block size, padding and boundary treatment based on “我的任务”. View-specific controls may use their own internal arrangement, but wrapping or conditional controls must not move the list origin under normal supported widths.
- Use consistent pane gaps and minimum widths based on the “我的任务” layout. Pane content must set explicit `min-width: 0` and `min-height: 0` where required to prevent intrinsic content from expanding the grid.
- Assign one vertical scroll owner to each scrollable sidebar, list and detail pane. The outer task workspace must not compete with those panes during normal wide and medium layouts.
- Keep empty, loading and error states inside the reserved pane surfaces. These states must not remove a track or collapse the shared workspace.
- Convert关注事项 surface, text, border, focus, muted, selected and semantic-state colors to the existing `--lc-*` design tokens where an equivalent token exists. Do not introduce a new palette or runtime font dependency.
- Preserve visible domain distinctions. Todo continues to describe work performed by the user;关注事项 continues to describe externally owned matters,责任人,复查时间,关注状态 and external results.
- Do not unify Todo and关注事项 card components merely to obtain visual similarity. Their information hierarchy may remain different within the shared pane dimensions.
- Avoid width, height, top, left or grid-track animations during view switching. Stable geometry is the solution; a crossfade must not be used to conceal structural movement.
- Retain the existing short color, background and shadow transition on the view switch. Any new content-only transition must be optional, brief, interruptible and disabled under `prefers-reduced-motion`; no new transition is required for acceptance.
- Preserve keyboard focus visibility, semantic button behavior and the current active-view announcement. The alignment must not rely on color alone to identify the active view or critical关注事项 states.
- Keep the work renderer-only. Do not add or modify database schema, IPC contracts, scheduler behavior, notification contracts, navigation handoff semantics or persisted settings.
- Keep scoped style ownership explicit. Shared geometry belongs to the shared layout boundary; view-specific content styles remain with their view. Do not depend on parent scoped selectors accidentally reaching child internals.

## Testing Decisions

- Tests must assert observable layout and interaction behavior rather than private component names, incidental DOM nesting or source-code strings.
- Use one primary seam: the existing Playwright desktop Web page running in Chromium with deterministic renderer bridge responses for the task tool. This is the highest existing seam that uses a real layout engine and can observe the user-visible switching problem.
- Exercise the task tool through its visible “我的任务 / 关注事项” switch. Do not call internal component methods to create the primary layout states.
- At the primary seam, measure the view switch bounding box and the sidebar, list and detail anchors before and after switching. At a fixed workspace size, stable anchors should differ by no more than 1-2 CSS pixels unless a documented scrollbar appearance accounts for the difference.
- At the primary seam, verify that the toolbar bottom edge and list content origin remain stable between views in the same responsive mode.
- At the primary seam, verify wide three-pane behavior, medium detail presentation and narrow single-content behavior. Check the critical width transitions immediately before and after the resolved shared thresholds, including the current 760, 900, 1024, 1050 and 1280 pixel risk areas where applicable.
- At the primary seam, verify that the switch remains visible and operable in every responsive mode, including after opening and closing a detail view.
- At the primary seam, cover empty list, populated list, no selected detail, selected detail, loading and visible failure states without track collapse or horizontal overflow.
- At the primary seam, cover Todo list and calendar modes plus关注事项 group changes, because these conditional structures can change toolbar and content geometry.
- At the primary seam, cover long titles, long progress content, filters that produce no result and enough content to require pane scrolling. Assert that each pane scrolls without moving the persistent switch or widening the workspace.
- At the primary seam, cover待复查 badge values zero, one and `99+`. The switch and label anchors must remain stable as the badge changes.
- At the primary seam, verify keyboard focus visibility and reduced-motion behavior. Reduced motion must not remove content, focus feedback or immediate interaction.
- Use screenshot comparisons as supporting evidence for surface, spacing and responsive consistency, but pair them with geometry and accessibility assertions so harmless antialiasing changes do not become the only signal.
- Extend the existing Playwright harness rather than introducing another end-to-end framework. Bridge responses used by the browser test must be deterministic and scoped to test setup; production behavior must remain unchanged.
- Retain and extend the mounted Vue task panel tests as supporting regressions for active-view switching, retained instances, navigation handoff,待复查 badge capping and business events. Happy DOM tests do not substitute for the primary geometry seam because they do not calculate browser layout.
- Retain existing Todo and关注事项 component and domain tests. This work must not weaken coverage of list grouping, lifecycle actions, Todo creation handoff or failure feedback.
- Minimum automated verification is the targeted Playwright task-workspace scenarios, targeted Vitest component tests, workspace type checking, desktop Web build and `git diff --check`.
- Runtime product visual acceptance must additionally inspect the default light theme, common and narrow window sizes, empty/loading/error states, dialogs and overflow. Starting the product UI still requires explicit authorization during implementation, and build or automated test success must not be reported as completed visual acceptance.

## Out of Scope

- Combining Todo and关注事项 into one entity, table, status machine, list or persistence model.
- Changing关注状态, external result,复查时间,待复查 grouping, reminder scheduling or any other关注事项 lifecycle rule.
- Changing Todo completion, overdue, recurrence, project, assignment, calendar or reminder behavior.
- Redesigning Todo cards,关注事项 cards or detail information architecture beyond the adjustments required to fit the shared workspace geometry and baseline visual tokens.
- Adding persistent links or automatic synchronization between Todo and关注事项.
- Adding new filters, search capabilities, bulk actions, dashboards, analytics or workflow states.
- Replacing the current task view switch with a new navigation hierarchy or moving the two views into separate tool tabs.
- Adding decorative page transitions, complex motion, shared-element animation or layout animation.
- Introducing a new color palette, font family, icon library or broad application design-system rewrite.
- Refactoring unrelated tools, global layout containers or the accepted tool-tab cache architecture.
- Adding database migrations, IPC commands, background tasks, network dependencies or online services.
- Packaging, publishing or releasing the desktop application as part of this work.

## Further Notes

ADR 0006 remains authoritative: Todo represents work the user executes, while关注事项 represents externally owned matters the user follows. This specification aligns their interface skeleton without weakening that domain boundary.

The UI research selected a dense productivity-tool treatment with low motion. Only the applicable layout-stability, semantic-token, reduced-motion and accessibility guidance is adopted; generated palette, typography and marketing-page recommendations do not override the existing LazyCat design system.

The central acceptance criterion is spatial continuity: at the same available workspace width, switching views must preserve the visible task-workspace skeleton while changing only the domain-specific controls and content inside it.
