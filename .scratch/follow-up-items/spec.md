Type: task
Labels: ready-for-agent

# 关注事项

## Problem Statement

用户目前可以用任务清单管理自己需要执行的任务，但缺少一种明确方式记录“由他人负责推进、自己只需要定期确认进展和结果”的事情。把这类事情直接放入 Todo 会混淆责任主体：Todo 的完成、逾期、周期生成和提醒清理都表达用户执行任务，而关注事项关心的是外部进展、下一次复查以及用户是否继续关注。

用户需要一个个人、离线的外部承诺跟踪闭环：记录责任人和预期结果，在约定时间提醒自己确认，保留历次反馈，在尚未完成时安排下一次复查，并在确认结果或不再需要跟踪时结束关注。

## Solution

在现有“任务清单”工具中增加独立的“关注事项”视图，与“我的任务”并列展示。关注事项采用独立实体、持久化和生命周期，复用现有人员名录、提醒基础设施、页面骨架和优先级词汇，但不复用 Todo 状态机。

用户创建关注事项时填写标题、单一责任人和复查时间，并可补充预期结果、预计完成时间、优先级、描述和相关链接。到达复查时间后，事项进入“待复查”；用户可以记录进展并继续关注、确认已完成、确认已取消，或在结果未知时结束关注。继续关注必须设置新的复查时间，所有进展和结束动作必须留下内容，历次记录形成时间线。已结束事项可以重新关注，原时间线继续保留。

提醒沿用 LazyCat 当前的进程内调度边界。应用或托盘进程运行时正常提醒；完全退出期间不运行调度，下次启动补发。多个同时待复查的事项合并为一条只负责导航的聚合复查提醒，单个事项可以稍后提醒，但稍后提醒不改变正式复查时间。

## User Stories

