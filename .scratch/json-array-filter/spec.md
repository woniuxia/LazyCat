Type: task
Labels: ready-for-agent

# JSON 工作台数组过滤

## Problem Statement

JSON 工作台目前可以格式化、压缩、排序和转换 JSON，但不能从一个包含多层结构的 JSON 文档中快速取出数组记录的指定属性。开发者需要手动删除字段，容易误改 JSON 输入文档，也难以重复调整想要查看的字段集合。

## Solution

在 JSON 工作台增加“数组过滤”页签。用户在独立输入区编辑 JSON 后，面板自动解析并按文档深度优先顺序找到第一个可用对象数组，显示其路径和顶层属性候选。用户通过复选框取消不需要的属性，面板即时生成一个只读的格式化 JSON 根数组，只保留选中属性及其原始值，支持复制结果。

## User Stories

1. As a developer, I want a dedicated array-filter tab in the JSON workbench, so that I can filter JSON arrays without disturbing formatting and schema workflows.
2. As a developer, I want an independent JSON input document for the array filter, so that editing it does not overwrite the processing tab's input.
3. As a developer, I want the input to be parsed automatically after I stop typing, so that I can see the filter result without an extra submit step.
4. As a developer, I want parsing to be debounced for about 300ms, so that intermediate keystrokes do not repeatedly scan the document.
5. As a developer, I want empty input to clear derived controls and output without an error, so that a blank workspace is a valid starting state.
6. As a developer, I want invalid JSON to show a readable error while preserving my text, so that I can fix the source without losing work.
7. As a developer, I want invalid or changed input to clear stale paths, selections, and results, so that displayed output can never be mistaken for a projection of the current text.
8. As a developer, I want the first usable array path to be shown after parsing, so that I know exactly which array is being filtered.
9. As a developer, I want a root array to win when the entire input is an array, so that the most direct target is selected predictably.
10. As a developer, I want the scan to follow document depth-first order, so that the selected array corresponds to the first array I encounter in the source structure.
11. As a developer, I want unsupported primitive and mixed arrays skipped, so that the filter controls never target an array that cannot provide object properties.
12. As a developer, I want an empty array to remain a valid target, so that a valid but currently empty collection is not silently ignored.
13. As a developer, I want a clear empty state when no usable object array exists, so that valid JSON with no applicable target is distinguishable from a parse failure.
14. As a developer, I want only one array target in the first version, so that I can use the simple workflow without choosing between many paths.
15. As a developer, I want top-level property candidates gathered from all objects, so that fields appearing only in later records are still available for selection.
16. As a developer, I want property candidates kept in first-seen order, so that the checkbox list follows the source's natural structure.
17. As a developer, I want all available properties selected initially, so that a newly parsed object array immediately produces a complete projection.
18. As a developer, I want to uncheck properties individually, so that the result contains only the fields I need.
19. As a developer, I want nested object and array values kept intact when their top-level property is selected, so that filtering does not flatten or rewrite nested data.
20. As a developer, I want a selected property that is absent from one record to be omitted only from that record, so that missing data is not fabricated as `null` or an empty value.
21. As a developer, I want all properties to be removable, so that an empty selection produces one empty object per input record while preserving array length.
22. As a developer, I want each result object's remaining keys to keep the original key order, so that the projection remains recognizable beside the source.
23. As a developer, I want the filtered array returned as the JSON root value, so that copied output can be used directly as a new array without unrelated wrapper objects.
24. As a developer, I want the result shown as formatted read-only JSON, so that nested values remain inspectable and the output is easy to reuse.
25. As a developer, I want to copy the filtered result, so that I can paste it into another tool or request quickly.
26. As a developer, I want copy failures to be visible, so that the UI does not imply a successful clipboard action when permission is unavailable.
27. As a developer, I want input, selections, and result to survive switching between JSON workbench tabs during the current run, so that I can compare tools without losing the filter setup.
28. As a developer, I do not want a large JSON document written to settings automatically, so that the feature remains transient and offline-friendly.
29. As a developer, I want the original JSON input to remain unchanged, so that experimenting with projections cannot corrupt the source document.
30. As a developer, I want the existing JSON workbench tabs and clipboard routing to remain available, so that adding the filter does not regress current JSON workflows.

## Implementation Decisions

