//! 文本对比 demo —— gpui + gpui-component
//!
//! 左右两栏输入文本，点击「对比」后按行展示差异：
//! 红色 = 删除行（仅左侧有），绿色 = 新增行（仅右侧有）。

mod diff;

use diff::{DiffRow, DiffStats, RowKind, compute_rows, summarize};

use gpui::*;
use gpui_component::{
    button::{Button, ButtonVariants},
    input::{Input, InputState},
    *,
};
use gpui_component_assets::Assets;

// ---------- 配色（浅色主题专用） ----------

const FG: u32 = 0x1f_29_37;
const FG_MUTED: u32 = 0x6b_72_80;
const FG_FAINT: u32 = 0x9c_a3_af;
const BORDER: u32 = 0xe5_e7_eb;
const PANEL_BG: u32 = 0xf9_fa_fb;
const INS_BG: u32 = 0xec_fd_f3;
const DEL_BG: u32 = 0xfe_f2_f2;
const INS_ACCENT: u32 = 0x16_a3_4a;
const DEL_ACCENT: u32 = 0xdc_26_26;
const WARN: u32 = 0xb4_53_09;

// ---------- 示例文本 ----------

const SAMPLE_A: &str = "\
LazyCat 工具箱
版本：1.2.0

核心工具：
- JSON 格式化
- Base64 编解码
- 时间戳转换

平台支持：Windows
许可证：MIT
备注：示例文本";

const SAMPLE_B: &str = "\
LazyCat 工具箱
版本：1.3.0

核心工具：
- JSON 格式化
- 文本对比（新增）
- Base64 编解码
- 时间戳转换
- 正则测试

平台支持：Windows、macOS
许可证：MIT";

// ---------- 结果与快照 ----------

struct DiffResult {
    rows: Vec<DiffRow>,
    stats: DiffStats,
    fast_mode: bool,
}

/// 渲染前把需要的状态复制出来，避免构造元素时与 cx 相互借用。
struct Snapshot {
    left_lines: usize,
    right_lines: usize,
    stats_line: String,
}

struct RenderRow {
    kind: RowKind,
    a_no: Option<usize>,
    b_no: Option<usize>,
    text: String,
}

impl RenderRow {
    fn from_ref(r: &DiffRow) -> Self {
        Self {
            kind: r.kind,
            a_no: r.a_no,
            b_no: r.b_no,
            text: r.text.clone(),
        }
    }
}

// ---------- 视图 ----------

struct DiffDemo {
    left: Entity<InputState>,
    right: Entity<InputState>,
    result: Option<DiffResult>,
}

impl DiffDemo {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let left = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(14)
                .placeholder("输入左侧文本 A…")
                .default_value(SAMPLE_A)
        });
        let right = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .rows(14)
                .placeholder("输入右侧文本 B…")
                .default_value(SAMPLE_B)
        });
        Self {
            left,
            right,
            result: None,
        }
    }

    fn snapshot(&self, cx: &App) -> Snapshot {
        let left_text = self.left.read(cx).value();
        let right_text = self.right.read(cx).value();
        let stats_line = match &self.result {
            Some(r) if !r.stats.has_changes() => "两段文本完全一致".to_string(),
            Some(r) => format!(
                "+{} 新增 · -{} 删除 · {} 相同",
                r.stats.inserted, r.stats.deleted, r.stats.equal
            ),
            None => "点击「对比」查看两段文本的差异".to_string(),
        };
        Snapshot {
            left_lines: left_text.lines().count(),
            right_lines: right_text.lines().count(),
            stats_line,
        }
    }
}

impl DiffDemo {
    fn do_compare(&mut self, cx: &mut Context<Self>) {
        let a = self.left.read(cx).value().to_string();
        let b = self.right.read(cx).value().to_string();
        let (rows, fast_mode) = compute_rows(&a, &b);
        let stats = summarize(&rows);
        self.result = Some(DiffResult {
            rows,
            stats,
            fast_mode,
        });
        cx.notify();
    }

    fn do_swap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let a = self.left.read(cx).value().to_string();
        let b = self.right.read(cx).value().to_string();
        self.left
            .update(cx, |state, cx| state.set_value(b, window, cx));
        self.right
            .update(cx, |state, cx| state.set_value(a, window, cx));
        self.result = None;
        cx.notify();
    }

    fn load_sample(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.left
            .update(cx, |state, cx| state.set_value(SAMPLE_A, window, cx));
        self.right
            .update(cx, |state, cx| state.set_value(SAMPLE_B, window, cx));
        self.result = None;
        cx.notify();
    }
}

impl Render for DiffDemo {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snap = self.snapshot(cx);
        let rendered_rows: Vec<RenderRow> = match &self.result {
            Some(r) => r.rows.iter().map(RenderRow::from_ref).collect(),
            None => Vec::new(),
        };
        let fast_mode = self.result.as_ref().map_or(false, |r| r.fast_mode);
        let has_result = self.result.is_some();
        let identical = has_result && !self.result.as_ref().map_or(true, |r| r.stats.has_changes());
        let empty_diff = has_result && rendered_rows.is_empty();

