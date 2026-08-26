Type: task
Labels: ready-for-agent

# 上线包运行日志独立文件保存

## Problem Statement

上线包当前只通过运行期事件把前端构建、后端构建和服务器上传日志送到页面内存。每个日志区最多保留最近 1000 条，应用重启后全部丢失；任务异常退出时，用户也无法找回已经产生的诊断信息。长时间构建、上传失败、上传后命令失败或健康检查异常发生后，日志因页面关闭或应用退出而消失，用户无法可靠复盘某次上线包运行。

当前阶段需要先建立可靠、有限且可诊断的本地日志保存能力。历史查看界面和滚动交互优化后续单独实施，本阶段不能借持久化之名扩大到历史浏览器或重新设计实时日志区。

## Solution

每次上线包普通运行、上传重试和上传后命令重试都在 LazyCat 应用数据目录中创建独立的上线包运行记录。一次运行拥有一个目录、一个原子更新的 `manifest.json`，以及按前端、后端和上传三个现有日志区分别保存的 UTF-8 文本日志。运行记录归属于当时的项目和环境；重试创建新记录并关联来源运行，不向原记录追加。

日志在运行期间增量写入，按 500ms 或累计 64KiB 的先到条件刷新，并在运行结束、中止和正常退出时强制刷新。应用异常退出后仍处于运行中的 manifest 在下次启动时转为未完成运行记录，保留已经落盘的内容，不伪造失败或已中止结果。

每个日志区实施 20MiB 容量保护：固定保留开头 1MiB，并通过可直接阅读的 UTF-8 分段文件保留最新 19MiB；中间丢弃量记录在 manifest 中。自动保留策略限制每个项目最近 50 次、最长 90 天，以及所有上线包日志合计 1GiB，任何规则触发时都从最旧的非活动记录开始清理。

日志保存属于非关键可降级能力。创建目录、追加内容、刷新、更新 manifest 或清理失败不得篡改打包和上传结果，但必须把失败动作、目标路径和原因作为独立保存告警返回当前页面。应用掌控的密码、口令和令牌不得进入日志；健康检查 URL 在进入日志前移除查询参数和片段；用户命令自行输出的 stdout/stderr 保持原文。

## User Stories

