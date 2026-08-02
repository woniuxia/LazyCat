# Tech Stack

- Monorepo: pnpm workspace; pinned `pnpm@10.18.1`, Node `>=22.12`.
- Renderer: Vue 3.5, TypeScript 5.7 strict mode, Vite 6, Element Plus 2, Tauri JS API 2.
- Desktop backend: Rust edition 2021, Tauri 2; async work uses Tokio; persistence commonly uses bundled SQLite through rusqlite.
- Tests: Vitest 3 for TypeScript/Vue, Playwright 1 for E2E, Cargo test for Rust.
- Formatting/quality: Prettier 3, ESLint 10 with TypeScript and Vue flat configs.
- Source encoding UTF-8; repository text EOL is LF except Windows scripts governed by `.gitattributes`.
- Desktop artifacts must be produced by Tauri build; bare `cargo build --release` is not a valid app delivery path.
