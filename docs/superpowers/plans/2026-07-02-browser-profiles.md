# Browser Profiles Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 新增 LazyCat「浏览器身份」工具，首版只发现、展示并启动本机 Microsoft Edge Profile，并接入 Spotlight 快速启动。

**Architecture:** 后端新增 `browser_profiles` 常规 tool domain，运行时扫描 Edge 可执行文件、Edge User Data 与 `Local State`，再叠加 `user_settings.browser_profiles_config_v1` 中的别名、隐藏和使用统计。前端把展示名、排序、分组、Spotlight 字段和权重都收口到纯函数，`BrowserProfilesPanel.vue` 只负责状态编排和 UI。启动链路不复用 Launcher 的 `split_whitespace` 参数路径，必须把 `--profile-directory=Profile 2` 作为一个完整 `Command` 参数传入。

**Tech Stack:** Tauri 2, Rust, rusqlite, serde_json, Vue 3, TypeScript, Vitest, Element Plus, Spotlight provider registry.

---

## Scope And Current State

设计文档：`docs/superpowers/specs/2026-07-02-browser-profiles-design.md`

已确认背景：

- 设计阶段已完成，提交为 `44ab460 docs(browser-profiles): 添加浏览器身份启动器设计`。
- 首版只支持 Edge Profile。
- 不自动登录、不绑定 URL、不读取 Cookie / Token / 密码 / 历史 / 收藏夹。
- Profile 稳定 key 使用目录名，例如 `Default`、`Profile 2`。
- Edge 显示名只读取 `Local State` 的 `profile.info_cache.<profileDir>.name`。
- 用户配置保存到 `user_settings` 的 `browser_profiles_config_v1`。
- `launch` 必须独立实现，不复用 `apps/desktop/src-tauri/src/tools/launcher.rs` 的 `arguments.split_whitespace()`。
- 默认排序为未隐藏优先、`launchCount DESC`、`lastLaunchedAt DESC`、展示名、目录名。
- Spotlight 只展示未隐藏 Profile，并用 `launchCount` 提升权重。

现有代码形状：

- 工具入口集中在 `apps/desktop/src/composables/toolCatalog.ts`。
- 面板注册集中在 `apps/desktop/src/tool-registry.ts`。
- IPC channel 映射集中在 `apps/desktop/src/bridge/tauri.ts`。
- Rust domain 分发集中在 `apps/desktop/src-tauri/src/tools/mod.rs`。
- Launcher 后端会把 `arguments` 按空白拆分，因此浏览器身份必须使用独立模块。
- Spotlight provider id 是 `apps/desktop/src/spotlight/types.ts` 里的联合类型，新增 provider 必须同步更新类型、运行时导入和 `config-store.test.ts` 的注册导入。

不做：

- 不新增数据库业务表。
- 不改造 Launcher 参数模型。
- 不新增 Chrome / Firefox 支持。
- 不新增 URL 绑定、置顶、快捷键或 Profile 创建/删除/重命名能力。
- 不在单元测试中真正启动 Edge。

---

## File Structure

新增：

- `apps/desktop/src-tauri/src/tools/browser_profiles.rs`
- `apps/desktop/src/components/BrowserProfilesPanel.vue`
- `apps/desktop/src/types/browser-profiles.ts`
- `apps/desktop/src/utils/browserProfiles.ts`
- `apps/desktop/src/utils/browserProfiles.test.ts`
- `apps/desktop/src/spotlight/providers/browser-profiles.ts`
- `apps/desktop/src/spotlight/providers/browser-profiles.test.ts`

修改：

- `apps/desktop/src-tauri/src/tools/mod.rs`
- `apps/desktop/src/composables/toolCatalog.ts`
- `apps/desktop/src/tool-registry.ts`
- `apps/desktop/src/bridge/tauri.ts`
- `apps/desktop/src/types/index.ts`
- `apps/desktop/src/spotlight/types.ts`
- `apps/desktop/src/components/SpotlightPanel.vue`
- `apps/desktop/src/components/settings/SpotlightSettings.vue`
- `apps/desktop/src/spotlight/config-store.test.ts`
- `process.md`（实现完成后记录经验）

---

## Public Contracts

### IPC Channels

Add these mappings to `apps/desktop/src/bridge/tauri.ts`:

```ts
"tool:browser-profiles:list": { domain: "browser_profiles", action: "list" },
"tool:browser-profiles:save-alias": { domain: "browser_profiles", action: "save_alias" },
"tool:browser-profiles:set-hidden": { domain: "browser_profiles", action: "set_hidden" },
"tool:browser-profiles:set-edge-path": { domain: "browser_profiles", action: "set_edge_path" },
"tool:browser-profiles:launch": { domain: "browser_profiles", action: "launch" },
```

### Frontend Types

Create `apps/desktop/src/types/browser-profiles.ts`:

```ts
export type BrowserProfileBrowser = "edge";

export interface BrowserProfileItem {
  browser: BrowserProfileBrowser;
  profileDir: string;
  edgeDisplayName: string;
  alias: string;
  hidden: boolean;
  launchCount: number;
  lastLaunchedAt: string | null;
}

export interface BrowserProfilesListResponse {
  edgeFound: boolean;
  edgePath: string | null;
  userDataDir: string;
  probedEdgePaths: string[];
  warnings: string[];
  profiles: BrowserProfileItem[];
}

export interface BrowserProfilesLaunchResponse {
  ok: true;
  launchCount: number;
  lastLaunchedAt: string;
  warnings: string[];
}
```

### Backend Config Shape

Store under `user_settings.key = "browser_profiles_config_v1"`:

```json
{
  "edgePath": "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "edge": {
    "Default": {
      "alias": "管理员",
      "hidden": false,
      "launchCount": 12,
      "lastLaunchedAt": "2026-07-02T10:30:00+08:00"
    }
  }
}
```

Implementation requirement: preserve unknown JSON fields with `#[serde(flatten)]` on config structs so future keys are not lost during read-modify-write.

---

## Task 1: Backend Pure Red Tests

**Files:**