1. As a release operator, I want each release-package run saved locally, so that closing the page does not erase its diagnostic output.
2. As a release operator, I want logs to survive an application restart, so that I can investigate a run after reopening LazyCat.
3. As a release operator, I want a normal packaging run to create its own run record, so that unrelated executions are never mixed.
4. As a release operator, I want an upload retry to create a separate run record, so that each delivery attempt retains its own result.
5. As a release operator, I want a post-upload command retry to create a separate run record, so that retry output does not rewrite the original attempt.
6. As a release operator, I want retry records linked to their source run, so that a future history view can reconstruct the attempt chain.
7. As a release operator, I want run records associated with the project and environment used at start time, so that later project selection cannot reassign historical facts.
8. As a release operator, I want the project and environment display identity snapshotted in the manifest, so that files remain understandable if names later change.
9. As a release operator, I want frontend build logs stored independently, so that frontend diagnostics remain easy to isolate.
10. As a release operator, I want backend build logs stored independently, so that backend diagnostics remain easy to isolate.
11. As a release operator, I want upload, post-upload command and health-check logs stored in the upload lane, so that server delivery remains one coherent diagnostic stream.
12. As a release operator, I want stdout and stderr preserved on every saved line, so that warnings and failures retain their original stream semantics.
13. As a release operator, I want every saved line timestamped when emitted, so that I can reconstruct the execution sequence.
14. As a release operator, I want log files to remain readable UTF-8 text, so that I can inspect them before LazyCat has a history viewer.
15. As a release operator, I want files written incrementally during a run, so that an abnormal exit does not discard the whole log.
16. As a release operator, I want buffered writes rather than a disk sync for every line, so that high-volume output does not unnecessarily slow packaging.
17. As a release operator, I want buffered output flushed at bounded intervals, so that an application crash loses at most a small recent window under normal filesystem behavior.
18. As a release operator, I want pending output flushed when a run succeeds, fails or is canceled, so that terminal diagnostics are retained.
19. As a release operator, I want pending output flushed during normal application shutdown, so that closing LazyCat does not silently drop buffered lines.
20. As a release operator, I want an abnormally interrupted run marked as incomplete after restart, so that missing terminal state is not mislabeled as failure or cancellation.
21. As a release operator, I want the already saved lines of an incomplete run retained, so that partial evidence remains useful.
22. As a release operator, I want each log lane bounded, so that a runaway command cannot consume unlimited disk space.
23. As a release operator, I want the start and latest output retained when a lane exceeds its limit, so that both startup context and the final error remain available.
24. As a release operator, I want truncation recorded with discarded byte counts, so that incomplete log content is never mistaken for a full transcript.
25. As a release operator, I want rotated segments directly readable, so that storage does not depend on a proprietary decoder.
26. As a LazyCat user, I want old run records cleaned automatically, so that logs do not grow without bound.
27. As a LazyCat user, I want active run files protected from automatic cleanup, so that retention cannot corrupt an ongoing operation.
28. As a LazyCat user, I want retention evaluated after startup and run completion, so that stale files are cleaned without a background maintenance service.
29. As a LazyCat user, I want per-project count and age limits, so that one long-lived project does not retain unlimited history.
30. As a LazyCat user, I want a global storage budget, so that many projects cannot collectively exceed the agreed disk boundary.
31. As a LazyCat user, I want project deletion to remove its run records, so that inaccessible orphan logs do not remain on disk.
32. As a LazyCat user, I want project deletion confirmation to disclose the affected run count and disk usage, so that the destructive scope is explicit.
33. As a release operator, I want packaging and upload to continue if log storage fails, so that an observability problem does not manufacture a delivery failure.
34. As a release operator, I want a visible storage warning when saving fails, so that successful packaging is not confused with successful history preservation.
35. As a release operator, I want storage warnings to include the failed action and path, so that permission, disk and filesystem problems are diagnosable.
36. As a release operator, I want unaffected log lanes to continue saving after one lane fails, so that a partial storage failure loses no more evidence than necessary.
37. As a release operator, I want the run manifest to record partial persistence and truncation, so that future readers can assess the record's completeness.
38. As a security-conscious user, I want application-managed passwords, passphrases and preflight tokens excluded from logs, so that persistence does not widen secret exposure.
39. As a security-conscious user, I want health-check URL queries and fragments removed before logging, so that URL-carried tokens are not written to disk.
40. As a release operator, I want user-command output preserved without heuristic rewriting, so that unreliable generic redaction does not corrupt diagnostic evidence.
41. As a security-conscious user, I want the manifest to state that command output may contain user-emitted sensitive data, so that direct file inspection carries an explicit warning.
42. As an existing release-package user, I want current packaging, upload, cancellation, retry and result semantics unchanged, so that adding storage does not alter delivery behavior.
43. As an existing release-package user, I want current live log events to continue working, so that persistence does not replace the page's runtime feedback.
44. As a future history-view user, I want versioned manifests and deterministic segment ordering, so that a later viewer can read old records without guessing their layout.
45. As a future history-view user, I want missing or corrupt files reported as record damage rather than ignored, so that filesystem problems do not become false completeness.

## Implementation Decisions