- Add a third “数组过滤” tab to the existing JSON workbench. Keep the existing processing tab as the default tab; the new panel owns its input, derived state, and runtime persistence independently.
- Keep the behavior in a small pure JSON utility seam plus a Vue panel that only coordinates input, selection, and presentation. Do not add a Rust action, IPC channel, database table, or settings record.
- Parse the input text in the renderer after a roughly 300ms debounce. Empty text resets the panel silently. A parse error preserves the text, clears every derived value, and shows an explicit error. A changed text must not continue to display a result derived from the previous text.
- Traverse the parsed JSON in deterministic depth-first document order. Treat the root path as `$`. Return the first array whose elements are all JSON objects; an empty array is eligible. Skip primitive arrays, arrays containing `null`, nested arrays, or any object/non-object mixture. Stop after the first eligible array; do not expose a path selector, wildcard path, or deep-array merge in this iteration.
- Represent the selected target by its concrete JSONPath label for display only. The filter operation receives the already located array value and does not implement a general JSONPath query language.
- Build the property-candidate list from the union of top-level keys across all target objects, deduplicated in first-seen order. Selecting a nested object or array property copies that value as-is; there is no nested-field flattening.
- Default every candidate to selected. Recompute the projection immediately whenever the selected set changes. For each object, iterate its own original entries and copy only selected keys that exist; missing keys are omitted and explicit `null` remains `null`.
- An empty selected set produces an array of empty objects with the same length as the target array. The result is always the projected array itself, serialized with stable two-space JSON formatting into a read-only output area.
- Keep the original input text as the sole source document. The projection is a newly constructed derived value and never mutates the parsed source or the user's text.
- Retain the panel's input, selected path, selected properties, and output for the current application lifetime so tab switches do not reset the workflow. Do not persist the document across application restarts.
- Provide clear loading/empty/error states, a clear-input action, a visible selected-property count, and a copy-result action. Use existing Element Plus and workbench styling conventions; keep the layout usable in narrow windows.
- Do not add an artificial byte or item limit in this iteration. Reuse the existing renderer-side JSON semantics and let parser/serializer errors remain explicit.

## Testing Decisions

- Tests assert observable JSON behavior and user workflow, not private helper arrangement, incidental DOM structure, or CSS implementation details.
- At the pure utility seam, cover: root-array selection; depth-first first-eligible selection; skipping primitive, `null`, nested, and mixed arrays; empty-array eligibility; no-usable-array results; first-seen candidate ordering; nested-value preservation; missing-key omission; original per-object key ordering; empty-selection projections; and input immutability.
- At the Vue panel behavior seam, cover: debounced parsing; empty input reset; visible invalid-JSON error; stale-state clearing after edits; first-path display; default-all checkbox state; selection-driven output updates; no-object-array empty state; formatted root-array output; copy success/failure feedback; clear action; and runtime state retention across unmount/remount where the existing panel pattern supports it.
- At the JSON workbench seam, verify the new tab is registered alongside the existing processing and schema tabs, does not change the default tab, and does not break existing clipboard input routing.
- Reuse the repository's existing Vitest conventions for pure utilities, source-structure checks, and happy-dom Vue behavior tests. No new IPC or end-to-end harness is warranted.
- Minimum verification is the targeted JSON utility and panel tests, followed by desktop type checking and the desktop web build. A full product UI launch is not required by this specification.

## Out of Scope

- Selecting among multiple arrays, editing a path manually, general JSONPath expressions, wildcard paths, or merging nested arrays from multiple records.
- Filtering primitive arrays, mixed arrays, or arrays whose elements are not all JSON objects.
- Flattening nested properties into checkbox candidates or changing nested values.
- Filtering records by value predicates, sorting, pagination, grouping, deduplication, joins, or aggregation.
- Preserving the original document's outer wrapper around the filtered array.
- Editing or saving the original JSON document, durable history, settings persistence, or cross-device synchronization.
- Backend/IPC processing, background workers, virtualized rendering, or a new large-document size policy.
- Replacing the existing processing or schema workflows, changing their input state, or redesigning JSON workbench navigation.

## Further Notes

The canonical terms are recorded in the project glossary: “JSON 输入文档”, “首个可用数组路径”, “对象数组”, “属性候选”, “缺失属性”, “默认全选”, “空属性投影”, “解析失败状态”, “无对象数组空态”, “自动解析”, and “数组过滤结果”. Implementation and UI copy should use these terms consistently.

The first-array-only rule is deliberate MVP scope: it keeps the interaction deterministic and avoids introducing a general JSONPath language. If later usage demonstrates a need for multiple targets or wildcard aggregation, that should be designed as a separate follow-up rather than inferred from this feature.
