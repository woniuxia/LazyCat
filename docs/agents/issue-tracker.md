# Issue tracker: Local Markdown

Issues and specs (you may know a spec as a PRD) for this repo live as markdown files in `.scratch/`.

## Conventions

- One feature per directory: `.scratch/<feature-slug>/`
- The spec is `.scratch/<feature-slug>/spec.md`
- Implementation issues are one file per ticket at `.scratch/<feature-slug>/issues/<NN>-<slug>.md`, numbered from `01`
- Comments and conversation history append to the bottom of the file under a `## Comments` heading

## When a skill says "publish to the issue tracker"

Create a new file under `.scratch/<feature-slug>/`, creating the directory if needed.

## When a skill says "fetch the relevant ticket"

Read the file at the referenced path. The user will normally pass the path or issue number directly.

## Wayfinding operations

Used by `/wayfinder`. The map has one child file per ticket.

- **Map**: `.scratch/<effort>/map.md`
- **Child ticket**: `.scratch/<effort>/issues/NN-<slug>.md`, numbered from `01`
- **Type**: a `Type:` line records `research`, `prototype`, `grilling`, or `task`
- **Status**: a `Status:` line records `claimed` or `resolved`
- **Blocking**: a `Blocked by: NN, NN` line lists unresolved dependencies
- **Frontier**: scan for open, unblocked, unclaimed tickets; first by number wins
- **Claim**: set `Status: claimed` and save before starting work
- **Resolve**: append the answer under `## Answer`, set `Status: resolved`, then add a context pointer to the map
