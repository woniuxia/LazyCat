# LazyCat Core

- Windows-first offline developer toolbox; Tauri desktop delivery is the product boundary.
- Authoritative working rules: `AGENTS.md`; workflow and experience index: `process.md`, `docs/experience/README.md`.
- Source map:
  - Vue renderer: `apps/desktop/src/`; module notes: `mem:frontend/core`.
  - Rust/Tauri backend: `apps/desktop/src-tauri/`; module notes: `mem:backend/core`.
  - Shared formatter package: `packages/formatters/`.
  - Offline assets/manuals: `resources/`; build/release automation: `scripts/`.
- Cross-layer changes must close the full chain: UI entry/registry -> frontend type and bridge -> Rust dispatch/module -> tests/docs/resources as applicable.
- Runtime assets must be local; no public CDN dependency.
- Project stack and pins: `mem:tech_stack`.
- Coding and architectural invariants: `mem:conventions`.
- Common commands: `mem:suggested_commands`.
- Completion and validation gates: `mem:task_completion`.