1. As a LazyCat user, I want to distinguish things I execute from things I only follow, so that responsibility remains clear.
2. As a LazyCat user, I want关注事项 to live beside my tasks in the same tool, so that related personal planning workflows remain easy to find.
3. As a LazyCat user, I want “我的任务” and “关注事项” to be separate views, so that their different lifecycle semantics do not mix in one list.
4. As a LazyCat user, I want to create a关注事项 with a title, so that I can identify the external matter quickly.
5. As a LazyCat user, I want every关注事项 to have one责任人, so that I know who is expected to advance it.
6. As a LazyCat user, I want to select the责任人 from the existing人员名录, so that I do not maintain duplicate person records.
7. As a LazyCat user, I want to add a person while creating or editing a关注事项, so that a missing name does not interrupt capture.
8. As a LazyCat user, I want every active关注事项 to have a复查时间, so that it cannot silently become a forgotten note.
9. As a LazyCat user, I want to enter a复查时间 in the past, so that I can backfill something that already needs attention.
10. As a LazyCat user, I want a backfilled past复查时间 to place the item in待复查 immediately, so that the application does not conceal its urgency.
11. As a LazyCat user, I want quick复查时间 choices for tomorrow, three days later and one week later, so that common follow-up intervals require fewer inputs.
12. As a LazyCat user, I want a custom复查时间, so that unusual commitments can be represented precisely.
13. As a LazyCat user, I want an optional expected outcome, so that I can remember what result I intend to confirm.
14. As a LazyCat user, I want an optional预计完成时间, so that the责任人的 commitment remains separate from my own复查时间.
15. As a LazyCat user, I want an external-deadline indication when the预计完成时间 arrives without a confirmed result, so that I can see that the external commitment is due.
16. As a LazyCat user, I want an optional priority, so that important关注事项 stand out without becoming Todo tasks.
17. As a LazyCat user, I want an optional description, so that I can preserve the context needed for the next conversation.
18. As a LazyCat user, I want to attach related links, so that evidence and external work items remain reachable.
19. As a LazyCat user, I want newly created active items grouped by复查时间, so that the next required attention is obvious.
20. As a LazyCat user, I want items whose复查时间 has arrived grouped under待复查, so that I know what to confirm now.
21. As a LazyCat user, I want future items within seven days grouped under近期复查, so that near-term attention is easy to scan.
22. As a LazyCat user, I want later items grouped under以后复查, so that distant items do not crowd immediate work.
23. As a LazyCat user, I want ended items kept under已结束, so that I can review prior outcomes without mixing them with active attention.
24. As a LazyCat user, I want active items sorted by复查时间 ascending, so that the nearest review appears first.
25. As a LazyCat user, I want a list item to show title,责任人,复查时间 and the latest progress summary, so that I can scan without opening every detail.
26. As a LazyCat user, I want priority, external-deadline state and links shown as secondary information, so that the main list remains compact.
27. As a LazyCat user, I want to open a detail view with the full description, expected outcome and progress timeline, so that all context is available in one place.
28. As a LazyCat user, I want to record what the责任人 told me, so that the latest external progress is explicit.
29. As a LazyCat user, I want each进展记录 to retain its time, so that I can reconstruct what was known when.
30. As a LazyCat user, I want new进展记录 appended rather than overwriting history, so that repeated delays and changed commitments remain visible.
31. As a LazyCat user, I want to choose继续关注 after an incomplete result, so that the same关注事项 remains active.
32. As a LazyCat user, I want继续关注 to require a new复查时间, so that every active item always has a next action.
33. As a LazyCat user, I want继续关注 to require progress content, so that rescheduling cannot create an empty history entry.
34. As a LazyCat user, I want to confirm that the external matter is completed, so that the known result and ended attention are recorded together.
35. As a LazyCat user, I want to confirm that the external matter is canceled, so that cancellation is not presented as successful completion.
36. As a LazyCat user, I want result confirmation to require content, so that the basis of the final result remains visible.
37. As a LazyCat user, I want to结束关注 while the external result remains unknown, so that my decision to stop tracking does not invent an external outcome.
38. As a LazyCat user, I want结束关注 to require a reason, so that future me understands why tracking stopped.
39. As a LazyCat user, I want concern state and external result stored separately, so that “not following” and “completed” never become synonyms.
40. As a LazyCat user, I want all result and attention transitions visible in the timeline, so that the history explains the current state.
41. As a LazyCat user, I want to重新关注 an ended item, so that an incorrectly ended or restarted external matter can resume without losing history.
42. As a LazyCat user, I want重新关注 to require a new复查时间, so that the restored item immediately satisfies the active-item invariant.
43. As a LazyCat user, I want重新关注 to preserve prior progress and append a transition record, so that the earlier outcome is not silently erased from history.
44. As a LazyCat user, I want the current external result to return to unknown after重新关注, so that a previous result is not presented as the current truth.
45. As a LazyCat user, I want to edit mistaken progress content, so that typographical and factual errors can be corrected.
46. As a LazyCat user, I want to delete an ordinary进展记录 after confirmation, so that accidental entries can be removed in this personal tool.
47. As a LazyCat user, I want editing or deleting ordinary progress to leave关注状态 unchanged, so that content maintenance cannot silently finish or restore an item.
48. As a LazyCat user, I want transition events retained as state history, so that deleting ordinary notes cannot make the lifecycle inexplicable.
49. As a LazyCat user, I want to edit the core fields of a关注事项, so that responsibility, context and dates can change as the situation evolves.
50. As a LazyCat user, I want changing复查时间 to reset stale reminder state, so that the new schedule can notify exactly once.
51. As a LazyCat user, I want deleting a person from人员名录 to preserve the责任人 name on existing关注事项, so that current and historical records remain intelligible.
52. As a LazyCat user, I want to reassign an item whose person record was deleted, so that active tracking can continue with a current责任人.
53. As a LazyCat user, I want to search title,责任人, description and progress content, so that I can find an item from any remembered context.
54. As a LazyCat user, I want to filter by责任人, priority and关注状态, so that I can focus the list without introducing a separate date-query system.
55. As a LazyCat user, I want a visible empty state for each list condition, so that an empty result is not mistaken for a loading failure.
56. As a LazyCat user, I want a reminder when复查时间 arrives while LazyCat is running, so that I remember to ask for the result.
57. As a LazyCat user, I want reminders to continue while the main window is hidden in the tray, so that hiding the application does not disable tracking.
58. As a LazyCat user, I want missed reminders detected when LazyCat next starts, so that fully exiting the application does not permanently lose due attention.
59. As a LazyCat user, I want multiple due items combined into one聚合复查提醒, so that reopening after an absence does not flood the desktop.
60. As a LazyCat user, I want a聚合复查提醒 to open the待复查 list, so that I can process each item in context.
61. As a LazyCat user, I do not want a聚合复查提醒 to postpone all items at once, so that unrelated review schedules are not changed accidentally.
62. As a LazyCat user, I want to稍后提醒 an individual item, so that I can defer the notification without falsifying its formal复查时间.
63. As a LazyCat user, I want an individual reminder to provide “查看” and “稍后提醒”, so that result transitions require deliberate detail entry.
64. As a LazyCat user, I want a due item to remain待复查 after稍后提醒, so that temporary notification suppression does not hide its business state.
65. As a LazyCat user, I want notification errors and persistence errors shown explicitly, so that the application never pretends a progress action succeeded.
66. As a LazyCat user, I want to open a prefilled Todo draft from a关注事项, so that work I must personally perform can be captured efficiently.
67. As a LazyCat user, I want to review and confirm that Todo draft before creation, so that copying context cannot create an accidental task.
68. As a LazyCat user, I do not want the generated Todo and关注事项 persistently linked in the first version, so that the two lifecycles cannot drift into a false synchronized state.
69. As a LazyCat user, I want creating a Todo draft to leave the关注事项 active, so that external tracking does not stop unless I explicitly end it.
70. As a LazyCat user, I want to delete an entire关注事项 explicitly, so that unwanted records and their progress history can be removed.
71. As a LazyCat user, I want deletion to require confirmation, so that an active timeline is not removed accidentally.
72. As a LazyCat user, I want deleting an item to remove its progress and links atomically, so that orphaned records do not remain.
73. As a LazyCat user, I want ended items retained indefinitely by default, so that the application never removes history through a hidden retention policy.
74. As a LazyCat user, I want my关注事项 and timeline restored after application restart, so that the feature remains a durable personal tracker.
75. As a LazyCat user, I want the entire core workflow to remain offline, so that tracking does not depend on remote accounts or collaboration services.
76. As an existing Todo user, I want Todo completion, overdue, recurrence and reminders to keep their current behavior, so that adding关注事项 does not regress my tasks.
77. As an existing Todo user, I want existing人员名录 entries to remain usable without a data migration, so that the new feature does not disrupt current assignments.

