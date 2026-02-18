# CLAUDE.md

This file provides project context and collaboration conventions for Claude or other coding agents.

## Project

- Name: Lazycat (懒猫)
- Type: Offline desktop developer toolbox
- Primary platform: Windows
- Runtime: Tauri 2 + Vue 3 + TypeScript

## Monorepo Layout

- `apps/desktop`: Tauri desktop app (Rust commands + Vue renderer)
- `packages/core`: codec, text tools, conversion, regex, cron, generators
- `packages/crypto`: RSA/AES/DES crypto wrappers
- `packages/formatters`: JSON/XML/HTML/Java/SQL formatters
- `packages/network`: connectivity/runtime/port checks
- `packages/file-tools`: file split/merge utilities
- `packages/image-tools`: image conversion/resize/crop/compress
- `packages/db`: sqlite persistence
- `packages/ipc-contracts`: request/response contracts
- `resources/manuals`: offline manual snapshots
- `resources/regex-library`: built-in regex templates

## Local Commands

- Install: `pnpm install`
- Dev: `pnpm dev`
- Typecheck: `pnpm typecheck`
- Build: `pnpm build`
- Build (with Windows precheck): `pnpm build:win:precheck`
- Unit tests: `pnpm test`
- E2E tests: `pnpm test:e2e`
- Windows package (NSIS): `pnpm build:win`
- Windows package (portable alias): `pnpm build:portable`

## Agent Collaboration Rules

- Do not start the app/dev server automatically.
- Only run `pnpm dev` (or any command that launches the desktop UI) when the user explicitly asks to start it.

## Encoding & Garbled Text (Important)

- Recent issue: garbled Chinese text introduced template/script corruption in `apps/desktop/src/App.vue`:
  - broken quoted attributes (missing closing `"`),
  - broken button closing tags (e.g. `?/el-button>`),
  - broken string literals in `<script>` (unterminated quotes).
- Typical symptoms:
  - Vite/Vue parse errors such as:
    - `Attribute name cannot contain ...`
    - `Unquoted attribute value cannot contain ...`
    - `Unterminated string constant`
    - `Error parsing JavaScript expression`
- Required handling when touching UI text:
  - Preserve valid UTF-8 and avoid bulk replacements that may alter punctuation/quotes.
  - Prefer small, targeted edits.
  - If garbling is detected, fix structural correctness first (quotes/tags), then fix display text.
- Required verification after text edits in Vue files:
  1. `pnpm --filter @lazycat/desktop typecheck`
  2. `pnpm --filter @lazycat/desktop build:web`
- Extra note for formatter feature:
  - Prettier in renderer path must use standalone + explicit plugins (`prettier/standalone`, plus parser plugins), otherwise parser resolution may fail at runtime.

## Architecture Notes

- Frontend ↔ backend call path:
  - Vue calls `invokeToolByChannel` in `apps/desktop/src/bridge/tauri.ts`
  - Channel strings (e.g. `tool:encode:base64-encode`) are mapped to `{domain, action}` via `CHANNEL_MAP`
  - Tauri command `tool_execute` dispatches in Rust via a single `match (domain, action)` in `main.rs`
- Frontend routing: no vue-router; single `App.vue` uses `v-else-if` chains on `activeTool` ref to switch panels
- Extracted sub-components in `apps/desktop/src/components/`: `HomePanel`, `CalcDraftPanel`, `FormatterPanel`, `RegexPanel`, `HostsPanel`, `PortsPanel`, `MonacoPane`
- Formatter architecture: XML/HTML/Java/SQL formatting is **passthrough in Rust**; actual formatting is done by `@lazycat/formatters` (Prettier standalone) in the renderer
- Cron preview (`cron.preview`) is currently a **stub** — returns placeholder strings, not real next-fire times
- Hosts activate requires **administrator privileges** to write `C:\Windows\System32\drivers\etc\hosts`; auto-backs up original before overwrite
- Runtime data:
  - App data under `%USERPROFILE%\\.lazycat`
  - Hosts profiles in sqlite
  - Hosts backup directory managed by Rust side
- State persistence: favorites, tool click history, calc draft history stored in `localStorage`; hosts profiles in SQLite

## Important Runtime Paths

- DB file: `%USERPROFILE%\\.lazycat\\lazycat.sqlite`
- Hosts backups: `%USERPROFILE%\\.lazycat\\hosts-backups`

## Current Known Constraints

- `pnpm build` requires Rust toolchain (`cargo`, `rustc`) and platform prerequisites.
- On Windows, vendored OpenSSL requires `perl` (e.g. Strawberry Perl).
- All Rust tool logic lives in a single `apps/desktop/src-tauri/src/main.rs` (~990 lines); no module splitting yet.
- `packages/core`, `packages/crypto`, `packages/db`, `packages/file-tools`, `packages/image-tools`, `packages/network`, `packages/ipc-contracts` are currently stubs or thin wrappers — actual logic runs in Rust. Only `packages/formatters` is actively used by the renderer (Prettier standalone).
- Cron preview is a stub (returns placeholder strings). Real next-fire-time calculation not yet implemented.
- XML/HTML/Java/SQL formatters in Rust are passthrough; formatting quality depends on `@lazycat/formatters` (Prettier).
- Offline manuals are placeholder snapshots and can be replaced by full static docs.
- Hosts activate requires running the app as Administrator on Windows.

## Commit Conventions

- Conventional commits preferred:
  - `feat: ...`
  - `fix: ...`
  - `docs: ...`
  - `chore: ...`
  - `test: ...`
- Keep each commit focused by domain (ui/core/build/test).

## Pre-commit Gate

Run these before pushing:

1. `pnpm typecheck`
2. `pnpm --filter @lazycat/desktop build:web` (renderer)
3. `pnpm test`
4. `pnpm test:e2e`

If packaging is required:

5. `pnpm build:win`
