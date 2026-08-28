# Slint native GUI risk prototype (archived failed gate)

Archived on 2026-08-28. This directory is frozen decision evidence for the
[resolved Wayfinder ticket](../../../../.scratch/native-rust-gui-wayfinder/issues/06-prototype-hardest-paths.md),
not an active implementation area.

This is a disposable, isolated decision prototype for the LazyCat native GUI
Wayfinder. It is not a product rewrite and does not read or write LazyCat user
data.

## Result

Slint 1.17.1 failed the dynamic-tab/focus hard gate. Do not extend this asset
into product code. The retained prototype documents two failed approaches:

- retaining component handles preserved state but produced stale accessibility
  focus IDs and intermittent recursive-property panics when switching;
- creating handles inside the factory avoided that focus-tree reuse, but the
  component could not be reliably upgraded for departure snapshots and repeated
  rebuilding triggered a FemtoVG texture panic.

The reproducible evidence and HITL checklist are in
[`evidence/e3-stage-1-dynamic-tabs.md`](evidence/e3-stage-1-dynamic-tabs.md).

Do not treat a successful build as E3 runtime evidence. The failed runtime
record remains authoritative; use the retained checklist only when auditing the
decision.

## Run

```powershell
cd docs/archive/native-rust-gui/slint-2026-08-28
cargo run
```

No existing LazyCat process or database is required.

Automated visible-startup regression:

```powershell
& .\target\debug\slint-native-risk-prototype.exe --startup-smoke
```

`--startup-smoke` only verifies startup without interaction. The current
`--state-smoke` deliberately exits non-zero because the official rebuild path
does not restore the probe state reliably.
