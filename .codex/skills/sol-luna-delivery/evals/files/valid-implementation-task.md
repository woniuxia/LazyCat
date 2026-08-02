# Executor Task Packet

## Task Type

implementation

## Goal

Apply one explicitly supplied documentation correction and validate the changed Markdown file.

## Context

The caller will replace this fixture with a real task packet before any live execution.

## Confirmed Decisions

The wording, target file, and expected result must already be fixed by Sol.

## Non-goals

Do not change product code, dependencies, behavior, or adjacent documentation.

## Allowed Scope

Modify only the single documentation file named by the real task packet and run read-only validation commands.

## Allowed Changed Paths

[
  "docs/example.md"
]

## Forbidden Scope

Do not commit, reset, restore, package, publish, start services, or access the network.

## Steps

Read the target diff, apply the exact wording correction, and run the supplied validation.

## Acceptance Criteria

Only the allowed file changes, the requested wording is present, and all supplied validation passes.

## Validation

Run the task-specific link or keyword check and git diff --check.

## Stop Conditions

Return blocked if the target file or exact replacement is missing, the file has conflicting changes, or validation cannot run.

## Output Requirements

Report the changed file, commands with exit codes, validation evidence, and remaining gaps.