## Implementation Decisions

- Follow ADR 0006:关注事项 is an independent domain entity with independent persistence and lifecycle. It is not implemented as a Todo category, assignee convention, kind discriminator or extension of `pending / in_progress / completed`.
- Keep the existing “任务清单” tool entry. Add a stable top-level segmented view for “我的任务” and “关注事项”; the existing Todo view remains the default and retains its current behavior.
- Add a focused关注事项 backend domain module, typed renderer model and bridge commands. Reuse existing project patterns for command dispatch, explicit errors, SQLite connection management and frontend composables without folding the new behavior into the Todo module.
- Persist关注事项 in an item table containing: identity; title; optional description and expected outcome; P0-P3 priority;关注状态; external result; ending mode; mandatory责任人 identity snapshot; optional current人员名录 ID; mandatory复查时间 while active; optional预计完成时间; notification snooze and last-notified state; and created, updated and ended timestamps.
- Persist the progress timeline in a child table containing item identity, entry kind, required user content where applicable, occurrence time and update time. Entry kinds distinguish ordinary progress from continue, completed, canceled, stopped-following and reopened transitions.
- Persist related links in a child table. Links belong to the关注事项 lifecycle and are removed with the parent item.
- Use database constraints where practical and domain validation for the full invariant: an active item must have a non-empty title, a责任人 display identity and a复查时间; an ended item has no scheduled reminder; completed and canceled results require a result-confirmation ending mode; stopped-following retains an unknown external result.
- Store both the selected人员名录 ID and a责任人 name snapshot. When the referenced person still exists, current directory data may supply its latest display name; when the directory entry is deleted, the snapshot remains the fallback. Do not rename or migrate the existing physical人员名录 table in this iteration.
- Preserve existing Todo person-deletion behavior.关注事项 must not lose its displayable责任人 when a directory association disappears, and editing may select a new person and refresh the snapshot.
- Keep关注状态 and external result independent.关注状态 has `active` and `ended`; external result has `unknown`, `completed` and `canceled`; ending mode distinguishes confirmed result from stopped following.
- Expose explicit domain actions rather than a generic status setter: create, update core fields, continue following, confirm completed, confirm canceled, stop following, reopen, edit ordinary progress, delete ordinary progress, snooze one item and delete the item.
- `继续关注` requires non-empty content and a new复查时间. In one SQLite transaction it appends the timeline record, updates the item schedule, clears prior snooze state and resets notification de-duplication for the new review cycle.
- Confirming completed or canceled requires non-empty content. In one transaction it appends the result record, sets the known external result, ends attention, clears复查时间 and all pending reminder state, and records the ended timestamp.
- `结束关注` requires non-empty content. In one transaction it appends the reason, ends attention while retaining an unknown external result, clears复查时间 and reminder state, and records the ended timestamp.
- `重新关注` requires a new复查时间. In one transaction it appends a visible reopen transition, sets关注状态 to active, restores the current external result to unknown, clears the previous ending mode and ended timestamp, and initializes a fresh notification cycle without deleting earlier timeline entries.
- Allow user-authored timeline content to be corrected. Ordinary progress entries can be deleted after confirmation; editing or deleting ordinary entries never mutates关注状态, external result or schedule. Lifecycle transition records remain visible as state history rather than being deletable through the ordinary-progress action.
- Deleting a关注事项 is an explicit confirmed destructive action. Delete the item, timeline and links in one transaction or by enforced cascading relations; failure must leave the record intact and return contextual error information.
- Treat the persisted复查时间 as the single source of truth for list grouping and business state. A past or current active复查时间 means待复查; it is not called overdue and does not assign lateness to the责任人.
- Define近期复查 as active items whose future复查时间 falls within the next seven calendar days in the user's local time. Define以后复查 as later active items. Sort active groups by复查时间 ascending and ended items by ended time descending.
- Treat预计完成时间 as a separate optional external commitment. When it has arrived and no completed or canceled result is confirmed, show `外部期限已到`; it does not move the item between复查分组 or fire the formal review reminder by itself.
- Provide tomorrow, three-days-later and one-week-later shortcuts plus a custom local date/time control. Every shortcut resolves to an explicit persisted复查时间; there is no recurrence rule or generated instance.
- Build a compact list/detail experience consistent with the existing task tool. The list shows title,责任人,复查时间 and latest progress summary, with priority, external-deadline state and links as secondary metadata. The detail view owns editing, timeline operations and lifecycle actions.
- Search title,责任人 snapshot/current name, description and progress content. Filter by责任人, P0-P3 priority and关注状态. Date urgency is represented by the fixed list groups rather than an additional date filter.
- Reuse the current进程内 scheduler lifecycle. While the application process is running, scan due active关注事项 on the existing scheduler cadence. Fully exiting the process does not create a new Windows service or scheduled task; overdue notifications are discovered on the next application start.
- Keep `复查时间`, temporary snooze and notification de-duplication separate. Snoozing one item changes only the effective next notification time. The item stays待复查, and any new rolling复查时间 clears the stale snooze and last-notified values.
- Dispatch one notification for a single newly due item and one聚合复查提醒 for a batch. A single-item notification offers查看 and稍后提醒. A聚合复查提醒 reports the count and opens the待复查 list; it never snoozes the batch.
- Mark a due review cycle as notified in the same transaction that claims it for dispatch, so repeated scheduler ticks cannot emit duplicates. A dispatch failure remains diagnosable and must not manufacture a successful user action or alter关注状态.
- Add a global notification payload and navigation route for关注事项 rather than disguising the notification as a Todo reminder. Opening it activates the existing task tool, selects “关注事项” and, for an aggregate, selects待复查.
- Implement “创建任务” as a transient navigation handoff within the task tool. Prefill a new Todo draft with the关注事项 title and relevant context, switch to “我的任务”, and require the user to confirm creation. Do not persist an association in either domain and do not end关注 automatically.
- Keep all writes local in the existing SQLite database. The feature has no remote account, collaboration protocol, cloud synchronization or network requirement; links are stored as references and are not fetched during the core workflow.
- Preserve errors from database, validation, bridge and notification operations with the failed action and target context. The UI must keep the user's form content after a failed write and must not display success until the domain command commits.
- Keep timestamp parsing and list grouping consistent with the repository's local-time rules. Do not use metadata timestamps as substitutes for复查时间,预计完成时间 or ended time.

