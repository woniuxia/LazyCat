# Backend Core

- Tauri crate: `apps/desktop/src-tauri/`; entrypoints are `src/main.rs` and `src/lib.rs`.
- Shared desktop services live in top-level modules such as background lifecycle, events, shortcuts, windows, clipboard, notifications, and manual server.
- Tool domains live under `src/tools/` (including action center, data dictionary, request forwarding, todo, widget, conversion, and diagnostics).
- Normal frontend call chain: `invokeToolByChannel` -> Tauri `tool_execute` -> Rust `execute_tool` -> domain module.
- Keep dispatch in the owning entry module; move cohesive business logic into domain modules. Test-only helpers/re-exports require `#[cfg(test)]`.
- SQLite migrations must be transactional, idempotent, explicitly failing, and verified against representative old schemas. Do not mask structural migration failure with defaults.
- The patched `vendor/auto-launch` crate is active through `[patch.crates-io]`; do not treat it as disposable generated code.
- Backend architecture, capability, window, IPC, migration, and lifecycle guidance is in `docs/experience/architecture.md`; domain-specific guidance is indexed by `docs/experience/README.md`.