- Treat this specification as the first persistence phase only. It saves上线包运行记录 but does not add a history list, history detail view, manual history deletion, search, export or scroll controls.
- Use the configured LazyCat application data directory as the storage root, under a dedicated release-package log namespace. Do not write logs into source projects, build output directories or local archive destinations.
- Use independent files rather than SQLite. This is a deliberate choice so logs remain inspectable with ordinary text tools and do not enlarge the main application database with high-volume append data.
- Make the filesystem layout self-describing. Each run directory is nested by stable project ID and run ID and contains a versioned manifest plus zero or more frontend, backend and upload text segments. A lane with no output does not require an empty file.
- Make the run directory and manifest files the single source of persisted truth. Do not introduce a SQLite index or global mutable `index.json`; future listing derives records from per-run manifests and may use only rebuildable in-memory caches.
- Store enough manifest metadata to identify and interpret the run without a database join: schema version, run ID, stable project and environment IDs, project-name snapshot, environment kind, operation kind, optional source run ID and retry kind, start/update/end timestamps, lifecycle result, persistence completeness, lane segment order and sizes, and discarded-byte counts.
- Create the initial manifest with `running` before the worker begins producing logs. Update manifest content through write-to-temporary-file plus atomic replacement so readers never accept a partially serialized manifest.
- On startup, records still marked `running` from a previous process become未完成运行记录. Do not infer `failed` or `cancelled`; those outcomes require an actual terminal result.
- Give one run-scoped persistence owner responsibility for directory creation, lane buffers, segment rotation, manifest updates, terminal finalization and release. Runtime events and the renderer remain mirrors of the live operation, not the persisted source of truth.
- Preserve the existing single-active-run invariant. Normal start, upload retry and command retry each create their persistence owner only after runtime admission succeeds, and release it on every success, failure, cancellation, panic-safe cleanup and startup-failure path.
- Treat upload and command retries as independent run identities. Their manifests reference the consumed source run and retry kind; they never reopen or append to an earlier directory.
- Route output through the existing frontend, backend and upload lane semantics. Upload includes transfer, post-upload command and health-check diagnostics. Persistence must not create a competing phase taxonomy.
- Format every physical log line as an RFC 3339 timestamp with local offset, followed by the original stream marker and line text. The timestamp is captured when the log event is emitted, not when a buffer is flushed.
- Normalize event boundaries so each emitted log entry occupies one textual record while preserving the original line content. Files use UTF-8 and platform-independent newline handling documented by the manifest schema.
- Buffer each lane independently. Flush when 500ms has elapsed or buffered content reaches 64KiB, whichever occurs first; force a final flush on terminal result, user cancellation and normal application shutdown.
- The 500ms policy covers application crashes under normal filesystem behavior, not sudden power-loss durability. Do not call a full disk synchronization for every line.
- Limit each logical lane to 20MiB of retained content. Preserve the first 1MiB as startup context and the latest 19MiB as ordered rotating UTF-8 text segments. Record omitted bytes and insert an explicit logical omission marker when the record is later assembled.
- Keep segment names deterministic and record their logical display order in the manifest. Rotation must never expose a partially replaced segment as a valid completed segment.
- Preserve the current live-page limit of 1000 entries per lane. Disk capacity and renderer memory limits are separate concerns; this phase does not alter realtime scrolling or its existing length watcher.
- Apply fixed retention rules in the first phase: at most 50 run records per project, at most 90 days, and at most 1GiB across all release-package logs. There is no settings UI in this phase.
- Run retention reconciliation during application startup and after a run is finalized. Delete the oldest non-active records until all applicable boundaries hold. Never select an active run directory for cleanup.
- Treat an incomplete record as non-active after startup reconciliation and subject it to the same age, count and global limits. If metadata is corrupt or insufficient to prove a directory is an eligible record, leave it untouched and report a diagnostic rather than deleting unknown data.
- Extend project deletion to calculate the associated log record count and bytes, expose that scope in the existing confirmation, and remove the project log directory only after project deletion is confirmed. A deletion failure must be explicit and must not report complete cleanup.
- Treat project deletion across the database and filesystem as a recoverable two-resource operation. Atomically rename the project log directory to a run-unique deletion tombstone before committing project deletion; restore the original directory if the database delete fails, and retry tombstone removal on startup if final filesystem cleanup fails after the project is gone. Tombstones are never exposed as valid history. If log scope cannot be inspected before confirmation, block deletion with the inspection error instead of displaying a false count or silently excluding files.
- Keep log persistence failure independent from the delivery result. A build or upload may succeed while its run record is incomplete; do not replace the existing overall or target result with a storage error.
- Add a distinct persistence warning to the runtime/status contract and renderer state. The current run-log card displays the warning with failed action, target path and cause while retaining the original packaging status and errors.
- Coalesce repeated failures from the same persistence operation so high-volume output cannot flood events or notifications. Continue saving unaffected lanes and metadata whenever that remains safe; manifest completeness records every failed lane or phase that can still be persisted.
- Never log application-managed server passwords, private-key passphrases, Vault master passwords, preflight tokens or host probe tokens. Preserve existing sensitive startup cleanup and do not add secret fields to manifests.
- Remove query parameters and fragments from health-check URLs before they enter either live log events or persisted text. This prevents the live and stored representations from becoming conflicting security truths.
- Preserve user-controlled build and remote-command stdout/stderr verbatim after line-boundary normalization. Do not claim generic secret detection. Mark manifests so future UI can warn that user-emitted command output may contain sensitive information.
- Do not backfill historical runs because no durable source exists. Persistence begins with runs started after this feature is installed.
- Record the independent-file choice in an ADR because it is a deliberate, costly-to-reverse deviation from the repository's existing SQLite log pattern, chosen for direct readability and isolation of high-volume append data.

## Testing Decisions