- Create: `apps/desktop/src-tauri/src/tools/browser_profiles.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Test: `apps/desktop/src-tauri/src/tools/browser_profiles.rs`

**Interfaces:**

- Produces expected pure APIs for Task 2:
  - `parse_local_state_profile_names(content: &str) -> Result<(BTreeMap<String, String>, Vec<String>), String>`
  - `is_edge_profile_dir_name(name: &str) -> bool`
  - `collect_profile_dirs_from_names(names: &[&str]) -> Vec<String>`
  - `merge_profiles(discovered, config) -> Vec<BrowserProfileItem>`
  - `sort_profiles(&mut [BrowserProfileItem])`
  - `build_edge_profile_arg(profile_dir: &str) -> String`
  - `validate_edge_exe_path(path: &Path) -> Result<(), String>`
  - `parse_config_json(raw: Option<&str>, warnings: &mut Vec<String>) -> BrowserProfilesConfig`

- [ ] **Step 1: Register the module**

In `apps/desktop/src-tauri/src/tools/mod.rs`, add:

```rust
pub mod browser_profiles;
```

Do not add the dispatch arm yet; this task only makes Rust compile the module tests.

- [ ] **Step 2: Write failing tests**

Create `apps/desktop/src-tauri/src/tools/browser_profiles.rs` with only enough type skeletons and tests to express behavior. Tests should cover:

```rust
#[test]
fn parses_profile_names_from_local_state_info_cache() {
    let content = r#"{
      "profile": {
        "info_cache": {
          "Default": { "name": "个人" },
          "Profile 2": { "name": "测试账号" },
          "Guest Profile": { "name": "访客" }
        }
      }
    }"#;

    let (names, warnings) = parse_local_state_profile_names(content).expect("parse");

    assert!(warnings.is_empty());
    assert_eq!(names.get("Default").map(String::as_str), Some("个人"));
    assert_eq!(names.get("Profile 2").map(String::as_str), Some("测试账号"));
    assert_eq!(names.get("Guest Profile").map(String::as_str), Some("访客"));
}

#[test]
fn invalid_local_state_returns_warning_and_empty_names() {
    let (names, warnings) = parse_local_state_profile_names("{not json").expect("soft parse");
    assert!(names.is_empty());
    assert!(warnings.iter().any(|w| w.contains("Local State")));
}

#[test]
fn filters_default_and_profile_number_directories_only() {
    let names = ["Default", "Profile 1", "Profile 22", "Guest Profile", "System Profile", "Profile abc"];
    assert_eq!(
        collect_profile_dirs_from_names(&names),
        vec!["Default", "Profile 1", "Profile 22"]
    );
}

#[test]
fn merges_discovered_profiles_with_user_config_without_showing_deleted_profiles() {
    let discovered = vec![
        DiscoveredProfile { profile_dir: "Default".into(), edge_display_name: "个人".into() },
        DiscoveredProfile { profile_dir: "Profile 2".into(), edge_display_name: "测试账号".into() },
    ];
    let mut config = BrowserProfilesConfig::default();
    config.edge.insert("Default".into(), BrowserProfileConfigEntry {
        alias: Some("管理员".into()),
        hidden: Some(false),
        launch_count: Some(12),
        last_launched_at: Some("2026-07-02T10:30:00+08:00".into()),
        extra: Default::default(),
    });
    config.edge.insert("Deleted".into(), BrowserProfileConfigEntry {
        alias: Some("旧账号".into()),
        hidden: Some(false),
        launch_count: Some(99),
        last_launched_at: None,
        extra: Default::default(),
    });

    let merged = merge_profiles(discovered, &config);

    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].profile_dir, "Default");
    assert_eq!(merged[0].alias, "管理员");
    assert_eq!(merged[0].launch_count, 12);
    assert!(!merged.iter().any(|p| p.profile_dir == "Deleted"));
}

#[test]
fn sorts_by_hidden_launch_count_last_launched_display_name_and_dir() {
    let mut items = vec![
        item("Profile 3", "", "Beta", false, 2, Some("2026-07-02T09:00:00+08:00")),
        item("Profile 2", "管理员", "Zeta", false, 3, Some("2026-07-01T09:00:00+08:00")),
        item("Default", "", "Alpha", false, 3, Some("2026-07-02T09:00:00+08:00")),
        item("Profile 4", "", "Hidden", true, 99, Some("2026-07-03T09:00:00+08:00")),
    ];

    sort_profiles(&mut items);

    assert_eq!(
        items.iter().map(|p| p.profile_dir.as_str()).collect::<Vec<_>>(),
        vec!["Default", "Profile 2", "Profile 3", "Profile 4"]
    );
}

