# Changelog

All notable changes to this project are documented in this file.

## [0.2.0] - 2026-03-10

### Added

- feat(todo): 新增待办工具（多提醒、重复、筛选、提醒弹窗）
- feat(maven): 新增 Maven 仓库搜索工具
- feat(diff): 添加左右内容交换按钮
- feat(hosts): 增加只读编辑切换
- feat(vault): 完成锁定流程优化

### Fixed

- fix(vault): 修复名称列链接与垂直居中
- fix(desktop): 双屏场景下唤起窗口跟随鼠标屏幕
- fix(build): 修复 Git link.exe 遮蔽 MSVC 链接器

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
