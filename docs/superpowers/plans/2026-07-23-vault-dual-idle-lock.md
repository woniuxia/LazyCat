# Vault Dual Idle Lock Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add independently configurable Vault-activity and Windows-system-idle auto-lock rules, with a 30-second backend monitor and immediate key clearing when either enabled rule expires.

**Architecture:** Keep session ownership in `vault.rs`, extract configuration parsing and expiration decisions into a pure Rust module, and reuse `GetLastInputInfo` through a richer input snapshot. The frontend normalizes the same settings, persists changes through the existing queue, and listens for a backend lock event while retaining status polling as a fallback.

**Tech Stack:** Tauri 2, Rust, rusqlite, Windows API, Vue 3, TypeScript, Element Plus, Vitest.

---

## File Map

- Create `apps/desktop/src-tauri/src/tools/vault_lock.rs`: Rust configuration and pure expiration decisions.
- Modify `apps/desktop/src-tauri/src/tools/widget/guards.rs`: fallible Windows input snapshot.
- Modify `apps/desktop/src-tauri/src/tools/vault.rs`: session enforcement and 30-second monitor.
- Modify `apps/desktop/src-tauri/src/tools/mod.rs`, `events.rs`, and `main.rs`: module, event, and startup wiring.
- Modify `apps/desktop/src/utils/vaultLock.ts` and its test: frontend policy and migration.
- Modify `apps/desktop/src/composables/useSettings.ts` and its test: persistence and subscriptions.
- Modify `apps/desktop/src/components/SettingsPanel.vue` and `VaultPanel.vue`: controls and runtime reactions.
- Modify `docs/experience/vault-and-inbox.md`: durable security guidance.

### Task 1: Frontend Lock Policy Model

**Files:**
- Modify: `apps/desktop/src/utils/vaultLock.test.ts`
- Modify: `apps/desktop/src/utils/vaultLock.ts`

- [ ] **Step 1: Write failing normalization tests**

Add:

```ts
import {
  resolveVaultLockSettings,
  summarizeVaultHardLockRules,
  toVaultLockRuntimePolicy,
} from "./vaultLock";

it("migrates balanced and enables system idle lock by default", () => {
  expect(resolveVaultLockSettings({ vault_lock_profile: "balanced" })).toEqual({
    sensitiveHideMinutes: 2,
    activityLockEnabled: true,
    activityLockMinutes: 30,
    systemIdleLockEnabled: true,
    systemIdleLockMinutes: 15,
  });
});

it("prefers explicit settings and normalizes illegal values", () => {
  expect(resolveVaultLockSettings({
    vault_lock_profile: "strict",
    vault_sensitive_hide_minutes: "5",
    vault_activity_lock_enabled: "false",
    vault_activity_lock_minutes: "60",
    vault_system_idle_lock_enabled: "true",
    vault_system_idle_lock_minutes: "30",
  })).toMatchObject({
    sensitiveHideMinutes: 5,
    activityLockEnabled: false,
    activityLockMinutes: 60,
    systemIdleLockMinutes: 30,
  });
  expect(resolveVaultLockSettings({
    vault_activity_lock_minutes: "999",
    vault_system_idle_lock_enabled: "invalid",
  })).toMatchObject({
    activityLockMinutes: 30,
    systemIdleLockEnabled: true,
  });
});

it("builds runtime policy and OR summaries", () => {
  const settings = resolveVaultLockSettings({});
  expect(toVaultLockRuntimePolicy(settings)).toEqual({
    hideSensitiveAfterSecs: 120,
    activityLockEnabled: true,
    activityLockAfterSecs: 1800,
  });
  expect(summarizeVaultHardLockRules(settings)).toBe("任一条件达到后即锁定");
  expect(summarizeVaultHardLockRules({
    ...settings,
    activityLockEnabled: false,
    systemIdleLockEnabled: false,
  })).toBe("仅手动或关闭到托盘时锁定");
});
```

