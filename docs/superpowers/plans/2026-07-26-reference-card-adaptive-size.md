# 置顶参考卡首次自适应尺寸实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让置顶参考卡仅在首次打开时根据初始文本调整窗口宽高，自适应上限为目标显示器可用工作区的 30%，同时保留用户手动突破该上限调整窗口的能力。

**Architecture:** 在 Rust `reference_card` 域新增无平台依赖的尺寸纯函数，根据文本显示列、折行视觉行和显示器逻辑工作区计算首次尺寸。窗口仍隐藏创建，由 Rust 在 ready 握手显示前选择目标显示器、应用首次尺寸并按现有规则定位；Vue、IPC 和正文生命周期保持不变。

**Tech Stack:** Rust 2021、Tauri 2、Vue 3、TypeScript、Vitest、Cargo test

---

## 文件结构

- Create: `apps/desktop/src-tauri/src/reference_card/size.rs`
  负责文本显示列、折行视觉行、30% 上限和最小尺寸优先的纯计算，并内置 Rust 单元测试。
- Modify: `apps/desktop/src-tauri/src/reference_card/mod.rs`
  负责把初始文本传入隐藏窗口创建流程，选择单一目标显示器，应用逻辑尺寸，并用同一显示器工作区完成物理定位。
- Modify: `apps/desktop/src/components/ReferenceCard.contract.test.ts`
  通过源码契约守卫首次自适应只发生在 Rust 隐藏创建阶段，窗口仍可手动调整，前端不接管持续 resize。

不修改 `ReferenceCard.vue`、IPC 类型、事件 payload 和 capability。现有架构经验已经覆盖动态窗口隐藏创建与 ready 握手，本次无需新增经验条目。

### Task 1: 建立内容尺寸纯函数

**Files:**

- Create: `apps/desktop/src-tauri/src/reference_card/size.rs`
- Modify: `apps/desktop/src-tauri/src/reference_card/mod.rs:1`
- Test: `apps/desktop/src-tauri/src/reference_card/size.rs`

- [ ] **Step 1: 写入失败的尺寸计算测试**

在 `apps/desktop/src-tauri/src/reference_card/mod.rs` 的模块声明区加入：

```rust
mod size;
```

创建 `apps/desktop/src-tauri/src/reference_card/size.rs`，先只写测试：

```rust
#[cfg(test)]
mod tests {
    use super::{adaptive_card_size, display_columns, CardSize};

    const FULL_HD_WORK_AREA: CardSize = CardSize {
        width: 1920.0,
        height: 1080.0,
    };

    #[test]
    fn short_text_uses_minimum_size() {
        assert_eq!(
            adaptive_card_size("short", FULL_HD_WORK_AREA),
            CardSize {
                width: 360.0,
                height: 220.0,
            }
        );
    }

    #[test]
    fn long_line_expands_width_within_monitor_limit() {
        assert_eq!(
            adaptive_card_size(&"x".repeat(60), FULL_HD_WORK_AREA).width,
            512.0
        );
    }

    #[test]
    fn multiple_lines_expand_height_within_monitor_limit() {
        let text = "line\n".repeat(11) + "line";
        assert_eq!(
            adaptive_card_size(&text, FULL_HD_WORK_AREA).height,
            290.0
        );
    }

    #[test]
    fn wrapped_long_line_adds_visual_rows() {
        assert_eq!(
            adaptive_card_size(&"x".repeat(700), FULL_HD_WORK_AREA),
            CardSize {
                width: 576.0,
                height: 271.0,
            }
        );
    }

    #[test]
    fn tabs_crlf_and_non_ascii_use_display_columns() {
        assert_eq!(display_columns("a\t中\r"), 6);
    }

    #[test]
    fn adaptive_size_stays_within_thirty_percent() {
        let text = ("x".repeat(2000) + "\n").repeat(100);
        assert_eq!(
            adaptive_card_size(&text, FULL_HD_WORK_AREA),
            CardSize {
                width: 576.0,
                height: 324.0,
            }
        );
    }

    #[test]
    fn minimum_size_wins_on_tiny_work_area() {
        assert_eq!(
            adaptive_card_size(
                &"x".repeat(2000),
                CardSize {
                    width: 800.0,
                    height: 600.0,
                },
            ),
            CardSize {
                width: 360.0,
                height: 220.0,
            }
        );
    }
}
```

