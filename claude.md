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

## Architecture Notes

- Frontend ↔ backend call path:
  - Vue calls `invokeToolByChannel` in `apps/desktop/src/bridge/tauri.ts`
  - Channel strings are mapped to `{domain, action}`
  - Tauri command `tool_execute` dispatches in Rust
- Runtime data:
  - App data under `%USERPROFILE%\\.lazycat`
  - Hosts profiles in sqlite
  - Hosts backup directory managed by Rust side

## Important Runtime Paths

- Startup logs: `%USERPROFILE%\\.lazycat\\logs\\startup.log`
- DB file: `%USERPROFILE%\\.lazycat\\lazycat.sqlite`
- Hosts backups: `%USERPROFILE%\\.lazycat\\hosts-backups`

## Current Known Constraints

- `pnpm build` requires Rust toolchain (`cargo`, `rustc`) and platform prerequisites.
- On Windows, vendored OpenSSL requires `perl` (e.g. Strawberry Perl).
- Some tools currently use pragmatic implementations and can be deepened (e.g. cron preview precision, advanced formatters).
- Offline manuals are placeholder snapshots and can be replaced by full static docs.

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