- [ ] **Step 2: Verify RED**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/vaultLock.test.ts
```

Expected: FAIL because the three new exports are undefined.

- [ ] **Step 3: Implement the frontend model**

Keep existing profile exports until Task 5 removes their callers. Add:

```ts
export const VAULT_HARD_LOCK_MINUTES = [5, 10, 15, 30, 60] as const;
export const VAULT_SENSITIVE_HIDE_MINUTES = [1, 2, 5] as const;
export type VaultHardLockMinutes = typeof VAULT_HARD_LOCK_MINUTES[number];
export type VaultSensitiveHideMinutes = typeof VAULT_SENSITIVE_HIDE_MINUTES[number];

export interface VaultLockSettings {
  sensitiveHideMinutes: VaultSensitiveHideMinutes;
  activityLockEnabled: boolean;
  activityLockMinutes: VaultHardLockMinutes;
  systemIdleLockEnabled: boolean;
  systemIdleLockMinutes: VaultHardLockMinutes;
}

export const VAULT_LOCK_SETTING_KEYS = {
  sensitiveHideMinutes: "vault_sensitive_hide_minutes",
  activityLockEnabled: "vault_activity_lock_enabled",
  activityLockMinutes: "vault_activity_lock_minutes",
  systemIdleLockEnabled: "vault_system_idle_lock_enabled",
  systemIdleLockMinutes: "vault_system_idle_lock_minutes",
} as const;

const LEGACY = {
  strict: { sensitiveHideMinutes: 1, activityLockMinutes: 10 },
  balanced: { sensitiveHideMinutes: 2, activityLockMinutes: 30 },
  convenient: { sensitiveHideMinutes: 5, activityLockMinutes: 60 },
} as const;

function parseBoolean(raw: string | undefined, fallback: boolean): boolean {
  if (raw === "true") return true;
  if (raw === "false") return false;
  return fallback;
}

function parseAllowed<T extends readonly number[]>(
  raw: string | undefined,
  allowed: T,
  fallback: T[number],
): T[number] {
  const value = Number(raw);
  return allowed.includes(value as T[number]) ? value as T[number] : fallback;
}

export function resolveVaultLockSettings(
  raw: Record<string, string | undefined>,
): VaultLockSettings {
  const legacy = LEGACY[normalizeVaultLockProfile(raw.vault_lock_profile)];
  return {
    sensitiveHideMinutes: parseAllowed(raw.vault_sensitive_hide_minutes,
      VAULT_SENSITIVE_HIDE_MINUTES, legacy.sensitiveHideMinutes),
    activityLockEnabled: parseBoolean(raw.vault_activity_lock_enabled, true),
    activityLockMinutes: parseAllowed(raw.vault_activity_lock_minutes,
      VAULT_HARD_LOCK_MINUTES, legacy.activityLockMinutes),
    systemIdleLockEnabled: parseBoolean(raw.vault_system_idle_lock_enabled, true),
    systemIdleLockMinutes: parseAllowed(raw.vault_system_idle_lock_minutes,
      VAULT_HARD_LOCK_MINUTES, 15),
  };
}

export function toVaultLockRuntimePolicy(settings: VaultLockSettings) {
  return {
    hideSensitiveAfterSecs: settings.sensitiveHideMinutes * 60,
    activityLockEnabled: settings.activityLockEnabled,
    activityLockAfterSecs: settings.activityLockMinutes * 60,
  };
}

export function summarizeVaultHardLockRules(settings: VaultLockSettings): string {
  if (settings.activityLockEnabled && settings.systemIdleLockEnabled) return "任一条件达到后即锁定";
  if (settings.activityLockEnabled) return `Vault 无活动 ${settings.activityLockMinutes} 分钟后锁定`;
  if (settings.systemIdleLockEnabled) return `电脑无操作 ${settings.systemIdleLockMinutes} 分钟后锁定`;
  return "仅手动或关闭到托盘时锁定";
}
```

- [ ] **Step 4: Verify GREEN and commit**

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/vaultLock.test.ts
git add apps/desktop/src/utils/vaultLock.ts apps/desktop/src/utils/vaultLock.test.ts
git commit -m "feat: 定义密码库双空闲锁定策略"
```