- [ ] **Step 2: 运行测试并确认 RED**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml reference_card::size -- --nocapture
```

Expected: FAIL，`size.rs` 中的 `adaptive_card_size`、`display_columns` 和 `CardSize` 尚未定义。

- [ ] **Step 3: 写入最小尺寸计算实现**

在 `size.rs` 的测试模块前加入：

```rust
pub(crate) const REFERENCE_CARD_DEFAULT_WIDTH: f64 = 560.0;
pub(crate) const REFERENCE_CARD_DEFAULT_HEIGHT: f64 = 360.0;
pub(crate) const REFERENCE_CARD_MIN_WIDTH: f64 = 360.0;
pub(crate) const REFERENCE_CARD_MIN_HEIGHT: f64 = 220.0;

const MAX_WORK_AREA_RATIO: f64 = 0.30;
const TAB_WIDTH: usize = 4;
const MONACO_COLUMN_WIDTH: f64 = 8.0;
const MONACO_LINE_HEIGHT: f64 = 19.0;
const EDITOR_HORIZONTAL_CHROME: f64 = 32.0;
const EDITOR_VERTICAL_CHROME: f64 = 24.0;
const TOOLBAR_HEIGHT: f64 = 38.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CardSize {
    pub(crate) width: f64,
    pub(crate) height: f64,
}

fn display_columns(line: &str) -> usize {
    line.chars().fold(0, |columns, character| match character {
        '\r' => columns,
        '\t' => columns + (TAB_WIDTH - columns % TAB_WIDTH),
        character if character.is_ascii_control() => columns,
        character if character.is_ascii() => columns + 1,
        _ => columns + 2,
    })
}

fn wrapped_rows(columns: usize, available_columns: usize) -> usize {
    let visible_columns = columns.max(1);
    (visible_columns + available_columns - 1) / available_columns
}

pub(crate) fn adaptive_card_size(text: &str, work_area: CardSize) -> CardSize {
    let line_columns: Vec<usize> = text.split('\n').map(display_columns).collect();
    let longest_line = line_columns.iter().copied().max().unwrap_or(0);

    let maximum_width =
        (work_area.width * MAX_WORK_AREA_RATIO).max(REFERENCE_CARD_MIN_WIDTH);
    let natural_width =
        longest_line.max(1) as f64 * MONACO_COLUMN_WIDTH + EDITOR_HORIZONTAL_CHROME;
    let width = natural_width.clamp(REFERENCE_CARD_MIN_WIDTH, maximum_width);

    let available_columns =
        ((width - EDITOR_HORIZONTAL_CHROME) / MONACO_COLUMN_WIDTH)
            .floor()
            .max(1.0) as usize;
    let visual_rows: usize = line_columns
        .into_iter()
        .map(|columns| wrapped_rows(columns, available_columns))
        .sum();
    let natural_height = TOOLBAR_HEIGHT
        + EDITOR_VERTICAL_CHROME
        + visual_rows.max(1) as f64 * MONACO_LINE_HEIGHT;
    let maximum_height =
        (work_area.height * MAX_WORK_AREA_RATIO).max(REFERENCE_CARD_MIN_HEIGHT);
    let height = natural_height.clamp(REFERENCE_CARD_MIN_HEIGHT, maximum_height);

    CardSize { width, height }
}
```

不要引入 Unicode 宽度依赖；本需求明确使用“ASCII 1 列、非 ASCII 2 列”的可预测估算。

- [ ] **Step 4: 运行尺寸测试并确认 GREEN**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml reference_card::size -- --nocapture
```

Expected: 7 tests PASS，输出中无 panic 或 warning。

- [ ] **Step 5: 提交尺寸纯函数**

```powershell
git add apps/desktop/src-tauri/src/reference_card/size.rs apps/desktop/src-tauri/src/reference_card/mod.rs
git commit -m "feat(reference-card): 添加首次自适应尺寸计算"
```

### Task 2: 在隐藏窗口创建阶段应用首次尺寸

**Files:**

- Modify: `apps/desktop/src/components/ReferenceCard.contract.test.ts`
- Modify: `apps/desktop/src-tauri/src/reference_card/mod.rs:14-23`
- Modify: `apps/desktop/src-tauri/src/reference_card/mod.rs:266-329`
- Modify: `apps/desktop/src-tauri/src/reference_card/mod.rs:449-466`
- Test: `apps/desktop/src/components/ReferenceCard.contract.test.ts`

- [ ] **Step 1: 写入失败的窗口生命周期契约测试**

在 `ReferenceCard.contract.test.ts` 的顶部常量区加入：

