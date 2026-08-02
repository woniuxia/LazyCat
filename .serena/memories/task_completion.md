# Task Completion

- Start with the most targeted regression/unit test for the changed behavior.
- Then run `pnpm typecheck`.
- Renderer changes: run `pnpm --filter @lazycat/desktop build:web`.
- Rust changes: run the relevant Cargo test, normally `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml <filter>`; broaden when shared state, IPC, transaction, or concurrency behavior changes.
- Cross-layer changes require frontend/bridge/Rust/type/test contract checks; data migrations require real old-schema upgrade, read/write, and second-start coverage.
- UI runtime/visual acceptance requires explicit authorization to start the product UI. If not run, state that visual acceptance is incomplete.
- Before commit: `git diff --check`, inspect `git status --short`, and stage only task files.
- Commit only a complete, independently verifiable stage with passing relevant checks. Use a Chinese conventional prefix such as `feat:`, `fix:`, `docs:`, `chore:`, or `test:`.
- Never describe static inspection, typecheck, or build success as runtime behavior verification.