Expected: all focused tests PASS before the commit.

### Task 2: Typed Settings Persistence

**Files:**
- Modify: `apps/desktop/src/composables/useSettings.test.ts`
- Modify: `apps/desktop/src/composables/useSettings.ts`

- [ ] **Step 1: Write failing persistence tests**

```ts
import {
  getVaultLockSettings,
  setVaultLockSettingAndWait,
  subscribeVaultLockSettings,
} from "./useSettings";

it("notifies after a typed Vault setting commits", async () => {
  const listener = vi.fn();
  const unsubscribe = subscribeVaultLockSettings(listener);
  await setVaultLockSettingAndWait("systemIdleLockMinutes", 30);
  expect(invokeMock).toHaveBeenCalledWith("tool:settings:set", {
    key: "vault_system_idle_lock_minutes",
    value: "30",
  });
  expect(getVaultLockSettings().systemIdleLockMinutes).toBe(30);
  expect(listener).toHaveBeenCalledTimes(1);
  unsubscribe();
});

it("does not notify when persistence fails", async () => {
  const listener = vi.fn();
  const unsubscribe = subscribeVaultLockSettings(listener);
  invokeMock.mockRejectedValueOnce(new Error("write failed"));
  await expect(setVaultLockSettingAndWait("activityLockEnabled", false))
    .rejects.toThrow("write failed");
  expect(listener).not.toHaveBeenCalled();
  unsubscribe();
});
```

- [ ] **Step 2: Verify RED**

```powershell
pnpm --filter @lazycat/desktop test -- src/composables/useSettings.test.ts
```

Expected: FAIL on missing typed Vault functions.

- [ ] **Step 3: Implement typed access and post-commit notification**

```ts
type VaultLockSettingName = keyof VaultLockSettings;
type VaultLockSettingsListener = (value: VaultLockSettings) => void;
const vaultLockSettingsListeners = new Set<VaultLockSettingsListener>();

export function getVaultLockSettings(): VaultLockSettings {
  return resolveVaultLockSettings(settings);
}

export function subscribeVaultLockSettings(listener: VaultLockSettingsListener): () => void {
  vaultLockSettingsListeners.add(listener);
  return () => vaultLockSettingsListeners.delete(listener);
}

export async function setVaultLockSettingAndWait<K extends VaultLockSettingName>(
  name: K,
  value: VaultLockSettings[K],
): Promise<void> {
  await setSettingAndWait(VAULT_LOCK_SETTING_KEYS[name], String(value));
  const current = getVaultLockSettings();
  for (const listener of vaultLockSettingsListeners) listener(current);
}
```

Import the policy types/functions from `vaultLock.ts`. Do not write migration defaults during startup; missing keys resolve from `vault_lock_profile` in both layers.

- [ ] **Step 4: Verify GREEN and commit**

```powershell
pnpm --filter @lazycat/desktop test -- src/composables/useSettings.test.ts
git add apps/desktop/src/composables/useSettings.ts apps/desktop/src/composables/useSettings.test.ts
git commit -m "feat: 持久化密码库独立锁定设置"
```

Expected: all settings tests PASS, including existing rollback and serialization cases.

### Task 3: Rust Policy Core and Input Snapshot

**Files:**
- Create: `apps/desktop/src-tauri/src/tools/vault_lock.rs`
- Modify: `apps/desktop/src-tauri/src/tools/widget/guards.rs`
- Modify: `apps/desktop/src-tauri/src/tools/mod.rs`

- [ ] **Step 1: Write failing pure decision tests**