```typescript
const referenceCardBackend = read("../src-tauri/src/reference_card/mod.rs");
```

在 `describe("ReferenceCard window wiring", ...)` 中加入：

```typescript
it("auto-sizes only during hidden creation and preserves manual resizing", () => {
  expect(referenceCardBackend).toContain(".visible(false)");
  expect(referenceCardBackend).toContain("configure_initial_geometry(&window, &text, ordinal)");
  expect(referenceCardBackend).toContain(".resizable(true)");
  expect(referenceCardBackend).not.toContain(".max_inner_size(");
  expect(component).not.toContain("setSize(");
  expect(component).not.toContain("onResized(");
});
```

- [ ] **Step 2: 运行契约测试并确认 RED**

Run:

```powershell
pnpm --filter @lazycat/desktop test -- src/components/ReferenceCard.contract.test.ts
```

Expected: FAIL，缺少 `configure_initial_geometry(&window, &text, ordinal)` 调用；其他现有契约仍通过。

- [ ] **Step 3: 调整导入和尺寸常量来源**

将 `mod.rs` 顶部：

```rust
mod position;
mod state;
```

改为：

```rust
mod position;
mod size;
mod state;
```

如果 Task 1 已加入 `mod size;`，本步只确认声明恰好存在一次。

将 Tauri 导入改为：

```rust
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, Monitor, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
```

在现有 `position` 和 `state` 导入之间加入：

```rust
use size::{
    adaptive_card_size, CardSize, REFERENCE_CARD_DEFAULT_HEIGHT,
    REFERENCE_CARD_DEFAULT_WIDTH, REFERENCE_CARD_MIN_HEIGHT, REFERENCE_CARD_MIN_WIDTH,
};
```

删除 `mod.rs` 中原有四个尺寸常量：

```rust
const REFERENCE_CARD_WIDTH: f64 = 560.0;
const REFERENCE_CARD_HEIGHT: f64 = 360.0;
const REFERENCE_CARD_MIN_WIDTH: f64 = 360.0;
const REFERENCE_CARD_MIN_HEIGHT: f64 = 220.0;
```

- [ ] **Step 4: 用同一显示器完成尺寸与定位**

用以下实现替换现有 `position_window`：

```rust
fn target_monitor(window: &WebviewWindow) -> Option<Monitor> {
    window
        .cursor_position()
        .ok()
        .and_then(|cursor| window.monitor_from_point(cursor.x, cursor.y).ok().flatten())
        .or_else(|| window.current_monitor().ok().flatten())
        .or_else(|| window.primary_monitor().ok().flatten())
}

fn logical_work_area(monitor: &Monitor) -> CardSize {
    let work_area = monitor.work_area();
    let scale = monitor.scale_factor();
    CardSize {
        width: f64::from(work_area.size.width) / scale,
        height: f64::from(work_area.size.height) / scale,
    }
}

fn physical_window_size(size: CardSize, scale: f64) -> PhysicalSize {
    PhysicalSize {
        width: (size.width * scale)
            .round()
            .clamp(0.0, i32::MAX as f64) as i32,
        height: (size.height * scale)
            .round()
            .clamp(0.0, i32::MAX as f64) as i32,
    }
}

fn position_window(
    window: &WebviewWindow,
    monitor: &Monitor,
    window_size: PhysicalSize,
    ordinal: usize,
) {
    let work_area = monitor.work_area();
    let area = PhysicalRect {
        x: work_area.position.x,
        y: work_area.position.y,
        width: i32::try_from(work_area.size.width).unwrap_or(i32::MAX),
        height: i32::try_from(work_area.size.height).unwrap_or(i32::MAX),
    };
    let (x, y) = card_position(area, window_size, ordinal);
    if let Err(error) = window.set_position(tauri::PhysicalPosition::new(x, y)) {
        eprintln!(
            "[reference-card] position {} failed: {error}",
            window.label()
        );
    }
}

fn configure_initial_geometry(
    window: &WebviewWindow,
    text: &str,
    ordinal: usize,
) -> Result<(), String> {
    let Some(monitor) = target_monitor(window) else {
        eprintln!(
            "[reference-card] target monitor {} unavailable; keeping default size",
            window.label()
        );
        return Ok(());
    };

    let scale = monitor.scale_factor();
    let size = adaptive_card_size(text, logical_work_area(&monitor));
    window
        .set_size(LogicalSize::new(size.width, size.height))
        .map_err(|error| format!("设置参考卡首次尺寸失败: {error}"))?;
    position_window(
        window,
        &monitor,
        physical_window_size(size, scale),
        ordinal,
    );
    Ok(())
}
```