## Testing Decisions

- Tests assert observable domain results and user workflows, not private helper names, table implementation details, incidental DOM nesting or source-code substrings.
- Use two primary seams because the feature crosses native persistence and renderer behavior: the Rust关注事项 domain command seam with a temporary SQLite database, and a mounted Vue关注事项 panel seam with mocked Tauri commands. Avoid adding a lower-level public testing interface.
- At the Rust domain command seam, cover creation validation; required title,责任人 and复查时间; past复查时间; item update; detail retrieval; deterministic list grouping inputs; search and filters; links; and responsibility snapshot fallback after人员名录 deletion.
- At the Rust domain command seam, cover all lifecycle transitions and rejected transitions:继续关注, completed, canceled, stopped following and reopened. Assert the resulting关注状态, external result, ending mode,复查时间, ended time and visible timeline.
- At the Rust domain command seam, force failures within multi-write actions and verify atomic rollback. No test should observe a new progress record without the matching schedule or state update, or a state transition without its required timeline record.
- At the Rust domain command seam, cover editing and deleting ordinary progress, protection of lifecycle transition history, cascade deletion, and explicit errors for missing or stale identities.
- At the Rust scheduler seam, use a controlled clock to cover not-yet-due, newly due, already-notified, snoozed and rescheduled items; one-item dispatch; multi-item aggregation; startup discovery of missed items; no batch snooze; and reminder-state clearing on ending or rolling复查.
- At the global notification contract seam, cover serialization and routing for individual and aggregate关注事项 notifications, including item identity or due count and the absence of Todo-only actions.
- At the mounted Vue panel seam, mock bridge responses and cover initial loading, view switching, the four list groups, stable ordering, latest-progress display, external-deadline indication, empty states, search, filters and visible failures.
- At the mounted Vue panel seam, cover create and edit forms; required validation; quick and custom复查时间 choices; related links;人员名录 selection and quick add; and preservation of unsaved input after rejected writes.
- At the mounted Vue panel seam, cover complete user paths for继续关注, completed, canceled, stopped following and reopened. Assert required content, required next复查时间, visible timeline updates, group movement and failure behavior.
- At the mounted Vue panel seam, cover ordinary progress editing/deletion, item deletion confirmation, responsibility snapshot display, individual snooze, aggregate-notification navigation and the prefilled Todo draft handoff without a persistent association.
- Add small pure-function tests only where local-time grouping, external-deadline derivation or stable sorting would be substantially harder to prove through the panel. These are supporting tests, not a third primary seam.
- Reuse the repository's existing temporary-SQLite Rust tests, controlled-time reminder tests, mounted Vue renderer harness, bridge mocks and Vitest fake timers. Do not add a new end-to-end framework.
- Minimum automated verification is targeted Rust关注事项 and notification tests, targeted Vue panel and supporting utility tests, workspace type checking, desktop web build and `git diff --check`.
- Runtime acceptance must exercise create, edit, continue, result confirmation, stop, reopen, delete, application restart persistence, tray reminder, missed-reminder startup aggregation and Todo draft handoff. Unit tests, type checking and build success do not replace these user-path checks.
- Because the feature changes a dense task-management UI, runtime visual acceptance must inspect the default light theme, empty/loading/error states, dialogs, active and ended details, narrow windows, long titles and progress content, overflow, scrolling and stable layout while changing filters and lifecycle state. Product UI launch still requires explicit authorization during implementation.

