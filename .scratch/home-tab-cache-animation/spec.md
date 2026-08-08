Type: task
Labels: ready-for-agent

# 首页缓存恢复即时可用

## Problem Statement

首页已经纳入统一标签页缓存。首页工具卡片带有首次呈现的交错入场动效；当用户从其他工具标签页返回首页时，缓存页面的恢复会受到该动效影响，部分内容在一段时间内不可见，用户会误以为首页尚未准备好。若用户在首页首次呈现动效完成前切走，返回时还可能继续等待未完成的动效。

## Solution

将首页入场动效限定为标签页首次呈现阶段。首页页面上下文首次创建时保留现有卡片动效和节奏；从失活状态恢复、首次动效被打断后恢复、以及首页内容发生变化时，已有和新出现的内容都立即可见并可交互。系统要求减少动态效果时，首页首次呈现也直接显示内容。

所有返回首页的入口遵循相同规则。卡片悬停、焦点、收藏和拖拽等交互动效继续保留。

## User Stories

1. As a developer, I want the home page to become immediately usable when I return to its cached tab, so that I can open another tool without waiting for decorative motion.
2. As a developer, I want the home page to retain its existing entrance animation on first presentation, so that the current visual character is preserved for a newly created page context.
3. As a developer, I want a cached home page to restore its visible content immediately after tab deactivation, so that page cache means continuity rather than a delayed re-entry.
4. As a developer, I want a home page whose first entrance animation was interrupted to complete visibly when I return, so that unfinished motion cannot hide usable content.
5. As a developer, I want existing home cards to remain immediately visible after favorite changes, so that changing home organization does not make the page temporarily blank.
6. As a developer, I want newly appearing home cards to be immediately visible after cached content changes, so that updated tool availability is usable as soon as it appears.
7. As a developer, I want home content changes while the page is inactive to be ready when I reactivate the page, so that inactive-page updates do not introduce a second entrance delay.
8. As a developer, I want home content changes while the page is active to render without a whole-page entrance sequence, so that normal updates feel instantaneous.
9. As a developer, I want the same restoration behavior when I return through the top brand control, sidebar, tab bar, or shortcut navigation, so that navigation entry points do not create inconsistent timing.
10. As a developer, I want the home page to obey the system preference for reduced motion, so that users who avoid animation see usable content immediately on first presentation.
11. As a developer, I want reduced-motion behavior to affect only the decorative entrance animation, so that card content, layout, and interactions remain available.
12. As a developer, I want hover feedback to remain available after this change, so that card affordances are still understandable.
13. As a developer, I want focus feedback to remain available after this change, so that keyboard navigation continues to communicate the active card.
14. As a developer, I want favorite and drag interactions to retain their current feedback, so that reorganizing common tools does not lose its interaction cues.
15. As a developer, I want the existing page cache to preserve the home page instance and its local context, so that the animation fix does not trade away input, scroll, or local state retention.
16. As a developer, I want closing and reopening a page context to remain distinct from activating a cached page, so that a genuinely new context may receive the first-presentation treatment again.
17. As a developer, I want the home page to remain the same page context across all supported return paths, so that the behavior does not depend on which control initiated navigation.
18. As a developer, I want existing tool pages to keep their current lifecycle behavior, so that fixing home presentation does not alter unrelated page activation or deactivation semantics.
19. As a developer, I want the home page to remain available without network resources, so that the behavior preserves the application's offline core path.
20. As a developer, I want the fix to avoid storing animation state in durable settings, so that a visual presentation concern does not expand the persistence model.
21. As a developer, I want the home page to show an empty state immediately when there is no available content, so that the absence of tools is not confused with an animation delay.
22. As a developer, I want the current home card content and ordering to remain unchanged, so that this work addresses timing without changing the home page's information architecture.
23. As a developer, I want the current entrance animation duration and stagger rhythm to remain unchanged for first presentation, so that the scope does not silently become a visual redesign.

## Implementation Decisions