Create the module with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> VaultLockConfig {
        VaultLockConfig {
            activity_enabled: true,
            activity_after_secs: 1800,
            system_idle_enabled: true,
            system_idle_after_secs: 900,
        }
    }

    #[test]
    fn either_rule_expires_at_the_boundary() {
        let input = SystemInputSnapshot { last_input_tick_ms: 10, idle_secs: 900 };
        assert_eq!(expired_reason(config(), 10, Some(input), None), Some(LockReason::SystemIdle));
        assert_eq!(expired_reason(config(), 1800, None, None), Some(LockReason::VaultActivity));
    }

    #[test]
    fn detects_threshold_crossed_before_input_reset() {
        let previous = SystemInputSnapshot { last_input_tick_ms: 1_000, idle_secs: 870 };
        let current = SystemInputSnapshot { last_input_tick_ms: 901_000, idle_secs: 1 };
        assert_eq!(expired_reason(config(), 0, Some(current), Some(previous)),
            Some(LockReason::SystemIdle));
    }

    #[test]
    fn disabled_rules_and_missing_samples_do_not_expire() {
        let disabled = VaultLockConfig {
            activity_enabled: false,
            system_idle_enabled: false,
            ..config()
        };
        assert_eq!(expired_reason(disabled, 99_999, None, None), None);
    }
}
```

- [ ] **Step 2: Register the module and verify RED**

Add `mod vault_lock;` in `tools/mod.rs`, then run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml vault_lock
```

Expected: FAIL because policy types/functions are undefined.

- [ ] **Step 3: Expose a fallible input snapshot**

In `widget/guards.rs` add:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SystemInputSnapshot {
    pub last_input_tick_ms: u32,
    pub idle_secs: u64,
}

#[cfg(windows)]
pub use imp::try_system_input_snapshot;
#[cfg(not(windows))]
pub fn try_system_input_snapshot() -> Option<SystemInputSnapshot> { None }

pub fn seconds_idle() -> u32 {
    try_system_input_snapshot()
        .map(|value| value.idle_secs.min(u32::MAX as u64) as u32)
        .unwrap_or(0)
}
```

The Windows implementation returns `None` on `GetLastInputInfo` failure and otherwise returns raw `dwTime` plus the wrapping difference from `GetTickCount64`. Preserve `seconds_idle()` for `widget/pulse.rs`.

- [ ] **Step 4: Implement config loading and decisions**

In `vault_lock.rs`, define:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VaultLockConfig {
    pub activity_enabled: bool,
    pub activity_after_secs: u64,
    pub system_idle_enabled: bool,
    pub system_idle_after_secs: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum LockReason { VaultActivity, SystemIdle }

pub(crate) fn expired_reason(
    config: VaultLockConfig,
    vault_idle_secs: u64,
    current: Option<SystemInputSnapshot>,
    previous: Option<SystemInputSnapshot>,
) -> Option<LockReason> {
    if config.activity_enabled && vault_idle_secs >= config.activity_after_secs {
        return Some(LockReason::VaultActivity);
    }
    if !config.system_idle_enabled { return None; }
    let current = current?;
    if current.idle_secs >= config.system_idle_after_secs {
        return Some(LockReason::SystemIdle);
    }
    if let Some(previous) = previous {
        let between_inputs_ms = current.last_input_tick_ms
            .wrapping_sub(previous.last_input_tick_ms) as u64;
        if current.last_input_tick_ms != previous.last_input_tick_ms
            && between_inputs_ms >= config.system_idle_after_secs * 1000
        {
            return Some(LockReason::SystemIdle);
        }
    }
    None
}
```

Implement `load_config(&Connection)` with `rusqlite::OptionalExtension`: accept only booleans `true/false`, accept only minutes `5/10/15/30/60`, map missing legacy `strict/balanced/convenient` activity values to `10/30/60`, and default system idle to enabled 15.

