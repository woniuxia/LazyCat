# Conventions

- Make the smallest complete change; avoid opportunistic refactors, extra layers, hidden behavior, and new dependencies without a concrete need.
- Keep APIs small, naming explicit, control flow flat, and errors contextual. Never swallow errors or return false success; best-effort degradation must remain diagnosable.
- Root-cause repeated state/concurrency/transaction failures by defining the single source of truth, owner, lifecycle, transaction boundary, release path, and recovery semantics.
- TypeScript is strict. Prettier: 2 spaces, semicolons, double quotes, trailing commas, 100 columns, LF.
- Shared types and pure logic belong in `src/types/` and `src/utils/`; Vue components primarily orchestrate state and bind UI.
- Frontend tool integration follows `App.vue` + `tool-registry.ts`; backend calls flow through `bridge/tauri.ts` and the Rust tool dispatch. Do not create parallel IPC contracts.
- UI defaults to clean light/white styling; significant visual direction changes require confirmation. Check both Element Plus themes, Teleport scope, narrow widths, overflow, and stable conditional layouts.
- Preserve unrelated dirty-worktree changes. Read the target diff before modifying an already changed file.
- Add regression tests at the lowest stable layer; tests/build/typecheck are evidence only for what they actually execute.