- Keep the existing independent page-cache host and `KeepAlive` boundary. The change is a page-presentation behavior layered on top of the accepted tab-cache architecture; it does not replace the cache boundary or introduce a second cache mechanism.
- Treat first presentation as a distinct page-context state. The entrance effect may run only while a newly created home page context is being presented for the first time.
- Treat any subsequent activation of the same cached home page context as immediate restoration. Restoration must not replay, resume, or wait for an unfinished entrance effect.
- Once the home page has left its first-presentation phase, changes to favorite tools, visible tools, ordering, or other home content must not trigger a whole-page entrance effect. Existing and newly rendered cards must be visible and interactive immediately.
- Apply the same behavior regardless of which navigation entry point activates the home tab. Navigation continues to use the existing active-tab contract.
- Respect the system reduced-motion preference by skipping the decorative first-presentation effect while rendering the same content and interactions.
- Preserve existing hover, focus, favorite, drag, layout, card content, ordering, and animation timing behavior outside the first-presentation visibility rule.
- Keep the behavior renderer-local and ephemeral. Do not add settings, database fields, IPC commands, background work, or application-level task state.
- Keep error behavior explicit. If home content loading or derivation already reports an error, the presentation change must not turn that error into a blank or false-success state.
- Use the existing glossary terms `标签页首次呈现` and `首页内容即时可用` in implementation comments, tests, and follow-up documentation where the concepts need naming.

## Testing Decisions

- Tests must assert observable page behavior: content visibility/readiness and preservation of the cached page context across activation. The approved happy-dom seam cannot calculate imported CSS animation styles, so its narrow fallback may assert the stable rendered entry-state marker that controls the user-visible animation; it must not assert private state names or incidental helper arrangement. Product UI runtime verification remains separate.
- Use the existing `TabPageCache` component behavior test seam as the single primary seam. Mount the cache host with minimal home content, exercise reactive active-tab changes, and observe the content through the same cached page lifecycle used by the application.
- Cover first presentation separately from cached restoration. First presentation may expose the existing entrance treatment; cached restoration must expose the complete content immediately and must not replay or resume an unfinished entrance treatment.
- Cover rapid switching away before first presentation finishes, then returning to the home tab. The content must be complete immediately after restoration.
- Cover home content changes while inactive and while active. Existing and newly rendered content must be immediately available without a whole-page entrance delay.
- Cover the reduced-motion preference at first presentation. The same content and interactions must be available without the decorative entrance effect.
- Cover all navigation entry points through the shared active-tab contract rather than duplicating one test for each control. The component seam is sufficient because the controls already converge on the same selection behavior.
- Retain existing cache assertions for page identity, local state, scroll continuity, activation/deactivation, and precise destruction when a tab is removed. The new behavior must not weaken the accepted cache contract.
- Reuse the repository's existing Vitest and DOM-based Vue test conventions. No new IPC, persistence, or end-to-end harness is warranted for this renderer-local presentation rule.
- Minimum implementation verification is the targeted cache test, followed by applicable desktop type checking and the desktop web build. Product UI visual verification remains a separate runtime check and must not be claimed from unit tests alone.

## Out of Scope

- Removing the home page entrance animation entirely.
- Changing the entrance animation's duration, easing, stagger rhythm, card layout, card content, tool ordering, or visual direction.
- Removing or changing hover, focus, favorite, drag, or other interaction feedback.
- Changing the independent page-cache host, `KeepAlive` architecture, tab identity rules, scroll ownership, or page lifecycle contract.
- Persisting first-presentation state or home page animation state across application restarts.
- Adding a user-facing animation setting beyond honoring the existing system reduced-motion preference.
- Adding network, IPC, database, background task, or recovery behavior.
- Redesigning the home page empty state or tool catalog.
- Changing animation behavior in unrelated tools or panels.

## Further Notes

The glossary records `标签页首次呈现` and `首页内容即时可用` as the canonical terms for this behavior. The accepted architecture decision for independent page-cache hosts remains authoritative.

This is a reversible renderer presentation rule rather than a new architectural boundary, so no additional ADR is required at this stage.
