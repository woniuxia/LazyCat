# Changelog

All notable changes to this project are documented in this file.

## [0.1.0] - 2026-02-14

### Added

- Initial monorepo scaffold with `apps/*` and `packages/*`.
- Tauri 2 + Vue 3 desktop shell with Rust command bridge.
- Core tool modules:
  - codec, crypto, formatter, conversion, text processing
  - network and environment checks
  - file split/merge
  - image conversion/resize/crop/compress
  - regex, cron, uuid/guid/password, timestamp utilities
- Local persistence via `sql.js`.
- Offline manuals resource placeholders.
- Playwright E2E smoke tests and initial unit tests.
- Windows packaging configuration (NSIS + portable targets).