- [ ] **Step 5: Verify GREEN and commit**

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml vault_lock
git add apps/desktop/src-tauri/src/tools/vault_lock.rs apps/desktop/src-tauri/src/tools/widget/guards.rs apps/desktop/src-tauri/src/tools/mod.rs
git commit -m "feat: 增加密码库空闲锁定判定核心"
```

Expected: policy and guards tests PASS before commit.

### Task 4: Session Enforcement and 30-Second Monitor

**Files:**
- Modify: `apps/desktop/src-tauri/src/tools/vault.rs`
- Modify: `apps/desktop/src-tauri/src/events.rs`
- Modify: `apps/desktop/src-tauri/src/main.rs`

- [ ] **Step 1: Write a failing dual-rule session test**

Update the existing `VaultSession` fixture so it only contains `key` and `last_activity`, then add:

```rust
#[test]
fn system_idle_expiry_clears_the_session_key() {
    let mut guard = Some(VaultSession {
        key: Some([7u8; KEY_LEN]),
        last_activity: Instant::now(),
    });
    let config = VaultLockConfig {
        activity_enabled: false,
        activity_after_secs: 1800,
        system_idle_enabled: true,
        system_idle_after_secs: 900,
    };
    let current = SystemInputSnapshot { last_input_tick_ms: 10, idle_secs: 900 };
    let error = ensure_session_alive(&mut guard, config, Some(current), None)
        .expect_err("system idle must lock");
    assert_eq!(error, "vault_locked_timeout");
    assert!(guard.is_none());
}
```

- [ ] **Step 2: Verify RED**

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml vault::tests
```

Expected: FAIL because the session and validator still use one cached timeout.

- [ ] **Step 3: Enforce current persisted policy before sensitive use**

Remove `hard_lock_after_secs` and the old profile loaders from `vault.rs`. Import the new policy types and replace validation with:

```rust
fn ensure_session_alive(
    guard: &mut Option<VaultSession>,
    config: VaultLockConfig,
    current: Option<SystemInputSnapshot>,
    previous: Option<SystemInputSnapshot>,
) -> Result<(), String> {
    let Some(session) = guard.as_ref() else {
        return Err("vault_locked".to_string());
    };
    if expired_reason(config, session.last_activity.elapsed().as_secs(), current, previous)
        .is_some()
    {
        hard_lock_session(guard);
        return Err("vault_locked_timeout".to_string());
    }
    Ok(())
}
```

For `status`, reuse its SQLite connection. For `touch` and `get_session_key`, load config and sample `try_system_input_snapshot()` before taking `VAULT_SESSION`; never hold the session mutex during database or Windows calls. Store only `key` and `last_activity` in every session constructor, and update activity only after validation succeeds.

- [ ] **Step 4: Write a failing monitor transition test**

```rust
#[test]
fn monitor_locks_once_after_input_resets() {
    install_test_session(Instant::now());
    let previous = SystemInputSnapshot { last_input_tick_ms: 1_000, idle_secs: 870 };
    let current = SystemInputSnapshot { last_input_tick_ms: 901_000, idle_secs: 1 };
    let reason = check_session_for_monitor(
        config_for_system_idle(900), Some(current), Some(previous));
    assert_eq!(reason, Some(LockReason::SystemIdle));
    assert!(VAULT_SESSION.lock().expect("session lock").is_none());
    assert_eq!(check_session_for_monitor(
        config_for_system_idle(900), Some(current), None), None);
}
```

Expected: FAIL because the monitor helper is absent.

- [ ] **Step 5: Add the event and idempotent monitor**

Add `EVENT_VAULT_LOCKED: &str = "vault://locked"` to `events.rs` and `ALL`. Implement `monitor_once(previous)` so a locked Vault returns `(None, None)` before database or Windows work and an expired session clears its key exactly once. Wrap it with:

```rust
pub fn start_auto_lock_monitor(app: tauri::AppHandle) {
    static RUNNING: AtomicBool = AtomicBool::new(false);
    if RUNNING.swap(true, Ordering::SeqCst) { return; }
    std::thread::spawn(move || {
        let mut previous = None;
        loop {
            std::thread::sleep(Duration::from_secs(30));
            match monitor_once(previous) {
                Ok((_, Some(reason))) => {
                    previous = None;
                    let _ = app.emit(crate::events::EVENT_VAULT_LOCKED,
                        json!({ "reason": reason }));
                }
                Ok((next, None)) => previous = next,
                Err(error) => eprintln!("vault auto-lock monitor failed: {error}"),
            }
        }
    });
}
```

