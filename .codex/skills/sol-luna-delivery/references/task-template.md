# Luna Task Packet

Fill every section with concrete task information, then send the complete packet directly as the `spawn_agent` message with `fork_turns = "none"`.

## Task Type

`read-only-analysis` or `implementation`

## Goal

State one concrete result Luna must deliver.

## Context

Provide only the repository facts, file paths, current behavior, and constraints needed to execute the task.

## Confirmed Decisions

List decisions already made by Sol or the user. Luna must not revisit them.

## Executable Design

Write the directly executable detailed design frozen by Sol. Do not delegate approach selection or requirement interpretation to Luna. For `implementation`, name exact target files and relevant symbols or sections, required changes, applicable behavior/data/control-flow contracts, edge/failure/compatibility constraints, and test changes or validation order. For `read-only-analysis`, name exact evidence sources, search or inspection method, applicable classification/deduplication rules, and independent completeness checks.

## Non-goals

List behavior and adjacent work that must remain unchanged. Write `None` only when the scope truly has no exclusions.

## Allowed Scope

List the files, directories, commands, and external actions Luna may use.

## Allowed Changed Paths

Provide a JSON array of exact repository-relative file paths. Use `[]` for read-only tasks. Wildcards, directories, absolute paths, parent traversal, and prose placeholders such as "corresponding tests" are not allowed. Sol must discover every path before delegation.

## Forbidden Scope

List files, behaviors, destructive actions, services, commits, packaging, publishing, child-agent creation, or other work Luna must not perform.

## Steps

Provide the execution order derived from the frozen `Executable Design`; it must not substitute for that design. Leave only explicitly named mechanical choices to Luna and state where it must stop. Prefer exact target reads and batch independent read-only commands. Limit individual command output. Run targeted validation first and any full build only once at the end.

## Acceptance Criteria

Define observable completion conditions, including behavior and preservation requirements.

## Validation

List exact commands or reproducible checks. Distinguish required checks from checks that are not applicable.

## Stop Conditions

Define ambiguity, conflicts, failures, scope changes, dirty-worktree changes, or missing prerequisites that require a `blocked` result.

## Output Requirements

Require the Markdown response contract from `SKILL.md` plus task-specific evidence such as file and line references, changed files, validation-relevant commands, exit codes, or remaining gaps. Do not repeat routine successful exploration commands unless they support a finding.
