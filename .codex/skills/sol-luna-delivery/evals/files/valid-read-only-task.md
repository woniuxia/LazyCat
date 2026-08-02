# Luna Task Packet

## Task Type

read-only-analysis

## Goal

Inventory every tracked Markdown file under docs and report the count.

## Context

The repository is E:\Projects\LazyCat. Use Git-tracked files as the source of truth.

## Confirmed Decisions

Only Markdown files under docs are in scope. Grouping or content analysis is not required.

## Executable Design

Sol's frozen design is to use `git ls-files -- docs` as the tracked-file evidence source, filter tracked paths whose names end in `.md`, and count the filtered paths. Run an independent tracked-file listing and filtering command as a completeness check and compare its count; do not delegate method selection or content inspection to Luna.

## Non-goals

Do not edit files, inspect generated output, or propose documentation changes.

## Allowed Scope

Read Git metadata and paths under docs. Run read-only Git and ripgrep commands.

## Allowed Changed Paths

[]

## Forbidden Scope

Do not write files, start services, commit, package, publish, or access the network.

## Steps

List tracked files under docs, select Markdown files, count them, and retain the command as evidence.

## Acceptance Criteria

Return one count backed by the exact command and exit code. Report any uninspected scope.

## Validation

Run an independent tracked-file listing and confirm the same count.

## Stop Conditions

Return blocked if Git metadata cannot be read or the repository root differs from the supplied context.

## Output Requirements

Include the count in `Summary` and both read-only commands with exit codes in `Commands`.
