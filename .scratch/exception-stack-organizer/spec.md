Type: task
Labels: ready-for-agent

# 异常堆栈整理器

## Problem Statement

全栈开发者在排查异常时，经常需要从 JavaScript/TypeScript 或 Java 的多行异常堆栈中快速找出异常类型、消息、调用位置和原因链。现有文本工具只能清洗或展示原文，不能识别堆栈结构、压缩成稳定的排查摘要，也不能明确指出哪些内容未被识别。

堆栈文本可能包含内部路径、业务信息和不完整的日志上下文。处理过程必须离线完成，且不能用猜测结果或静默丢弃内容制造伪成功。

## Solution

新增一个独立的“异常堆栈整理器”工具，沿用应用统一工具入口，但拥有独立面板、输入状态和解析状态。用户粘贴一条异常堆栈链后，显式触发解析；工具自动识别 JavaScript/TypeScript 或 Java，必要时允许手动指定格式，提取异常信息和调用帧，并生成可复制的规范化排查摘要。

工具保留原始输入，解析从整段输入的末尾向前识别调用帧，最终只在摘要中保留全局最后五个可识别堆栈帧，同时显示省略数量。无法识别的行和解析诊断单独展示；打开文件、复制和另存为均由用户显式触发，不保存内容历史，也不依赖网络。

## User Stories

1. As a developer, I want to open the exception stack organizer as an independent tool, so that I can investigate stack traces without entering a larger workbench.
2. As a developer, I want to paste a raw exception stack, so that I can inspect the text I already received from a terminal, IDE, issue, or chat.
3. As a developer, I want the raw input to remain visible and unchanged, so that I can compare the normalized result with the original evidence.
4. As a developer, I want to open a text file as the input, so that I can process a saved stack trace without copying it manually.
5. As a developer, I want the tool to recognize common JavaScript and TypeScript stack formats, so that browser and Node/V8 errors can use the same workflow.
6. As a developer, I want the tool to recognize common Java stack formats, so that ordinary Java exceptions can be summarized without manual editing.
7. As a developer, I want the tool to distinguish browser and Node/V8 JavaScript stacks when their common syntax differs, so that file, line, and column fields are extracted consistently.
8. As a developer, I want JavaScript and TypeScript source paths to be shown as they appear in the stack, so that the tool does not pretend to resolve source maps it does not have.
9. As a developer, I want Java cause chains to be retained, so that the underlying failure is not hidden behind the top-level exception.
10. As a developer, I want Java common-frame markers such as `... N more` to be represented as omitted-frame information, so that abbreviated traces remain understandable.
11. As a developer, I want the tool to auto-detect the input format, so that normal use does not require a format selection first.
12. As a developer, I want to manually override the detected format, so that an ambiguous or unusual input can still be parsed deliberately.
13. As a developer, I want uncertain detection to be visible, so that the tool does not silently apply an arbitrary parser.
14. As a developer, I want parsing to start only after I click the parse action or press `Ctrl+Enter`, so that editing or pasting a large stack does not repeatedly refresh the result.
15. As a developer, I want the parse action to make the current raw input explicit, so that a result can always be traced to one deliberate parse attempt.
16. As a developer, I want to see the detected or manually selected format, so that I know which interpretation produced the result.
17. As a developer, I want to see the top-level exception type and message, so that I can understand the immediate failure without scanning the whole text.
18. As a developer, I want each recognized frame to expose its callable name, file path, line, and column when available, so that I can locate the relevant code quickly.
19. As a developer, I want a JavaScript/TypeScript frame without a column to remain valid, so that missing optional fields do not make the entire result fail.
20. As a developer, I want a Java frame in the form `Class.method(File.java:line)` to be recognized, so that standard JVM traces produce structured locations.
21. As a developer, I want the exception cause chain displayed separately from the frame list, so that cause relationships are not confused with call order.
22. As a developer, I want one input to represent one root exception chain, so that unrelated stack traces are not silently merged.
23. As a developer, I want an input containing multiple independent exception blocks to produce a clear diagnostic, so that I know to split the evidence before parsing again.
24. As a developer, I want the parser to scan recognized frames from the end of the input backwards, so that the summary can focus on the final five frames required by the MVP.
25. As a developer, I want the five-frame limit to apply to the entire exception chain, so that output length stays predictable when several causes are present.
26. As a developer, I want the selected five frames displayed in their original source order, so that the call sequence remains readable.
27. As a developer, I want to see how many recognized frames were omitted, so that the shortened summary does not look like a complete trace.
28. As a developer, I want unrecognized lines preserved separately, so that parser limitations do not destroy evidence.
29. As a developer, I want a partially recognizable stack to show the useful fields that were found, so that one malformed line does not discard the entire investigation.
30. As a developer, I want a completely unrecognized input to show an explicit error and no fabricated summary, so that failure is distinguishable from a valid empty result.
31. As a developer, I want a normalized text summary, so that I can paste a concise stack into an issue or team conversation.
32. As a developer, I want structured exception and frame information alongside the summary, so that I can inspect details before copying them.
33. As a developer, I want to copy the normalized summary, so that I can reuse it in a ticket or debugging note quickly.
34. As a developer, I want copy failures to be visible, so that the UI does not claim success when clipboard permission is unavailable.
35. As a developer, I want to save the normalized summary to a new file, so that I can keep a cleaned diagnostic artifact without overwriting the source.
36. As a developer, I want the tool to require an explicit save destination, so that opening or parsing a file never changes it implicitly.
37. As a developer, I want a clear action to reset the current input and result, so that I can start a separate investigation without stale state.
38. As a developer, I do not want raw stack contents stored in history or settings automatically, so that sensitive paths and messages remain transient.
39. As a developer, I want the tool to remain usable without a network connection, so that local evidence never leaves the machine.
40. As a developer, I want the tool to remain usable in a narrow window, so that the raw input, structured result, diagnostics, and actions do not overlap.

