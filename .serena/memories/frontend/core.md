# Frontend Core

- Renderer root: `apps/desktop/src/`; Vue 3 Composition API + TypeScript.
- `App.vue` owns the main tool surface; `tool-registry.ts` maps tool IDs to async panel components.
- `bridge/tauri.ts` is the centralized frontend IPC channel/contract boundary. Prefer `invokeToolByChannel`; keep channel names, request/response types, mocks, Rust actions, and fixtures synchronized.
- `components/` contains panels and reusable UI; `composables/` owns reusable stateful behavior; `types/` and `utils/` are preferred for common types and pure functions; `styles/` owns shared styling.
- Feature-specific architecture and IPC invariants are indexed in `docs/experience/architecture.md`; UI failure modes and validation are in `docs/experience/ui-and-styling.md`.
- New tools use the existing entry -> registry -> panel -> optional bridge/backend path; do not add a bypass protocol.
