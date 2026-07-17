# Default Lite Portable Packaging Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the Windows release script build only the lite portable zip by default while preserving the existing four-package flow behind `-AllPackages`.

**Architecture:** Keep one release script and select the expected artifact list from a new switch. The default branch uses Tauri `--no-bundle` and packages only the existing portable runtime files; the all-packages branch retains the current fixedRuntime NSIS and two-portable workflow. Hashing and GitHub upload consume the selected artifact list so `-SkipBuild` remains mode-aware.

**Tech Stack:** PowerShell, pnpm, Tauri 2, JSON package scripts, Markdown project documentation

---

### Task 1: Add mode-aware release behavior

**Files:**
- Modify: `scripts/release-all-win.ps1`

**Step 1: Add the public switch and selected artifacts**

Add `[switch]$AllPackages`. Define the four artifact paths as today, then set `$artifacts` to only `$portableLiteZip` by default or all four paths when `$AllPackages` is present.

**Step 2: Split prerequisite checks by mode**

Resolve and require the fixed WebView2 runtime only inside the all-packages path. Keep common command, version, Git, output-directory and upload checks unchanged.

**Step 3: Implement the default build path**

Run `build:web`, invoke `tauri build --no-bundle` in the VS developer environment, create only the lite stage, call `Copy-PortableFiles`, and zip it to `$portableLiteZip`.

**Step 4: Preserve the full build path**

Move the current fixedRuntime NSIS build, lite NSIS derivation, dual-stage copy and dual zip creation under `$AllPackages` without changing its commands or artifact names.

**Step 5: Make post-build behavior mode-aware**

Use `$artifacts` for existence checks, SHA lines and upload assets. Append `$shaFile` only when calling GitHub CLI. Replace fixed `[1/6]` progress labels with mode-neutral stage messages so both branches report truthfully.

**Step 6: Parse the script**

Run:

```powershell
powershell -NoProfile -Command "$errors = $null; [void][System.Management.Automation.Language.Parser]::ParseFile((Resolve-Path 'scripts/release-all-win.ps1'), [ref]$null, [ref]$errors); if ($errors.Count) { $errors | Format-List; exit 1 }"
```

Expected: exit code 0 with no parser errors.

### Task 2: Expose default and all-package commands

**Files:**
- Modify: `package.json`

**Step 1: Update package scripts**

Add `release:win` calling `scripts/release-all-win.ps1` without a mode flag. Change `release:all:win` to call the same script with `-AllPackages` before forwarded user arguments.

**Step 2: Validate JSON and command values**

Run:

```powershell
node -e "const p=require('./package.json'); if(!p.scripts['release:win'] || !p.scripts['release:all:win'].includes('-AllPackages')) process.exit(1)"
```

Expected: exit code 0.

### Task 3: Synchronize documentation and project rules

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`
- Modify: `CLAUDE.md`
- Modify: `process.md`

**Step 1: Document commands and package modes**

Describe `pnpm release:win -- -Tag vX.Y.Z` as the default lite portable build and retain `pnpm release:all:win -- -Tag vX.Y.Z` as the four-package flow. Update `-SkipBuild` and `-SkipUpload` examples where necessary.

**Step 2: Keep both agent documents identical**

Apply matching changes to `AGENTS.md` and `CLAUDE.md`, including quick commands, build behavior and formal release flow.

**Step 3: Record the packaging decision**

Append a concise `process.md` entry explaining why mode-specific artifact sets and delayed WebView2 checks are required.

**Step 4: Verify synchronized rules and references**

Run:

```powershell
git diff --no-index -- AGENTS.md CLAUDE.md
rg -n "release:win|release:all:win|AllPackages|portable-lite" package.json scripts/release-all-win.ps1 README.md AGENTS.md CLAUDE.md process.md
```

Expected: the first command has no diff; the second lists both default and full commands consistently.

### Task 4: Final validation

**Files:**
- Verify all modified files

**Step 1: Review the focused diff**

Run `git diff --check` and `git diff --stat`.

Expected: no whitespace errors; only planned script, command and documentation files changed.

**Step 2: Re-run script and JSON checks**

Repeat the PowerShell parser check and Node package-script assertion from Tasks 1 and 2.

**Step 3: Confirm mode invariants statically**

Verify that fixed WebView2 lookup, NSIS generation, full stage creation and full artifacts occur only in the `$AllPackages` branch, while `$artifacts` drives missing-file checks, SHA output and upload.

**Step 4: Report runtime validation boundary**

Do not run an actual Windows/Tauri package build unless explicitly requested; report that the expensive end-to-end packaging path remains unexecuted.