## Implementation Decisions

- Treat the exception stack organizer as a separate tool entry and panel. Reuse the existing application shell, tool catalog, asynchronous component registry, clipboard behavior, file dialog, notification, and standard error presentation; keep parser state and result state private to this tool.
- Keep parsing in a pure renderer-side TypeScript module. The parser receives raw text and an optional format override, and returns the detected format, root exception information, cause-chain information, selected frames, omitted-frame count, unrecognized lines, and diagnostics. The parser performs no network access, file I/O, persistence, or UI work.
- Use the existing local file read and text write capabilities for optional file opening and explicit summary export. Do not add a Rust parser action, a new IPC channel, a database table, or a content-history model for this feature.
- Keep the user flow explicit: raw text is editable, parsing happens only from the parse action or `Ctrl+Enter`, and editing text alone does not update the derived result. A successful parse replaces the derived result; a failed parse clears derived fields for the attempted input while preserving the raw text and showing the diagnostic.
- Support only JavaScript/TypeScript and Java in the first version. JavaScript/TypeScript covers common browser and Node/V8 stack syntax. Java covers ordinary exception headers, `at` frames, `Caused by` chains, and `... N more` common-frame markers. Python and Rust are not part of this implementation.
- Auto-detect between the two supported formats and expose a manual format override. If detection is ambiguous or unsupported, show that state explicitly and do not silently fall back to a default parser.
- Define one input as one root exception chain. A top-level exception may contain Java cause information, but multiple unrelated root blocks are not merged; the parser returns a diagnostic and preserves the original text for that case.
- Preserve the source text as the evidence source. Recognized fields are derived values. Unrecognized lines remain visible in a separate diagnostic area and are never silently dropped or interpreted as a recognized frame.
- Collect all recognized frames in source order, scan from the end of the input backwards to select at most five frames globally, then present the selected frames in original order. Report the number of additional recognized frames omitted by the limit.
- Treat partial recognition as a usable but diagnosable result when an exception header or at least one frame is recognized. Treat an input with no meaningful recognized structure as a parse failure; do not generate an empty or guessed summary.
- Keep the result model split into an overview, cause chain, selected frame list, unrecognized-line diagnostics, and normalized copyable summary. The summary is derived from the same result and must not become a second source of truth.
- Do not resolve source maps, open source files, infer framework-specific semantics, inspect dependencies, or perform online symbolication. File paths and locations are shown only as supplied by the input.
- Do not persist input, output, diagnostics, file contents, or parse history. Tool-level favorites remain controlled by the application shell and are unrelated to stack content.
- Use existing light-theme and responsive panel conventions. Keep actions discoverable with text and familiar icons where the surrounding application already uses them; do not introduce a new workbench navigation model.
- Keep the parser line-oriented and linear in input size. Use bounded, anchored recognition rules rather than unbounded backtracking; synchronous renderer execution is acceptable for one explicit stack-chain parse. A worker or Rust implementation is reserved for a future scope that includes large files or streaming logs.

## Testing Decisions

- Tests assert observable parsing and user behavior, not private helper layout, incidental DOM structure, or a particular regular-expression implementation.
- The primary test seam is the pure parser module. Cover JavaScript/TypeScript browser and Node/V8 examples; Java exception headers and frames; `Caused by`; `... N more`; automatic detection; manual overrides; ambiguous and unsupported formats; one root-chain validation; optional frame fields; tail scanning; global five-frame selection; original-order presentation; omitted counts; partial recognition; unrecognized-line preservation; complete failure; and input immutability.
- At the panel behavior seam, cover explicit parse triggering, `Ctrl+Enter`, format override, preservation of raw input, clearing derived state after a failed parse, structured result rendering, diagnostic rendering, copy success/failure, file open failure, explicit save-as behavior, clear action, and the absence of content persistence. Do not duplicate every parser fixture in component tests.
- At the application registration seam, verify the new tool is present in the catalog, resolves through the existing component registry, and does not alter existing tool IDs or default navigation.
- Reuse repository Vitest conventions for pure utilities, source-structure checks, and happy-dom Vue behavior tests. Do not add a new Rust contract test or end-to-end harness because the feature introduces no Rust action or IPC contract.
- Verify in order: targeted parser and panel tests, desktop type checking, desktop web build, and a minimal manual panel smoke check if a product UI session is explicitly requested. No network or online-service verification is required.

## Out of Scope

- Python, Rust, or other stack formats beyond JavaScript/TypeScript and Java.
- Real-time log collection, log aggregation, APM integration, tailing, or multi-root stack merging.
- Source-map lookup, source-file opening, dependency resolution, framework-specific diagnosis, or online symbolication.
- Automatic root-cause inference, remediation suggestions, or claims about frames that were not present in the input.
- Editing or overwriting the source stack file.
- Automatic content history, settings persistence, cross-device synchronization, or cloud processing.
- A general-purpose log parser, arbitrary multiline text classifier, or large-file/streaming processing policy.
- A new standalone window, cross-tool workbench, shared domain state, or implementation of the text encoding and line-ending diagnostic tool.

## Further Notes

This is a focused implementation spec derived from the broader offline micro-tools roadmap. The current roadmap also contains a separate text encoding and line-ending candidate; that candidate remains outside this spec until its own behavior is confirmed.

The project glossary uses “离线核心路径” for the complete input-to-output path that works without network, remote authentication, CDN, or online services. For this feature, that path is raw stack input, local parsing, structured result, and optional explicit local export.
