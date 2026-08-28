# E3 result: Slint dynamic tabs failed

Status: failed hard gate on 2026-08-28.

## Environment

- Windows reported as Windows 10 Pro, version 2009, build 26200, 64-bit.
- Rust 1.93.1, `x86_64-pc-windows-msvc`.
- Slint and `slint-build` locked to official stable 1.17.1.
- Isolated prototype only; no LazyCat process, database, or user directory.

## Gate under test

Dynamic heterogeneous tool tabs must retain input, selection, inner and outer
scroll positions, and local operation context when switching; closing alone
destroys page state. Focus must remain valid and keyboard operation must not
crash. The implementation may keep page trees alive or use explicit Rust page
state, but may not depend on a growing central dispatch or unstable behavior
that cannot be confined behind a small interface.

## E3 failures

### Retained handle approach

The first implementation retained one strong component handle per open tab and
returned it through a small type-erased `ToolView` interface. This preserved
component properties in an unshown runtime test, but the first user-visible run
failed:

```text
Focused ID #132448258 is not in the node list
Focused ID #1114115 is not in the node list
```

The user reported several such errors. An automated shown-window focus sequence
then reproduced a more severe symptom: switching after focusing embedded input
controls caused Slint `properties.rs:628: Recursion detected`. Across 20 runs,
9 crashed. Moving focus to a stable main-window node did not fix it (8/20 still
crashed). The failure therefore was not an application-level missing blur.

### Factory-created handle approach

The second implementation followed the official `ComponentFactory` contract
and created a fresh handle inside the factory closure. The same focus sequence
then completed 20/20 times without the recursive-property panic, confirming
that reusing a pre-created handle across embedded parent trees was the cause.

This alternative did not meet the state-continuity gate. The outgoing generated
weak handle could not reliably upgrade after ownership transferred into
`ComponentContainer`, so the explicit Rust snapshot could not read the live
draft, selection, or scroll state. With actions separated by real event-loop
frames, rebuilding TextTool and FormTool subsequently panicked in FemtoVG:

```text
called `Result::unwrap()` on an `Err` value: "Unable to create Texture object"
```

The retained command below returns non-zero because restored state cannot be
observed in the recreated component:

```powershell
& .\docs\archive\native-rust-gui\slint-2026-08-28\target\debug\slint-native-risk-prototype.exe --state-smoke
```

### Long-list interaction

Slint 1.17.1 `ListView` inside the experimental static
`ComponentContainer` independently produced recursive layout evaluation at
startup. Replacing only `ListView` with a non-virtualized `ScrollView` avoided
that crash, at the cost of losing the suitable long-list primitive.

## Decision

Slint 1.17.1 fails the agreed dynamic-tab state continuity and complete focus
path hard gates. Its only heterogeneous static embedding route is experimental,
and neither retaining nor rebuilding handles produced a reliable, maintainable
result on the target Windows environment. No IME, mixed-DPI, shell, async, or
visual scoring work should continue for Slint; positive scores cannot offset
this veto.
