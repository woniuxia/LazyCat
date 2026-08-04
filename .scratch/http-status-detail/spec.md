Type: task
Labels: ready-for-agent

# HTTP 状态码详情与检索优化

## Problem Statement

HTTP 状态码工具目前只能在表格中快速浏览码值、英文名、中文说明、用途和常见原因。用户遇到具体状态码时，需要在多行文本中手动比对，无法在当前列表上下文中快速查看语义、排查建议和相关响应头；搜索也只覆盖少量基础字段，未知三位数会直接落入空结果，难以判断它所属的错误范围。

## Solution

把 HTTP 状态码工具升级为可展开的实战速查表。用户点击任意状态码条目后，可在原行位置展开详情，同时保留当前列表或搜索结果；多个条目可以同时展开。详情提供语义解释、常见场景、触发原因、排查建议和适用的响应头提示。搜索覆盖全部详情字段并按相关性稳定排序。对于未收录的三位数，显示独立的通用分类提示，说明其所属 1xx–5xx 段位但不猜测具体含义。

## User Stories

1. As a developer, I want to scan HTTP status codes grouped by 1xx–5xx, so that I can locate the relevant response family quickly.
2. As a developer, I want to click a status code row and expand its details in place, so that I can keep the surrounding codes visible for comparison.
3. As a developer, I want to expand multiple status code rows at the same time, so that I can compare related responses without repeatedly reopening them.
4. As a developer, I want clicking an expanded row to collapse only that row, so that other open details remain available.
5. As a developer, I want each detail view to explain the status code's semantics, so that I understand what the server response means rather than memorizing a label.
6. As a developer, I want to see common usage scenarios, so that I can distinguish normal protocol behavior from an application failure.
7. As a developer, I want to see common trigger causes, so that I can form a first troubleshooting hypothesis.
8. As a developer, I want actionable troubleshooting suggestions, so that I know what to inspect next.
9. As a developer, I want related response-header guidance when it applies, so that I can inspect the right headers during diagnosis.
10. As a developer, I do not want empty detail sections, so that irrelevant fields do not add visual noise.
11. As a developer, I want to search by a code, English name, Chinese meaning, usage, cause, troubleshooting suggestion, or header hint, so that I can start from the symptom or term I already have.
12. As a developer, I want exact matches to appear before weaker matches, so that the most likely result is immediately visible.
13. As a developer, I want search results with equal relevance to be ordered by status code, so that repeated searches produce stable results.
14. As a developer, I want common standard status codes beyond the original small list to be available, so that the tool covers routine modern HTTP troubleshooting.
15. As a developer, I want standard codes and vendor-specific codes kept separate, so that I do not mistake a proxy or vendor extension for an HTTP standard.
16. As a developer, I want an unknown three-digit code in a valid 1xx–5xx range to show its generic response family, so that I can orient myself without receiving invented semantics.
17. As a developer, I want the unknown-code message to state that the specific meaning is undefined, so that I understand the limits of the lookup.
18. As a developer, I want an unknown code to remain a separate classification hint rather than a fake status-code row, so that only verified entries can be expanded.
19. As a developer, I want the tool to show a clear no-match state for input that is not a known code or meaningful search term, so that an empty result is understandable.
20. As a developer, I want backend lookup failures to surface as errors, so that stale or incomplete results are not mistaken for a successful search.
21. As a developer, I want initial list-loading failures to surface clearly, so that I know the reference data did not load.
22. As a developer, I want to switch between grouped browsing and search without losing an already expanded entry when that state can be retained simply, so that returning to a previous result does not require extra clicks.
23. As a developer, I want the expansion state to be associated with the status code, so that a filter change does not accidentally attach one code's details to another row.
24. As a developer, I want the panel to remain usable in a narrow window, so that long troubleshooting text and multiple open details do not overlap or become inaccessible.
25. As a developer, I want the existing HTTP status-code tool entry and IPC channels to remain available, so that this optimization does not break the rest of LazyCat.

## Implementation Decisions