Import `tauri::Emitter` and atomic types. In `main.rs`, add `tools::vault::start_auto_lock_monitor(app.handle().clone());` beside existing scheduler starts.

- [ ] **Step 6: Verify GREEN and commit**

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml vault
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
git add apps/desktop/src-tauri/src/tools/vault.rs apps/desktop/src-tauri/src/events.rs apps/desktop/src-tauri/src/main.rs
git commit -m "feat: 后端监测密码库双空闲锁定"
```

Expected: Vault tests PASS and `cargo check` exits 0 before commit.

### Task 5: Settings UI and Vault Runtime Wiring

**Files:**
- Modify: `apps/desktop/src/components/SettingsPanel.vue`
- Modify: `apps/desktop/src/components/VaultPanel.vue`
- Modify: `apps/desktop/src/utils/vaultLock.test.ts`

- [ ] **Step 1: Add a runtime-policy regression test**

```ts
it("keeps masking while activity hard lock is disabled", () => {
  const settings = resolveVaultLockSettings({ vault_activity_lock_enabled: "false" });
  expect(toVaultLockRuntimePolicy(settings)).toEqual({
    hideSensitiveAfterSecs: 120,
    activityLockEnabled: false,
    activityLockAfterSecs: 1800,
  });
});
```

Run the focused `vaultLock.test.ts` command. Expected: PASS after Task 1; this pins component behavior before editing Vue files.

- [ ] **Step 2: Replace the combined preset with explicit controls**

Remove `vaultLockProfile`, its hint, old imports, handler, and profile styles from `SettingsPanel.vue`. Initialize:

```ts
const vaultLockSettings = reactive(getVaultLockSettings());
const vaultLockSummary = computed(() => summarizeVaultHardLockRules(vaultLockSettings));

async function saveVaultLockSetting<K extends keyof VaultLockSettings>(name: K) {
  try {
    await setVaultLockSettingAndWait(name, vaultLockSettings[name]);
    Object.assign(vaultLockSettings, getVaultLockSettings());
    ElMessage.success("密码库锁定设置已更新");
  } catch (error) {
    Object.assign(vaultLockSettings, getVaultLockSettings());
    ElMessage.error(`设置失败：${(error as Error).message}`);
    return;
  }
  try {
    await invokeToolByChannel("tool:vault:status", {});
  } catch {
    // 保存已成功；VaultPanel 的订阅和轮询继续同步状态。
  }
}
```

Render three rows in the existing section. The first row is:

```vue
<div class="setting-item">
  <div class="setting-label">
    <span class="label-text">敏感信息隐藏</span>
    <span class="label-desc">窗口失焦时仍会立即恢复密码掩码</span>
  </div>
  <el-select v-model="vaultLockSettings.sensitiveHideMinutes"
    @change="saveVaultLockSetting('sensitiveHideMinutes')">
    <el-option v-for="minutes in VAULT_SENSITIVE_HIDE_MINUTES" :key="minutes"
      :label="`${minutes} 分钟`" :value="minutes" />
  </el-select>
</div>
```

Use this exact shape for each hard-lock row:

```vue
<div class="setting-item">
  <div class="setting-label">
    <span class="label-text">Vault 无活动自动锁定</span>
    <span class="label-desc">没有操作密码库达到时长后清除解锁会话</span>
  </div>
  <div class="vault-lock-rule-control">
    <el-switch v-model="vaultLockSettings.activityLockEnabled"
      @change="saveVaultLockSetting('activityLockEnabled')" />
    <el-select v-model="vaultLockSettings.activityLockMinutes"
      :disabled="!vaultLockSettings.activityLockEnabled"
      @change="saveVaultLockSetting('activityLockMinutes')">
      <el-option v-for="minutes in VAULT_HARD_LOCK_MINUTES" :key="minutes"
        :label="`${minutes} 分钟`" :value="minutes" />
    </el-select>
  </div>