#[test]
fn builds_profile_directory_as_single_command_argument() {
    assert_eq!(
        build_edge_profile_arg("Profile 2"),
        "--profile-directory=Profile 2"
    );
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test browser_profiles -- --nocapture`

Expected: FAIL with missing function/type errors. This proves the tests are wired into the Rust module tree.

- [ ] **Step 4: Commit**

```powershell
git add apps/desktop/src-tauri/src/tools/mod.rs apps/desktop/src-tauri/src/tools/browser_profiles.rs
git commit -m "test(browser-profiles): 添加后端纯函数红测"
```

---

## Task 2: Backend Pure Implementation

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/browser_profiles.rs`
- Test: `apps/desktop/src-tauri/src/tools/browser_profiles.rs`

**Interfaces:**

- Consumes tests from Task 1.
- Produces pure helpers for scanning, merging, sorting, config parsing, path validation, and launch arg construction.

- [ ] **Step 1: Implement data structs**

In `browser_profiles.rs`, add serde structs using camelCase output:

```rust
const CONFIG_KEY: &str = "browser_profiles_config_v1";
const BROWSER_EDGE: &str = "edge";

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserProfilesConfig {
    #[serde(default)]
    edge_path: Option<String>,
    #[serde(default)]
    edge: std::collections::BTreeMap<String, BrowserProfileConfigEntry>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserProfileConfigEntry {
    #[serde(default)]
    alias: Option<String>,
    #[serde(default)]
    hidden: Option<bool>,
    #[serde(default, rename = "launchCount")]
    launch_count: Option<i64>,
    #[serde(default, rename = "lastLaunchedAt")]
    last_launched_at: Option<String>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DiscoveredProfile {
    profile_dir: String,
    edge_display_name: String,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct BrowserProfileItem {
    browser: String,
    profile_dir: String,
    edge_display_name: String,
    alias: String,
    hidden: bool,
    launch_count: i64,
    last_launched_at: Option<String>,
}
```

- [ ] **Step 2: Implement Local State parsing**

Rules:

- Parse JSON with `serde_json::from_str::<Value>()`.
- On malformed JSON, return `Ok((BTreeMap::new(), vec![...]))`; do not fail the scan.
- Read only `profile.info_cache`.
- Use `name` only when it is a non-empty string after trim.
- Do not read cookies, tokens, passwords, history, bookmarks, extension state, or any profile subdatabase.

- [ ] **Step 3: Implement profile dir filtering**

Rules:

- Accept `Default`.
- Accept `Profile ` followed by at least one ASCII digit.
- Reject `Guest Profile`, `System Profile`, `Profile abc`, empty names.
- Sort result as `Default` first, then numeric `Profile N` ascending, then lexical fallback.

- [ ] **Step 4: Implement config parsing and preserving unknown fields**

Rules:

- `parse_config_json(None, warnings)` returns default config.
- Malformed config returns default config and pushes a warning containing `browser_profiles_config_v1`.
- Unknown fields must be preserved when parsing succeeds.
- Saving later must not drop config-level or entry-level `extra` fields.

- [ ] **Step 5: Implement merge and sort**

Rules:

- Discovered scan result is the fact source.
- Config only overlays `alias`, `hidden`, `launchCount`, `lastLaunchedAt`.
- Missing config values default to `""`, `false`, `0`, `None`.
- Do not include config entries for deleted Edge profiles.
- Display name for sorting should use `alias > edgeDisplayName > profileDir`, case-insensitive.
- Sort hidden profiles after visible profiles.

- [ ] **Step 6: Implement path validation and launch arg helper**

Rules:

- `validate_edge_exe_path` requires `path.exists()`, `path.is_file()`, and file name equals `msedge.exe` case-insensitively.
- `build_edge_profile_arg("Profile 2")` must return exactly `--profile-directory=Profile 2`.

- [ ] **Step 7: Run backend pure tests**

Run: `cargo test browser_profiles -- --nocapture`

Expected: PASS for the tests added in Task 1.

- [ ] **Step 8: Commit**

```powershell
git add apps/desktop/src-tauri/src/tools/browser_profiles.rs
git commit -m "feat(browser-profiles): 实现后端扫描纯函数"
```

---

## Task 3: Backend IPC Actions, User Settings, And Launch

**Files:**

- Modify: `apps/desktop/src-tauri/src/tools/browser_profiles.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`
- Test: `apps/desktop/src-tauri/src/tools/browser_profiles.rs`

**Interfaces:**

- Consumes pure helpers from Task 2.
- Produces `browser_profiles::execute(action, payload)` with actions:
  - `list`
  - `save_alias`
  - `set_hidden`
  - `set_edge_path`
  - `launch`

- [ ] **Step 1: Add failing action tests without spawning Edge**

Add tests for pure payload validation and dispatch edge cases:

```rust
#[test]
fn rejects_non_edge_browser_payload() {
    let err = require_edge_browser(&serde_json::json!({ "browser": "chrome" }))
        .expect_err("chrome unsupported");
    assert!(err.contains("只支持 Edge") || err.contains("edge"));
}

#[test]
fn trims_alias_before_writing_config_entry() {
    let mut config = BrowserProfilesConfig::default();
    save_alias_in_config(&mut config, "Profile 2", "  普通用户  ");
    assert_eq!(
        config.edge.get("Profile 2").and_then(|e| e.alias.as_deref()),
        Some("普通用户")
    );
}

#[test]
fn empty_alias_clears_alias_but_preserves_entry_stats() {
    let mut config = BrowserProfilesConfig::default();
    config.edge.insert("Profile 2".into(), BrowserProfileConfigEntry {
        alias: Some("旧名".into()),
        launch_count: Some(7),
        ..Default::default()
    });

    save_alias_in_config(&mut config, "Profile 2", " ");

    let entry = config.edge.get("Profile 2").expect("entry");
    assert_eq!(entry.alias.as_deref(), Some(""));
    assert_eq!(entry.launch_count, Some(7));
}

#[test]
fn launch_stats_increment_preserves_alias_and_hidden() {
    let mut config = BrowserProfilesConfig::default();
    config.edge.insert("Profile 2".into(), BrowserProfileConfigEntry {
        alias: Some("普通用户".into()),
        hidden: Some(true),
        launch_count: Some(8),
        last_launched_at: Some("2026-07-01T09:00:00+08:00".into()),
        extra: Default::default(),
    });

    update_launch_stats_in_config(&mut config, "Profile 2", "2026-07-02T10:30:00+08:00");

    let entry = config.edge.get("Profile 2").expect("entry");
    assert_eq!(entry.alias.as_deref(), Some("普通用户"));
    assert_eq!(entry.hidden, Some(true));
    assert_eq!(entry.launch_count, Some(9));
    assert_eq!(entry.last_launched_at.as_deref(), Some("2026-07-02T10:30:00+08:00"));
}
```

Run: `cargo test browser_profiles -- --nocapture`

Expected: FAIL until helper/action code exists.

- [ ] **Step 2: Add dispatch arm**

In `apps/desktop/src-tauri/src/tools/mod.rs`, add:

```rust
"browser_profiles" => browser_profiles::execute(action, payload),
```

Keep `pm_or_todo_data_changed` unchanged; browser profile launch should not notify widget refresh.

- [ ] **Step 3: Implement Edge discovery**

In `browser_profiles.rs`, implement:

- `candidate_edge_paths(config_edge_path: Option<&str>) -> Vec<PathBuf>`
- `find_edge_path(config_edge_path: Option<&str>) -> (Option<PathBuf>, Vec<PathBuf>)`
- `edge_user_data_dir() -> PathBuf`
- `scan_edge_profiles(user_data_dir: &Path) -> (Vec<DiscoveredProfile>, Vec<String>)`

Rules:

- Candidate order:
  1. configured `edgePath`
  2. `%ProgramFiles(x86)%\Microsoft\Edge\Application\msedge.exe`
  3. `%ProgramFiles%\Microsoft\Edge\Application\msedge.exe`
  4. `%LOCALAPPDATA%\Microsoft\Edge\Application\msedge.exe`
- User data dir is `%LOCALAPPDATA%\Microsoft\Edge\User Data`.
- If `Local State` is missing or malformed, return warning and fall back to profile directories.
- Directory discovery should combine Local State keys and actual directories, then filter through `is_edge_profile_dir_name`.

- [ ] **Step 4: Implement user_settings read/write**

Use `super::helpers::db_conn` and `rusqlite::params`.

Implement:

```rust
fn load_config_from_settings(conn: &rusqlite::Connection, warnings: &mut Vec<String>) -> BrowserProfilesConfig
fn save_config_to_settings(conn: &rusqlite::Connection, config: &BrowserProfilesConfig) -> Result<(), String>
fn mutate_config<F>(f: F) -> Result<BrowserProfilesConfig, String>
where
    F: FnOnce(&mut BrowserProfilesConfig) -> Result<(), String>
```

Rules:

- Read-modify-write must happen in one operation using one connection.
- Save with the existing `INSERT ... ON CONFLICT(key) DO UPDATE SET value=excluded.value, updated_at=CURRENT_TIMESTAMP` pattern from `settings.rs`.
- Malformed config may downgrade to default with warning on read; save then writes a valid v1 structure.

- [ ] **Step 5: Implement `list` action**

Response shape:

```rust
Ok(json!({
    "edgeFound": edge_path.is_some(),
    "edgePath": edge_path.map(|p| p.to_string_lossy().to_string()),
    "userDataDir": user_data_dir.to_string_lossy().to_string(),
    "probedEdgePaths": probed_paths_as_strings,
    "warnings": warnings,
    "profiles": profiles,
}))
```

Rules:

- `list` always rescans local state and directories.
- It should still return profiles if Edge executable is missing but User Data exists.
- It should return empty profiles if User Data does not exist, with explicit path in `userDataDir`.

- [ ] **Step 6: Implement `save_alias`, `set_hidden`, `set_edge_path`**

Rules:

- `browser` must be `"edge"` for profile actions.
- `profileDir` must exist in the current scan before saving alias/hidden.
- Alias is stored after `.trim()`. Empty string means clear alias.
- Hidden stores explicit boolean.
- `set_edge_path` validates file exists and file name is `msedge.exe`.

- [ ] **Step 7: Implement `launch` without split_whitespace**

Implementation shape:

```rust
fn launch_profile(payload: &Value) -> Result<Value, String> {
    let profile_dir = require_profile_dir(payload)?;
    let mut warnings = Vec::new();
    let config = {
        let conn = db_conn()?;
        load_config_from_settings(&conn, &mut warnings)
    };
    let (edge_path, _) = find_edge_path(config.edge_path.as_deref());
    let edge_path = edge_path.ok_or("未找到 msedge.exe")?;
    ensure_profile_exists(&profile_dir)?;

    std::process::Command::new(&edge_path)
        .arg(build_edge_profile_arg(&profile_dir))
        .spawn()
        .map_err(|e| format!("launch failed: {e}"))?;

    let now = chrono::Local::now().to_rfc3339();
    let stats_result = mutate_config(|cfg| {
        update_launch_stats_in_config(cfg, &profile_dir, &now);
        Ok(())
    });

    if let Err(err) = stats_result {
        warnings.push(format!("启动成功，但使用统计保存失败：{err}"));
    }

    let launch_count = latest_launch_count_after_update(...);
    Ok(json!({
        "ok": true,
        "launchCount": launch_count,
        "lastLaunchedAt": now,
        "warnings": warnings,
    }))
}
```

Required behavior:

- Only update stats after `spawn()` returns `Ok`.
- If `spawn()` fails, return error and do not update stats.
- If stats write fails after successful spawn, return `ok: true` plus warning.
- Do not wait for Edge to exit.
- Do not attempt to inspect or focus Edge windows.

- [ ] **Step 8: Run backend tests**

Run: `cargo test browser_profiles -- --nocapture`

Expected: PASS.

- [ ] **Step 9: Commit**

```powershell
git add apps/desktop/src-tauri/src/tools/browser_profiles.rs apps/desktop/src-tauri/src/tools/mod.rs
git commit -m "feat(browser-profiles): 添加后端 IPC 与启动能力"
```

---

## Task 4: Frontend Types And Pure Red Tests

**Files:**

- Create: `apps/desktop/src/types/browser-profiles.ts`
- Create: `apps/desktop/src/utils/browserProfiles.test.ts`
- Test: `apps/desktop/src/utils/browserProfiles.test.ts`

**Interfaces:**

- Produces expected pure APIs for Task 5:
  - `getBrowserProfileDisplayName(profile)`
  - `sortBrowserProfiles(profiles)`
  - `splitBrowserProfilesByHidden(profiles)`
  - `formatBrowserProfileLastLaunchedAt(value)`
  - `buildBrowserProfileSearchFields(profile)`
  - `getBrowserProfileSpotlightWeight(profile)`

- [ ] **Step 1: Add frontend type file**

Create the type file exactly as shown in the Public Contracts section.

- [ ] **Step 2: Write failing utility tests**

Create `apps/desktop/src/utils/browserProfiles.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  buildBrowserProfileSearchFields,
  formatBrowserProfileLastLaunchedAt,
  getBrowserProfileDisplayName,
  getBrowserProfileSpotlightWeight,
  sortBrowserProfiles,
  splitBrowserProfilesByHidden,
} from "./browserProfiles";
import type { BrowserProfileItem } from "../types/browser-profiles";

function profile(overrides: Partial<BrowserProfileItem>): BrowserProfileItem {
  return {
    browser: "edge",
    profileDir: "Default",
    edgeDisplayName: "",
    alias: "",
    hidden: false,
    launchCount: 0,
    lastLaunchedAt: null,
    ...overrides,
  };
}

describe("browserProfiles utils", () => {
  it("uses alias, then Edge display name, then profile dir as display name", () => {
    expect(getBrowserProfileDisplayName(profile({ alias: "管理员", edgeDisplayName: "个人" }))).toBe("管理员");
    expect(getBrowserProfileDisplayName(profile({ alias: "", edgeDisplayName: "个人" }))).toBe("个人");
    expect(getBrowserProfileDisplayName(profile({ alias: "", edgeDisplayName: "", profileDir: "Profile 2" }))).toBe("Profile 2");
  });

  it("sorts visible profiles before hidden profiles by usage and display name", () => {
    const sorted = sortBrowserProfiles([
      profile({ profileDir: "Profile 4", edgeDisplayName: "Hidden", hidden: true, launchCount: 99 }),
      profile({ profileDir: "Profile 2", alias: "管理员", launchCount: 3, lastLaunchedAt: "2026-07-01T09:00:00+08:00" }),
      profile({ profileDir: "Default", edgeDisplayName: "Alpha", launchCount: 3, lastLaunchedAt: "2026-07-02T09:00:00+08:00" }),
      profile({ profileDir: "Profile 3", edgeDisplayName: "Beta", launchCount: 2 }),
    ]);

    expect(sorted.map((item) => item.profileDir)).toEqual(["Default", "Profile 2", "Profile 3", "Profile 4"]);
  });

  it("splits visible and hidden profiles without mutating input", () => {
    const input = [
      profile({ profileDir: "Default" }),
      profile({ profileDir: "Profile 2", hidden: true }),
    ];
    const grouped = splitBrowserProfilesByHidden(input);

    expect(grouped.visible.map((item) => item.profileDir)).toEqual(["Default"]);
    expect(grouped.hidden.map((item) => item.profileDir)).toEqual(["Profile 2"]);
    expect(input[0].hidden).toBe(false);
  });

  it("builds Spotlight search fields from alias, display name and profile dir", () => {
    const fields = buildBrowserProfileSearchFields(profile({
      alias: "管理员",
      edgeDisplayName: "测试账号",
      profileDir: "Profile 2",
    }));

    expect(fields.map((field) => field.text)).toEqual(["管理员", "测试账号", "Profile 2"]);
    expect(fields[0].weight).toBeGreaterThan(fields[1].weight);
  });

  it("increases Spotlight item weight with launch count but caps growth", () => {
    expect(getBrowserProfileSpotlightWeight(profile({ launchCount: 0 }))).toBeGreaterThan(1);
    expect(getBrowserProfileSpotlightWeight(profile({ launchCount: 30 }))).toBeGreaterThan(
      getBrowserProfileSpotlightWeight(profile({ launchCount: 1 })),
    );
    expect(getBrowserProfileSpotlightWeight(profile({ launchCount: 999 }))).toBe(
      getBrowserProfileSpotlightWeight(profile({ launchCount: 50 })),
    );
  });

  it("formats empty last launched time as never used", () => {
    expect(formatBrowserProfileLastLaunchedAt(null)).toBe("未启动过");
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `pnpm test src/utils/browserProfiles.test.ts`

Expected: FAIL because `./browserProfiles` does not exist.

- [ ] **Step 4: Commit**

```powershell
git add apps/desktop/src/types/browser-profiles.ts apps/desktop/src/utils/browserProfiles.test.ts
git commit -m "test(browser-profiles): 添加前端纯函数红测"
```

---

## Task 5: Frontend Pure Implementation

**Files:**

- Create: `apps/desktop/src/utils/browserProfiles.ts`
- Modify: `apps/desktop/src/types/index.ts`
- Test: `apps/desktop/src/utils/browserProfiles.test.ts`

**Interfaces:**

- Consumes tests from Task 4.
- Produces reusable browser profile utilities for panel and Spotlight provider.

- [ ] **Step 1: Implement `browserProfiles.ts`**

Implementation rules:

- `getBrowserProfileDisplayName` returns first non-empty trimmed value from `alias`, `edgeDisplayName`, `profileDir`.
- `sortBrowserProfiles` returns a new array and does not mutate input.
- Date comparison should treat invalid or missing `lastLaunchedAt` as older than valid dates.
- Display name comparison should use `localeCompare(..., "zh-CN", { sensitivity: "base" })`.
- `buildBrowserProfileSearchFields` must include only non-empty unique fields in this order:
  1. alias weight `1.4`
  2. Edge display name weight `1.0`
  3. profile dir weight `0.8`
- `getBrowserProfileSpotlightWeight` should start above ordinary tool entries and cap launch count:

```ts
export function getBrowserProfileSpotlightWeight(profile: BrowserProfileItem): number {
  return 1.08 + Math.min(Math.max(profile.launchCount, 0), 50) * 0.012;
}
```

- [ ] **Step 2: Export types from index**

In `apps/desktop/src/types/index.ts`, add:

```ts
export type {
  BrowserProfileBrowser,
  BrowserProfileItem,
  BrowserProfilesListResponse,
  BrowserProfilesLaunchResponse,
} from "./browser-profiles";
```

- [ ] **Step 3: Run frontend utility tests**

Run: `pnpm test src/utils/browserProfiles.test.ts`

Expected: PASS.

- [ ] **Step 4: Commit**

```powershell
git add apps/desktop/src/utils/browserProfiles.ts apps/desktop/src/types/index.ts apps/desktop/src/utils/browserProfiles.test.ts
git commit -m "feat(browser-profiles): 实现前端展示与排序工具"
```

---

## Task 6: BrowserProfilesPanel UI

**Files:**

- Create: `apps/desktop/src/components/BrowserProfilesPanel.vue`
- Test indirectly: `pnpm typecheck`

**Interfaces:**

- Consumes:
  - `tool:browser-profiles:list`
  - `tool:browser-profiles:save-alias`
  - `tool:browser-profiles:set-hidden`
  - `tool:browser-profiles:set-edge-path`
  - `tool:browser-profiles:launch`
  - `BrowserProfilesListResponse`
  - `browserProfiles.ts` utilities
- Produces usable panel UI for manual testing and later tool registration.

- [ ] **Step 1: Create panel state and load flow**

Use `script setup` with:

```ts
const loading = ref(false);
const launchingKey = ref("");
const response = ref<BrowserProfilesListResponse | null>(null);
const errorMessage = ref("");
const hiddenExpanded = ref(false);
let requestSeq = 0;

async function loadProfiles() {
  const seq = ++requestSeq;
  loading.value = true;
  errorMessage.value = "";
  try {
    const result = await invokeToolByChannel("tool:browser-profiles:list", {}) as BrowserProfilesListResponse;
    if (seq !== requestSeq) return;
    response.value = result;
  } catch (err) {
    if (seq !== requestSeq) return;
    errorMessage.value = err instanceof Error ? err.message : String(err);
  } finally {
    if (seq === requestSeq) loading.value = false;
  }
}
```

Call `loadProfiles()` in `onMounted`.

- [ ] **Step 2: Create derived lists**

Use pure functions:

```ts
const sortedProfiles = computed(() => sortBrowserProfiles(response.value?.profiles ?? []));
const groupedProfiles = computed(() => splitBrowserProfilesByHidden(sortedProfiles.value));
```

- [ ] **Step 3: Implement actions**

Implement:

- `launchProfile(profile)`:
  - call `tool:browser-profiles:launch` with `{ browser: "edge", profileDir }`
  - show `ElMessage.success("已打开 Edge：<displayName>")`
  - reload profiles after success so launch count and sort update
- `editAlias(profile)`:
  - use `ElMessageBox.prompt`
  - input default is current `alias`
  - submit trimmed alias to `save-alias`
  - reload after save
- `setHidden(profile, hidden)`:
  - call `set-hidden`
  - reload after success
- `chooseEdgePath()`:
  - use `open` from `@tauri-apps/plugin-dialog`
  - filter `exe`
  - submit selected path to `set-edge-path`
  - reload after success

Do not add create/delete/rename Profile actions.

- [ ] **Step 4: Implement template**

Structure:

- Root `.browser-profiles-panel`.
- Top toolbar/status:
  - title `浏览器身份`
  - Edge found status
  - visible count / hidden count
  - refresh button
  - `选择 msedge.exe` button when not found, and also small secondary action when found
- Warning area:
  - show `response.warnings`
  - show `errorMessage`
  - if `edgeFound === false`, show probed paths in a compact list
- Main list:
  - visible profiles only
  - each row shows display name, Edge display name, profile dir, launch count, last launched time
  - actions: `启动`, `别名`, `隐藏`
- Hidden group:
  - collapsed by default
  - actions: `恢复`, `别名`
- Empty states:
  - User Data missing or no profiles: `未发现 Edge Profile`
  - Edge path missing but profiles found: allow list display but launch will fail until path configured

Keep layout utilitarian and light; do not create a landing page.

- [ ] **Step 5: Implement scoped styles**

Use existing CSS tokens:

- `var(--lc-surface-0)`
- `var(--lc-surface-1)`
- `var(--lc-border-subtle)`
- `var(--lc-text)`
- `var(--lc-text-secondary)`
- `var(--lc-accent)`

Avoid nested cards. Use rows and bands. Keep button text compact and prevent overflow.

- [ ] **Step 6: Run typecheck**

Run: `pnpm typecheck`

Expected: it may fail because IPC channels and registry are not wired yet only if panel imports unresolved files. It should not fail due to the panel itself.

- [ ] **Step 7: Commit**

```powershell
git add apps/desktop/src/components/BrowserProfilesPanel.vue
git commit -m "feat(browser-profiles): 添加浏览器身份面板"
```

---

## Task 7: Tool Entry, Panel Registry, And IPC Channel Registration

**Files:**

- Modify: `apps/desktop/src/composables/toolCatalog.ts`
- Modify: `apps/desktop/src/tool-registry.ts`
- Modify: `apps/desktop/src/bridge/tauri.ts`
- Test: typecheck and build web

**Interfaces:**

- Consumes panel and backend domain from previous tasks.
- Produces LazyCat tool entry `browser-profiles`.

- [ ] **Step 1: Add sidebar tool entry**

In `apps/desktop/src/composables/toolCatalog.ts`, add under the `更多工具` group near `launcher`:

```ts
{ id: "browser-profiles", name: "浏览器身份", desc: "一键启动 Edge 用户身份窗口" },
```

Reason: this is a launcher-style productivity tool, not a network protocol tool.

- [ ] **Step 2: Register async component**

In `apps/desktop/src/tool-registry.ts`, add:

```ts
"browser-profiles": defineAsyncComponent(() => import("./components/BrowserProfilesPanel.vue")),
```

- [ ] **Step 3: Add IPC channel mappings**

In `apps/desktop/src/bridge/tauri.ts`, add the five mappings from Public Contracts near the `tool:launcher:*` group.

- [ ] **Step 4: Run validation**

Run:

```powershell
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: both PASS.

- [ ] **Step 5: Commit**

```powershell
git add apps/desktop/src/composables/toolCatalog.ts apps/desktop/src/tool-registry.ts apps/desktop/src/bridge/tauri.ts
git commit -m "feat(browser-profiles): 注册工具入口和 IPC 通道"
```

---

## Task 8: Spotlight Provider Red Tests

**Files:**

- Create: `apps/desktop/src/spotlight/providers/browser-profiles.test.ts`
- Test: `apps/desktop/src/spotlight/providers/browser-profiles.test.ts`

**Interfaces:**

- Produces expected APIs for Task 9:
  - `buildBrowserProfileSpotlightItem(profile)`
  - `browserProfilesProvider`

- [ ] **Step 1: Write failing provider tests**

Create test file:

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeToolByChannel = vi.fn();
const invoke = vi.fn();

vi.mock("../../bridge/tauri", () => ({
  invokeToolByChannel: (...args: unknown[]) => invokeToolByChannel(...args),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invoke(...args),
}));

import {
  browserProfilesProvider,
  buildBrowserProfileSpotlightItem,
} from "./browser-profiles";
import type { BrowserProfileItem } from "../../types/browser-profiles";

function profile(overrides: Partial<BrowserProfileItem>): BrowserProfileItem {
  return {
    browser: "edge",
    profileDir: "Default",
    edgeDisplayName: "个人",
    alias: "",
    hidden: false,
    launchCount: 0,
    lastLaunchedAt: null,
    ...overrides,
  };
}

beforeEach(() => {
  invokeToolByChannel.mockReset();
  invoke.mockReset();
});

describe("buildBrowserProfileSpotlightItem", () => {
  it("maps alias display name and searchable fields", () => {
    const item = buildBrowserProfileSpotlightItem(profile({
      profileDir: "Profile 2",
      alias: "管理员",
      edgeDisplayName: "测试账号",
      launchCount: 8,
    }));

    expect(item.providerId).toBe("browser-profiles");
    expect(item.itemId).toBe("edge:Profile 2");
    expect(item.title).toBe("管理员");
    expect(item.subtitle).toContain("测试账号");
    expect(item.subtitle).toContain("Profile 2");
    expect(item.badge).toEqual({ short: "Edge", tone: "primary" });
    expect(item.searchFields.map((field) => field.text)).toEqual(["管理员", "测试账号", "Profile 2"]);
    expect(item.weight).toBeGreaterThan(1.08);
  });
});

describe("browserProfilesProvider", () => {
  it("prefetches only visible profiles", async () => {
    invokeToolByChannel.mockResolvedValue({
      profiles: [
        profile({ profileDir: "Default" }),
        profile({ profileDir: "Profile 2", hidden: true }),
      ],
    });

    const items = await browserProfilesProvider.prefetch();

    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:browser-profiles:list", {});
    expect(items.map((item) => item.itemId)).toEqual(["edge:Default"]);
  });

  it("launches selected profile as default action", async () => {
    const item = buildBrowserProfileSpotlightItem(profile({
      profileDir: "Profile 2",
      alias: "管理员",
    }));

    const result = await browserProfilesProvider.defaultAction(item, {} as never);

    expect(invokeToolByChannel).toHaveBeenCalledWith("tool:browser-profiles:launch", {
      browser: "edge",
      profileDir: "Profile 2",
    });
    expect(result).toEqual({
      closeSpotlight: true,
      toast: { message: "已打开 Edge：管理员", type: "success" },
    });
  });

  it("opens browser profiles tool from action menu", async () => {
    const item = buildBrowserProfileSpotlightItem(profile({ profileDir: "Default" }));

    const result = await browserProfilesProvider.executeAction?.(item, "open_tool", {} as never);

    expect(invoke).toHaveBeenCalledWith("spotlight_pick", { target: "browser-profiles" });
    expect(result).toEqual({ closeSpotlight: true });
  });

  it("returns explicit error for malformed payload", async () => {
    const result = await browserProfilesProvider.defaultAction({
      providerId: "browser-profiles",
      itemId: "bad",
      title: "bad",
      searchFields: [],
      payload: {},
    }, {} as never);

    expect(result.errorMessage).toContain("无效");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm test src/spotlight/providers/browser-profiles.test.ts`

Expected: FAIL because provider file and provider id type do not exist.

- [ ] **Step 3: Commit**

```powershell
git add apps/desktop/src/spotlight/providers/browser-profiles.test.ts
git commit -m "test(browser-profiles): 添加 Spotlight provider 红测"
```

---

## Task 9: Spotlight Provider Implementation And Registration

**Files:**

- Create: `apps/desktop/src/spotlight/providers/browser-profiles.ts`
- Modify: `apps/desktop/src/spotlight/types.ts`
- Modify: `apps/desktop/src/components/SpotlightPanel.vue`
- Modify: `apps/desktop/src/components/settings/SpotlightSettings.vue`
- Modify: `apps/desktop/src/spotlight/config-store.test.ts`
- Test: `apps/desktop/src/spotlight/providers/browser-profiles.test.ts`
- Test: `apps/desktop/src/spotlight/config-store.test.ts`

**Interfaces:**

- Consumes `tool:browser-profiles:list` and `tool:browser-profiles:launch`.
- Produces registered Spotlight provider `browser-profiles`.

- [ ] **Step 1: Extend provider id type**

In `apps/desktop/src/spotlight/types.ts`, add:

```ts
| "browser-profiles"
```

to `SpotlightProviderId`.

- [ ] **Step 2: Implement provider**

Create `apps/desktop/src/spotlight/providers/browser-profiles.ts`.

Rules:

- `id: "browser-profiles"`
- `name: "浏览器身份"`
- `description: "启动 Edge 用户身份窗口"`
- `badgeShort: "Edge"`
- `badgeTone: "primary"`
- `weight: 1.02`
- `defaultAliases: []`
- `defaultEnabled: true`
- `prefetch` calls `tool:browser-profiles:list`
- `prefetch` returns only profiles where `hidden === false`
- No query-time `search`; profile list is small enough to prefetch.
- Default action launches the selected profile.
- Actions:
  - `launch` label `启动`
  - `open_tool` label `跳转到浏览器身份`

Implementation shape:

```ts
interface BrowserProfilePayload {
  browser: "edge";
  profileDir: string;
  displayName: string;
}

function payloadOf(item: SpotlightItem): BrowserProfilePayload | null {
  if (item.payload?.browser !== "edge") return null;
  if (typeof item.payload.profileDir !== "string") return null;
  if (typeof item.payload.displayName !== "string") return null;
  return {
    browser: "edge",
    profileDir: item.payload.profileDir,
    displayName: item.payload.displayName,
  };
}
```

- [ ] **Step 3: Reuse frontend utilities**

Use:

- `getBrowserProfileDisplayName`
- `buildBrowserProfileSearchFields`
- `getBrowserProfileSpotlightWeight`

Do not duplicate display-name or weight logic inside the provider.

- [ ] **Step 4: Register runtime imports**

In `apps/desktop/src/components/SpotlightPanel.vue`, add:

```ts
import "../spotlight/providers/browser-profiles";
```

near other provider imports.

In `apps/desktop/src/components/settings/SpotlightSettings.vue`, add:

```ts
import "../../spotlight/providers/browser-profiles";
```

near other provider imports so settings can show and toggle the provider.

In `apps/desktop/src/spotlight/config-store.test.ts`, add:

```ts
import "./providers/browser-profiles";
```

- [ ] **Step 5: Run provider and config tests**

Run:

```powershell
pnpm test src/spotlight/providers/browser-profiles.test.ts src/spotlight/config-store.test.ts
```

Expected: PASS.

- [ ] **Step 6: Run typecheck**

Run: `pnpm typecheck`

Expected: PASS.

- [ ] **Step 7: Commit**

```powershell
git add apps/desktop/src/spotlight/providers/browser-profiles.ts apps/desktop/src/spotlight/types.ts apps/desktop/src/components/SpotlightPanel.vue apps/desktop/src/components/settings/SpotlightSettings.vue apps/desktop/src/spotlight/config-store.test.ts apps/desktop/src/spotlight/providers/browser-profiles.test.ts
git commit -m "feat(browser-profiles): 接入 Spotlight 快速启动"
```

---

## Task 10: End-To-End Validation And Process Note

**Files:**

- Modify: `process.md`
- Validate: whole feature

**Interfaces:**

- Consumes completed implementation.
- Produces verification evidence and project process note.

- [ ] **Step 1: Run backend verification**

Run:

```powershell
cargo test browser_profiles -- --nocapture
```

Expected: PASS.

- [ ] **Step 2: Run frontend targeted tests**

Run:

```powershell
pnpm test src/utils/browserProfiles.test.ts src/spotlight/providers/browser-profiles.test.ts src/spotlight/config-store.test.ts
```

Expected: PASS.

- [ ] **Step 3: Run integration checks**

Run:

```powershell
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: PASS.

- [ ] **Step 4: Manual smoke in dev app only if user requests UI verification**

AGENTS says do not auto-start UI/dev server unless explicitly requested. If user asks for visual/manual verification, run:

```powershell
pnpm dev
```

Smoke checklist:

- Sidebar shows `浏览器身份`.
- Panel loads without Edge path errors crashing UI.
- `Default` and `Profile N` display if present under `%LOCALAPPDATA%\Microsoft\Edge\User Data`.
- Alias save updates visible display name.
- Hide moves a row to the hidden section; restore moves it back.
- Manual `msedge.exe` selection rejects non-Edge executables.
- Launch opens the correct profile and increments count.
- Spotlight can find by alias, Edge display name, or `Profile 2`.
- Spotlight launch shows success message and refreshes next prefetch with updated usage count.

- [ ] **Step 5: Record process note**

Add a new top entry to `process.md` after implementation:

```md
## 2026-07-02: 浏览器身份启动器避免复用通用参数拆分

**场景**: 新增 Edge Profile 启动器，按 Profile 目录名发现、展示、别名管理和 Spotlight 启动。
**使用次数**: 0
**问题**:
1. Launcher 的通用参数启动会用空白拆分，`--profile-directory=Profile 2` 会被拆坏。
2. Edge Profile 显示名可变且可能重复，不能作为稳定 key。
3. 面板和 Spotlight 都需要展示名、排序和权重，若各自实现会形成双重规则。
**解决**:
1. 后端独立 `browser_profiles` 模块启动 Edge，并把 profile 参数作为单个 `Command` arg。
2. 稳定 key 固定使用目录名，扫描结果为事实源，`user_settings` 只做覆盖层。
3. 前端抽 `browserProfiles.ts` 纯函数，面板和 Spotlight 共用展示名、排序和权重。
**涉及文件**:
- `apps/desktop/src-tauri/src/tools/browser_profiles.rs`
- `apps/desktop/src/components/BrowserProfilesPanel.vue`
- `apps/desktop/src/utils/browserProfiles.ts`
- `apps/desktop/src/spotlight/providers/browser-profiles.ts`
**验证**:
- `cargo test browser_profiles -- --nocapture`
- `pnpm test src/utils/browserProfiles.test.ts src/spotlight/providers/browser-profiles.test.ts src/spotlight/config-store.test.ts`
- `pnpm typecheck`
- `pnpm --filter @lazycat/desktop build:web`
```

- [ ] **Step 6: Inspect diff before final commit**

Run:

```powershell
git status --short
git diff --stat
git diff
```

Check:

- No unrelated files are staged or modified by this work.
- No code path reads Edge cookies, tokens, passwords, history, bookmarks, or profile databases.
- `browser_profiles.rs` launch path uses `.arg(format!("--profile-directory={profile_dir}"))`, not `.args(split_whitespace())`.
- `browser_profiles_config_v1` read-modify-write preserves unknown fields.
- `SpotlightProviderId` includes `browser-profiles`.
- Runtime imports include the provider in both Spotlight panel and settings.

- [ ] **Step 7: Commit final process note if all verification passes**

```powershell
git add process.md
git commit -m "docs(process): 记录浏览器身份启动器实现经验"
```

---

## Final Verification Commands

Before reporting complete, run:

```powershell
cargo test browser_profiles -- --nocapture
pnpm test src/utils/browserProfiles.test.ts src/spotlight/providers/browser-profiles.test.ts src/spotlight/config-store.test.ts
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
```

Expected: all commands exit `0`.

---

## Implementation Notes

- Keep backend tests mostly pure; avoid tests that depend on real Edge installation.
- Do not add new runtime dependencies.
- Do not add a database table or migration.
- Do not store absolute profile paths as keys; `profileDir` is the stable key.
- Do not clear config for profiles that no longer scan; hidden/alias/history should survive temporary Edge/User Data unavailability.
- Do not silently swallow launch errors; only stats-write failure after successful `spawn()` may become a warning.
- Keep UI text in Chinese and keep the panel as a working tool surface, not an explanatory landing page.