- Keep the current Rust network action boundary as the source of truth for HTTP status-code data. The front end remains responsible for loading, rendering, filtering state, and expansion state; it does not duplicate the reference dataset.
- Extend the status-code record with the confirmed practical-detail concepts: semantic explanation, common scenarios, common causes, troubleshooting suggestions, and applicable response-header hints. Existing display fields remain available for compatibility.
- Return the expanded record through both list and lookup responses so a row can render its detail without a second per-row request.
- Keep list responses grouped into the existing five categories. Add common standard codes, including the previously identified gaps such as 205, 418, 425, 428, 431, 451, 507, and 511, while retaining existing standard entries. Do not add vendor/proxy extension codes such as 499 or 520–526.
- Treat a status-code row as the unit of expansion. Expansion is toggled by status code, multiple rows may remain open, and closing one row leaves other rows unchanged.
- Render the detail inline in the table context. The grouped list and search result list use the same detail behavior; an unknown-code classification hint is not an expandable row.
- Keep expansion state keyed by numeric status code when that can be implemented without introducing complex synchronization. If filtering/reloading makes retention unreliable or materially expands the state model, clear the affected state rather than risk stale detail attachment.
- Extend lookup matching to all user-visible detail fields. Normalize the query for case-insensitive matching where applicable and preserve Chinese text matching.
- Rank lookup results deterministically: exact code or exact name first, then name/Chinese-meaning prefix matches, then contains matches in usage, causes, troubleshooting, or header hints; break ties by ascending numeric code.
- For a three-digit query that is not in the standard dataset but belongs to 100–599, return a separate generic classification hint identifying its 1xx–5xx family and explicitly stating that the specific meaning is undefined. Do not fabricate a status record, semantics, causes, or troubleshooting advice.
- For queries outside the supported three-digit range or with no meaningful match, return the existing empty-results shape and let the panel show its normal no-match state.
- Do not add status-code favorites, history, or a new persistence model in this iteration. Existing tool-level favorites are unaffected.
- Preserve explicit error propagation from the IPC layer. Loading and lookup failures must remain distinguishable from an empty result and visible to the user.

## Testing Decisions

- Tests should assert observable data and user behavior, not private implementation details, CSS selectors that are not part of the contract, or the exact storage mechanism used for expansion state.
- At the Rust action seam, cover: five category groups; presence of the newly included common standard codes; detail fields on representative 2xx, 3xx, 4xx, and 5xx entries; matching through each detail-field class; relevance ordering; stable numeric tie-breaking; unknown-code classification hints; unsupported/out-of-range empty results; and explicit malformed/error behavior where applicable.
- At the Vue panel seam, mock the existing tool-invocation bridge and cover: initial list rendering; expanding and collapsing one row; keeping multiple rows expanded; rendering conditional response-header sections; switching between grouped and search views; retaining expansion by code when straightforward; rendering the unknown-code classification hint without an expandable row; empty results; and visible IPC failure feedback.
- Reuse the repository's existing Vitest conventions for Rust-adjacent action contract tests and Vue/bridge-mocked component behavior tests. Do not introduce a new end-to-end harness solely for this reference panel.
- The minimum verification sequence is targeted HTTP status tests, then desktop type checking, then the desktop web build. A full product UI launch is not required by this spec.

## Out of Scope

- Vendor, proxy, CDN, or framework-specific non-standard status codes.
- Fetching status-code definitions from the network or depending on external documentation at runtime.
- Status-code favorites, recent-status history, or cross-device synchronization.
- Automatic HTTP requests, live endpoint probing, response capture, or integration with the separate HTTP connectivity test.
- Editing the reference data from the UI.
- A separate detail page, modal workflow, or right-side drawer.
- Automatically inferring a concrete meaning, cause, or remediation for an unknown status code.
- Redesigning the surrounding network-tool navigation or changing the existing tool identifier and IPC channel names.

## Further Notes

The project glossary in `CONTEXT.md` defines the canonical terms “状态码条目”, “详情展开”, “实战型详情”, “多重详情展开”, “常见标准状态码”, “未知状态码”, and “通用分类提示”. The implementation should use those concepts consistently in code-facing documentation and user-visible copy.

The feature remains offline-first: all reference data is bundled with the application. The spec intentionally leaves the exact prose for each status code to the implementation pass, but every included standard entry must have enough content to satisfy the detail-field and search contracts above.