</div>
```

Repeat it for `systemIdleLockEnabled` / `systemIdleLockMinutes` with label “电脑无操作自动锁定”. Show `vaultLockSummary` below. Keep a stable select width and no new card container.

- [ ] **Step 3: Wire VaultPanel to settings and backend events**

Replace profile getters with `getVaultLockSettings()` and `toVaultLockRuntimePolicy()`. Always start masking and conditionally start the activity hard-lock timer:

```ts
hideSensitiveTimer = setTimeout(hideSensitiveContent,
  currentLockPolicy.hideSensitiveAfterSecs * 1000);
if (currentLockPolicy.activityLockEnabled) {
  hardLockTimer = setTimeout(() => { void onLock(); },
    currentLockPolicy.activityLockAfterSecs * 1000);
}
```

Add `unlistenVaultLocked` and `unsubscribeVaultLockSettings`. On mount:

```ts
void listen("vault://locked", () => setLockState("locked"))
  .then((unlisten) => { unlistenVaultLocked = unlisten; })
  .catch(() => { unlistenVaultLocked = null; });

unsubscribeVaultLockSettings = subscribeVaultLockSettings((settings) => {
  currentLockPolicy = toVaultLockRuntimePolicy(settings);
  startInactivityTimers();
  void reconcileVaultSessionOnFocus();
});
```

Call both cleanup functions in `onBeforeUnmount`. Keep the existing 30-second status interval as event-delivery fallback.

- [ ] **Step 4: Verify GREEN and commit**

```powershell
pnpm --filter @lazycat/desktop test -- src/utils/vaultLock.test.ts src/composables/useSettings.test.ts
pnpm --filter @lazycat/desktop typecheck
pnpm --filter @lazycat/desktop build:web
git add apps/desktop/src/components/SettingsPanel.vue apps/desktop/src/components/VaultPanel.vue apps/desktop/src/utils/vaultLock.test.ts
git commit -m "feat: 配置密码库双空闲锁定规则"
```

Expected: tests PASS, typecheck exits 0, and Vite completes before commit.

### Task 6: Experience Documentation and Final Verification

**Files:**
- Modify: `docs/experience/vault-and-inbox.md`

- [ ] **Step 1: Record the durable security boundary**

Add this section and increment the usage counter by one:

```markdown
## 双空闲自动锁定

Vault 活动空闲与 Windows 系统输入空闲是两条独立规则，允许单独启用或按 OR 语义叠加。真正的硬锁由 Rust 会话层统一判断并清零密钥；前端计时器只负责敏感信息隐藏、活动续期和锁屏同步。

系统空闲使用 `GetLastInputInfo` 的最后输入 tick，并由后端每 30 秒采样。监测保留相邻输入 tick，避免用户恢复输入后空闲值归零而漏掉此前已达到的阈值；请求前仍执行同一策略作为最后防线。
```

- [ ] **Step 2: Run the complete relevant verification**

```powershell
pnpm test
pnpm typecheck
pnpm --filter @lazycat/desktop build:web
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml vault
cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml
git diff --check
```

Expected: every command exits 0. If an unrelated existing test fails, record its exact name and separately prove the targeted Vault tests pass.

- [ ] **Step 3: Perform the Windows smoke paths**

1. Enable only Vault-activity lock at 5 minutes; use another application for over 5 minutes and confirm Vault locks.
2. Enable only computer-idle lock at 5 minutes; leave the computer untouched and confirm lock between 5:00 and 5:30.
3. Enable both and confirm the first reached rule locks.
4. Disable both and confirm automatic hard lock stops while manual and close-to-tray lock remain.
5. Shorten an enabled threshold below current idle time and confirm the next check locks.
6. Confirm lost focus hides revealed passwords without destroying the session.

- [ ] **Step 4: Commit documentation and inspect scope**

```powershell
git add docs/experience/vault-and-inbox.md
git commit -m "docs: 记录密码库双空闲锁定边界"
git status --short
git log -6 --oneline
```

Expected: feature commits contain only files listed in this plan; preserve the unrelated release-package changes already present in the worktree.