## Out of Scope

- Implementing关注事项 as a Todo kind, category, special assignee rule or extension of the Todo completion state.
- Mixing关注事项 into the existing Todo active, recently completed or completed buckets.
- Todo recurrence, generated follow-up instances, cron expressions or automatic repeating rules;滚动复查 always updates the same item.
- Treating复查时间 as a责任人 deadline or labeling待复查 as Todo overdue.
- Automatically deriving复查时间 from预计完成时间 or firing the formal review reminder solely because an external deadline arrived.
- Multiple责任人 on one关注事项.
- A separate关注事项 category system or reuse of Todo categories in the first version.
- Progress percentages, project workflow states, kanban boards or collaboration assignment.
- File and image attachments on items or progress records.
- Persistent one-way or two-way associations between关注事项 and Todo, automatic synchronization, or automatic关注结束 after creating a Todo draft.
- Spotlight results, desktop widget content, PM embedded lists, calendar entries or other secondary surfaces.
- Renaming or migrating the existing人员名录 table, introducing a general contact system, or storing phone, email and messaging-account data.
- A Windows service, scheduled task or other background execution mechanism that survives full application exit.
- Batch稍后提醒 from a聚合复查提醒.
- Automatic archival, retention limits or cleanup of ended items.
- Remote collaboration, external-system polling, automatic progress ingestion, cloud synchronization or online authentication.
- Import/export, bulk create, bulk lifecycle actions, analytics or progress dashboards.
- Redesigning the existing Todo data model, status normalization, recurrence, action binding, reminder semantics or task tool navigation outside the added top-level view.
- Automatically starting the product UI, packaging installers or publishing a release as part of implementation.

## Further Notes

The project glossary defines the canonical language for this feature: “关注事项”, “外部进展”, “关注状态”, “进展记录”, “复查时间”, “责任人”, “人员名录”, “确认结果”, “结束关注”, “待复查”, “滚动复查”, “预计完成时间”, “外部期限已到”, “继续关注”, “稍后提醒”, “重新关注” and “聚合复查提醒”. UI copy, domain APIs and implementation discussion should preserve these distinctions.

ADR 0006 records the central architecture decision:关注事项 owns an independent entity and lifecycle because Todo completion, overdue, recurrence and reminder cleanup express execution responsibility. Shared人员名录, notification infrastructure and UI patterns are reuse opportunities, not permission to share the Todo state machine.

The first version is deliberately a personal offline follow-up loop rather than a lightweight project-management or collaboration system. Its success criterion is that a user can reliably answer: who owns the external matter, what was last reported, when should I ask again, what result was confirmed, and am I still following it?