- Tests assert externally observable files, manifests, status events and rendered warnings. They do not assert private buffer types, timer helper names, incidental thread structure or source-code substrings.
- Use two confirmed seams because the behavior crosses the native runtime and renderer: the run-event-to-filesystem integration seam and the mounted release-package panel seam.
- At the primary Rust integration seam, feed real running, log and terminal events through the file-backed persistence boundary using a temporary application-data root. Inspect the resulting directory, manifest and text files as a user or future reader would.
- At that seam, cover a normal local archive run, a server upload run, an upload retry and a command retry. Assert stable identities, separate directories, retry linkage, terminal status and frontend/backend/upload routing.
- Assert the textual contract: UTF-8 readability, RFC 3339 emission timestamps, stdout/stderr markers, original line content, deterministic segment ordering and optional omission of empty lanes.
- Use a controlled clock and explicit flush hook to verify the 500ms/64KiB thresholds without wall-clock sleeps. Assert terminal, cancellation and normal-shutdown flush behavior through persisted output.
- Simulate an application restart with a manifest left in `running` and assert that it becomes incomplete while retaining saved logs. Do not assert a fabricated failure or cancellation result.
- Drive more than 20MiB through each lane boundary and assert the retained first 1MiB, latest 19MiB, readable segments, omission marker metadata and exact discarded-byte accounting.
- Seed temporary roots with records spanning project counts, ages and total sizes, run reconciliation, and assert deterministic oldest-first cleanup while an active record remains untouched.
- Cover corrupt manifests, missing segments and deletion failures. Unknown filesystem content must survive cleanup, and every skipped or failed operation must expose diagnostic context.
- Exercise project deletion through the highest existing project action seam with a temporary data root. Assert the disclosed record count and bytes, confirmed cascade cleanup, and explicit partial failure behavior.
- Inject directory-create, append, flush, manifest-replace and cleanup failures through the filesystem boundary. Assert that the release result remains unchanged, a distinct persistence warning is emitted, repeated warnings are coalesced, and unaffected lanes continue where safe.
- Verify sensitive-data boundaries at the same Rust seam: application-managed secret fields never enter manifests or log text, health-check URL query and fragment content is absent, and user-command stdout/stderr is otherwise unchanged.
- At the mounted Vue panel seam, send a runtime status carrying a persistence warning. Assert that the current log card renders the action, path and cause while preserving the original overall and target status.
- At the mounted panel seam, also assert warning replacement or coalescing across repeated events and clearing when a new run begins. Do not test the deferred history UI or scrolling behavior.
- Reuse the existing release-package event sink and pipeline test patterns, temporary-directory Rust tests, mounted Vue renderer harness and mocked Tauri event bridge. Introduce no new end-to-end framework.
- Minimum automated verification is targeted Rust release-package tests, targeted release-package Vue tests, workspace type checking, desktop web build and `git diff --check`.
- Runtime acceptance for implementation should execute at least one real local packaging run and one server-upload or controlled upload-failure path, inspect the application data files, then restart LazyCat and verify incomplete reconciliation with a deliberately interrupted run. Starting the product UI still requires explicit user authorization; if not authorized, report this runtime acceptance as outstanding.

## Out of Scope

- Top/bottom log navigation buttons, automatic-follow changes, the existing 1000-line watcher defect and any other realtime log scrolling optimization.
- A history list, history detail view, switching between live and historical logs, or loading saved segments into the renderer.
- Manual deletion of one run, clearing project history from a history page, or a cross-project cleanup interface.
- Full-text search, filtering, export, sharing, archive packaging or cross-project aggregation of saved logs.
- User-configurable retention days, run counts, capacity limits, storage locations, flush intervals or segment sizes.
- SQLite run or log tables, a global history index, cloud synchronization or remote log storage.
- Generic keyword or regular-expression redaction of user-command output.
- Changing build, archive, upload transaction, rollback, cancellation, retry, post-upload command, health-check result or notification semantics.
- Recovering or resuming an interrupted package or upload operation. An incomplete record preserves evidence only.
- Backfilling logs for runs completed before this feature exists.
- Automatically starting the product UI, packaging installers, publishing a release, or pushing commits as part of specification work.

## Further Notes

The canonical domain terms are“上线包运行记录”“未完成运行记录”和“运行日志截断”. A record is historical evidence, not a resumable job. Truncation is an explicit storage boundary and does not change the delivery result.

The chosen independent-file model intentionally trades database query convenience for direct readability, isolation from the main SQLite database and safer handling of high-volume append data. The per-run manifest is therefore the only durable metadata truth; a later history viewer must tolerate missing or damaged files rather than manufacturing completeness.

The fixed retention policy and project-delete cascade are part of safe storage, even though manual history management is deferred. Without them, “先存起来” would create unbounded or orphaned files before the viewer exists.

Command output is trusted as diagnostic evidence, not as secret-safe content. LazyCat prevents secrets it owns from entering the pipeline and sanitizes health-check URLs, but a user script can still print sensitive values. Direct inspection and any future history UI must preserve this distinction.