`target_monitor` 只选择一次显示器；尺寸计算和错位定位必须共享该返回值。

- [ ] **Step 5: 将初始文本接入隐藏窗口创建**

将 `build_window` 签名和函数开头改为：

```rust
async fn build_window(
    app: &AppHandle,
    label: &str,
    ordinal: usize,
    text: &str,
) -> Result<(), String> {
    let app = app.clone();
    let label = label.to_string();
    let text = text.to_string();
    let (sender, receiver) = tokio::sync::oneshot::channel();
```

将主线程闭包中的 `result` 构造替换为：

```rust
let result = WebviewWindowBuilder::new(&app, &label, reference_card_url())
    .title(REFERENCE_CARD_TITLE)
    .inner_size(
        REFERENCE_CARD_DEFAULT_WIDTH,
        REFERENCE_CARD_DEFAULT_HEIGHT,
    )
    .min_inner_size(
        REFERENCE_CARD_MIN_WIDTH,
        REFERENCE_CARD_MIN_HEIGHT,
    )
    .decorations(false)
    .resizable(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .focused(false)
    .visible(false)
    .build()
    .map_err(|error| format!("创建参考卡窗口失败: {error}"))
    .and_then(|window| configure_initial_geometry(&window, &text, ordinal));
let _ = sender.send(result);
```

在 `ShowReservation::Create` 分支中，把：

```rust
if let Err(error) = build_window(&app, &label, ordinal).await {
```

改为：

```rust
if let Err(error) = build_window(&app, &label, ordinal, &text).await {
```

不要在 `reference_card_ready`、初始化事件监听或正文编辑事件中增加尺寸调用。

- [ ] **Step 6: 运行定向测试并确认 GREEN**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml reference_card -- --nocapture
pnpm --filter @lazycat/desktop test -- src/components/ReferenceCard.contract.test.ts
```

Expected:

- Rust `reference_card` 测试全部 PASS。
- Vitest `ReferenceCard.contract.test.ts` 全部 PASS。
- 输出中没有 Rust 编译错误、Vitest warning 或未处理异常。

- [ ] **Step 7: 提交窗口接入**

```powershell
git add apps/desktop/src-tauri/src/reference_card/mod.rs apps/desktop/src/components/ReferenceCard.contract.test.ts
git commit -m "feat(reference-card): 首次打开按内容调整窗口"
```

### Task 3: 完整验证与差异审查

**Files:**

- Verify: `apps/desktop/src-tauri/src/reference_card/size.rs`
- Verify: `apps/desktop/src-tauri/src/reference_card/mod.rs`
- Verify: `apps/desktop/src/components/ReferenceCard.contract.test.ts`

- [ ] **Step 1: 运行参考卡前后端定向测试**

Run:

```powershell
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml reference_card -- --nocapture
pnpm --filter @lazycat/desktop test -- src/components/ReferenceCard.contract.test.ts
```

Expected: 全部 PASS，输出无 warning。

- [ ] **Step 2: 运行全工作区类型检查**

Run:

```powershell
pnpm typecheck
```

Expected: exit code 0，无 TypeScript 类型错误。

- [ ] **Step 3: 构建渲染层**

Run:

```powershell
pnpm --filter @lazycat/desktop build:web
```

Expected: exit code 0，Vite 构建完成；不出现公网 CDN 依赖。

- [ ] **Step 4: 审查提交差异和空白错误**

Run:

```powershell
git diff c663e67..HEAD -- apps/desktop/src-tauri/src/reference_card/size.rs apps/desktop/src-tauri/src/reference_card/mod.rs apps/desktop/src/components/ReferenceCard.contract.test.ts
git diff --check
git status --short
```

Expected:

- 差异只包含尺寸纯函数、隐藏窗口首次尺寸接入和对应契约测试。
- `git diff --check` 无输出。
- `git status --short` 无未提交任务改动。

- [ ] **Step 5: 执行完成前验证审查**

使用 `superpowers:verification-before-completion`，核对实际命令输出后再声明完成。重点确认：

- 自适应只在 `build_window` 隐藏阶段调用一次。
- 最大自适应宽高均为目标显示器逻辑工作区的 30%。
- `360×220` 最小尺寸优先。
- 未设置 `max_inner_size`，用户可继续手动调整。
- 无 Vue、IPC、设置或持久化改动。