        v_flex()
            .id("page")
            .size_full()
            .overflow_y_scroll()
            .bg(rgb(0xff_ff_ff))
            .text_color(rgb(FG))
            .p_4()
            .gap_3()
            // 工具栏
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new("compare").primary().label("对比").on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| this.do_compare(cx)),
                        ),
                    )
                    .child(Button::new("swap").label("交换 A/B").on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| this.do_swap(window, cx)),
                    ))
                    .child(Button::new("sample").label("载入示例").on_click(
                        cx.listener(|this, _: &ClickEvent, window, cx| {
                            this.load_sample(window, cx)
                        }),
                    ))
                    .child(div().flex_1())
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(rgb(FG_MUTED))
                            .child(snap.stats_line),
                    ),
            )
            // 双栏输入
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .items_start()
                    .child(render_input_column("文本 A", &self.left, snap.left_lines))
                    .child(render_input_column("文本 B", &self.right, snap.right_lines)),
            )
            // 结果区
            .child(render_result(
                has_result,
                identical,
                empty_diff,
                fast_mode,
                &rendered_rows,
            ))
    }
}

// ---------- 元素构建 ----------

fn render_input_column(title: &str, state: &Entity<InputState>, lines: usize) -> Div {
    v_flex()
        .flex_1()
        .gap_1p5()
        .child(
            h_flex()
                .gap_2()
                .items_baseline()
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{title}")),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(FG_FAINT))
                        .child(format!("{lines} 行")),
                ),
        )
        .child(Input::new(state))
}

fn render_row(row: &RenderRow) -> Div {
    let (bg, accent) = match row.kind {
        RowKind::Equal => (None, rgb(FG)),
        RowKind::Insert => (Some(rgb(INS_BG)), rgb(INS_ACCENT)),
        RowKind::Delete => (Some(rgb(DEL_BG)), rgb(DEL_ACCENT)),
    };
    let sign = match row.kind {
        RowKind::Equal => " ",
        RowKind::Insert => "+",
        RowKind::Delete => "-",
    };

    let base = h_flex()
        .w_full()
        .font_family("Consolas")
        .text_size(px(12.5));
    let base = match bg {
        Some(bg) => base.bg(bg),
        None => base,
    };

    base.child(num_cell(row.a_no))
        .child(num_cell(row.b_no))
        .child(div().w(px(14.)).text_color(accent).child(sign.to_string()))
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_color(match row.kind {
                    RowKind::Equal => rgb(FG_MUTED),
                    _ => rgb(FG),
                })
                .child(if row.text.is_empty() {
                    "·".to_string()
                } else {
                    row.text.clone()
                }),
        )
}

fn num_cell(no: Option<usize>) -> Div {
    let label = match no {
        Some(n) => format!("{n:>4} "),
        None => "     ".to_string(),
    };
    div()
        .w(px(34.))
        .text_align(TextAlign::Right)
        .text_size(px(11.5))
        .text_color(rgb(FG_FAINT))
        .child(label)
}

fn pill(text: &str, color: u32) -> Div {
    div()
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(color))
        .text_size(px(11.))
        .text_color(rgb(color))
        .child(text.to_string())
}

#[allow(clippy::too_many_arguments)]
fn render_result(
    has_result: bool,
    identical: bool,
    empty_diff: bool,
    fast_mode: bool,
    rows: &[RenderRow],
) -> Div {
    if !has_result {
        return placeholder_box("尚未对比 —— 点击上方「对比」按钮查看差异");
    }

    let mut header = h_flex()
        .w_full()
        .gap_2()
        .items_center()
        .px_3()
        .py_2()
        .bg(rgb(PANEL_BG))
        .child(
            div()
                .text_size(px(12.))
                .text_color(rgb(FG_MUTED))
                .child("差异"),
        );
    if identical {
        header = header.child(pill("内容一致", INS_ACCENT));
    } else {
        header = header
            .child(pill("+ 新增", INS_ACCENT))
            .child(pill("- 删除", DEL_ACCENT));
    }
    if fast_mode {
        header = header.child(
            div()
                .text_size(px(11.))
                .text_color(rgb(WARN))
                .child("文本过长，已使用快速对齐模式"),
        );
    }

    let container = v_flex()
        .w_full()
        .border_1()
        .border_color(rgb(BORDER))
        .rounded_lg()
        .overflow_hidden()
        .child(header);

    if empty_diff {
        return container.child(empty_hint("两段文本均为空"));
    }

    let list = v_flex()
        .w_full()
        .py_1()
        .children(rows.iter().map(render_row));
    container.child(list)
}

fn placeholder_box(text: &str) -> Div {
    h_flex()
        .w_full()
        .h(px(72.))
        .border_1()
        .border_color(rgb(BORDER))
        .rounded_lg()
        .items_center()
        .justify_center()
        .text_size(px(13.))
        .text_color(rgb(FG_FAINT))
        .child(text.to_string())
}

fn empty_hint(text: &str) -> Div {
    h_flex()
        .w_full()
        .h(px(48.))
        .items_center()
        .justify_center()
        .text_size(px(13.))
        .text_color(rgb(FG_MUTED))
        .child(text.to_string())
}

// ---------- 应用入口 ----------

fn main() {
    Application::new().with_assets(Assets).run(move |cx| {
        gpui_component::init(cx);
        Theme::change(ThemeMode::Light, None, cx);

        let bounds = Bounds::centered(None, size(px(1080.), px(760.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("文本对比 · gpui demo".into()),
                appears_transparent: false,
                ..Default::default()
            }),
            ..Default::default()
        };

        cx.spawn(async move |cx| {
            cx.open_window(options, |window, cx| {
                let view = cx.new(|cx| DiffDemo::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })?;
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
