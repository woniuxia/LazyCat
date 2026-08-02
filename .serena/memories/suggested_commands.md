# Suggested Commands

Run from repository root in PowerShell.

- Install: `pnpm install`
- Unit tests: `pnpm test`
- Type checks: `pnpm typecheck`
- Lint: `pnpm lint`
- Formatting check: `pnpm format:check`
- Renderer build: `pnpm --filter @lazycat/desktop build:web`
- Rust tests: `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`
- E2E: `pnpm test:e2e`
- Development UI: `pnpm dev` (agents require explicit user authorization before starting it)
- Default local Windows package: `pnpm package:win`
- NSIS installer: `pnpm build:win`
- Release: `pnpm release:win -- -Tag vX.Y.Z`; four-package release: `pnpm release:all:win -- -Tag vX.Y.Z`
- Diff hygiene: `git diff --check`; status: `git status --short`
- Prefer `rg` / `rg --files`; PowerShell uses `Get-Content -Raw`, `Get-ChildItem -Force`, and `;` rather than Unix-only utilities or `&&`.
