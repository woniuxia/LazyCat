//! 文本对比 demo —— gpui + gpui-component
//!
//! 左右两栏输入文本，点击「对比」后以双栏并排视图展示差异：
//! 左栏 = 文本 A（红色 = 删除行），右栏 = 文本 B（绿色 = 新增行），
//! 修改行左右对齐显示；单侧独有的行在另一侧显示灰色占位。

mod diff;

use diff::{DiffStats, HalfLine, PairRow, RowKind, compute_rows, pair_rows, summarize};

use gpui::*;
use gpui_component::{
    ActiveTheme, Root, Theme, ThemeMode,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    v_flex,
};
use gpui_component_assets::Assets;

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

// ---------- 主题调色板快照 ----------

/// 渲染前从全局主题一次性取出的语义色。
/// 说明：唯一保留的固定像素是「等宽行号列宽度」与「1px 分隔线」，
/// 属于对齐网格的物理边界例外；其余颜色一律来自 cx.theme()。
struct Palette {
    fg: Hsla,
    muted: Hsla,
    border: Hsla,
    panel_bg: Hsla,
    ins_tint: Hsla,
    del_tint: Hsla,
    ins_accent: Hsla,
    del_accent: Hsla,
    warning: Hsla,
}

fn with_alpha(c: Hsla, a: f32) -> Hsla {
    Hsla { a, ..c }
}

impl Palette {
    fn from_theme(cx: &App) -> Self {
        let t = cx.theme();
        Self {
            fg: t.foreground,
            muted: t.muted_foreground,
            border: t.border,
            panel_bg: t.secondary,
            // 新增/删除底色：对应语义色的低透明度版本
            ins_tint: with_alpha(t.success, 0.14),
            del_tint: with_alpha(t.danger, 0.12),
            ins_accent: t.success,
            del_accent: t.danger,
            warning: t.warning,
        }
    }
}

/// 渲染前把需要的状态复制出来，避免构造元素时与 cx 相互借用。
struct Snapshot {
    left_lines: usize,
    right_lines: usize,
    stats_line: String,
    palette: Palette,
}

/// 渲染用结果数据
struct DiffResult {
    pairs: Vec<PairRow>,
    stats: DiffStats,
    fast_mode: bool,
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
            palette: Palette::from_theme(cx),
        }
    }

    fn do_compare(&mut self, cx: &mut Context<Self>) {
        let a = self.left.read(cx).value().to_string();
        let b = self.right.read(cx).value().to_string();
        let (rows, fast_mode) = compute_rows(&a, &b);
        let stats = summarize(&rows);
        let pairs = pair_rows(&rows);
        self.result = Some(DiffResult {
            pairs,
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
        let rendered_pairs: Vec<PairRow> = self
            .result
            .as_ref()
            .map(|r| r.pairs.clone())
            .unwrap_or_default();
        let fast_mode = self.result.as_ref().map_or(false, |r| r.fast_mode);
        let has_result = self.result.is_some();
        let identical = has_result && !self.result.as_ref().map_or(true, |r| r.stats.has_changes());
        let empty_diff = has_result && rendered_pairs.is_empty();

        v_flex()
            .id("page")
            .size_full()
            .overflow_y_scroll()
            .bg(snap.palette.panel_bg)
            .text_color(snap.palette.fg)
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
                            .text_xs()
                            .text_color(snap.palette.muted)
                            .child(snap.stats_line),
                    ),
            )
            // 双栏输入
            .child(
                h_flex()
                    .w_full()
                    .gap_3()
                    .items_start()
                    .child(render_input_column(
                        "文本 A",
                        &self.left,
                        snap.left_lines,
                        &snap.palette,
                    ))
                    .child(render_input_column(
                        "文本 B",
                        &self.right,
                        snap.right_lines,
                        &snap.palette,
                    )),
            )
            // 结果区
            .child(render_result(
                has_result,
                identical,
                empty_diff,
                fast_mode,
                &rendered_pairs,
                &snap.palette,
            ))
    }
}

// ---------- 元素构建 ----------

fn render_input_column(
    title: &str,
    state: &Entity<InputState>,
    lines: usize,
    pal: &Palette,
) -> Div {
    v_flex()
        .flex_1()
        .gap_1p5()
        .child(
            h_flex()
                .gap_2()
                .items_baseline()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(pal.muted)
                        .child(format!("{lines} 行")),
                ),
        )
        .child(Input::new(state))
}

/// 并排一行：左半 | 分隔线 | 右半
fn render_pair(row: &PairRow, pal: &Palette) -> Div {
    h_flex()
        .w_full()
        .overflow_hidden()
        .font_family("Consolas")
        .text_sm()
        .child(half_cell(row.left.as_ref(), pal))
        .child(
            // 物理分隔线：0.5 级别的细线属于对齐网格例外
            div().w(px(1.)).bg(pal.border),
        )
        .child(half_cell(row.right.as_ref(), pal))
}

/// 半行：None 表示空白占位（对面是删除/新增行）
fn half_cell(half: Option<&HalfLine>, pal: &Palette) -> Div {
    let base = h_flex().flex_1().overflow_hidden();

    let Some(h) = half else {
        return base.bg(pal.panel_bg);
    };

    let cell = match h.kind {
        RowKind::Equal => base,
        RowKind::Insert => base.bg(pal.ins_tint),
        RowKind::Delete => base.bg(pal.del_tint),
    };
    let accent = match h.kind {
        RowKind::Equal => pal.muted,
        RowKind::Insert => pal.ins_accent,
        RowKind::Delete => pal.del_accent,
    };
    let sign = match h.kind {
        RowKind::Equal => " ",
        RowKind::Insert => "+",
        RowKind::Delete => "-",
    };

    cell.child(line_no(h.no, pal))
        .child(div().w(px(12.)).text_color(accent).child(sign.to_string()))
        .child(
            div()
                .flex_1()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_color(match h.kind {
                    RowKind::Equal => pal.muted,
                    _ => pal.fg,
                })
                .child(if h.text.is_empty() {
                    "·".to_string()
                } else {
                    h.text.clone()
                }),
        )
}

/// 等宽行号列：固定宽度是对齐网格的物理边界例外
fn line_no(no: usize, pal: &Palette) -> Div {
    div()
        .w(px(32.))
        .text_align(TextAlign::Right)
        .text_xs()
        .text_color(pal.muted)
        .child(format!("{no:>3} "))
}

fn pill(text: &str, color: Hsla) -> Div {
    div()
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(color)
        .text_xs()
        .text_color(color)
        .child(text.to_string())
}

#[allow(clippy::too_many_arguments)]
fn render_result(
    has_result: bool,
    identical: bool,
    empty_diff: bool,
    fast_mode: bool,
    pairs: &[PairRow],
    pal: &Palette,
) -> Div {
    if !has_result {
        return placeholder_box("尚未对比 —— 点击上方「对比」按钮查看差异", pal);
    }

    // 先收集徽标，再一次性构建头部，避免可变重绑定
    let mut badges: Vec<Div> = Vec::new();
    if identical {
        badges.push(pill("内容一致", pal.ins_accent));
    } else {
        badges.push(pill("+ 新增", pal.ins_accent));
        badges.push(pill("- 删除", pal.del_accent));
    }

    let mut header = h_flex()
        .w_full()
        .gap_2()
        .items_center()
        .px_3()
        .py_2()
        .bg(pal.panel_bg)
        .child(div().text_xs().text_color(pal.muted).child("差异"))
        .children(badges);
    if fast_mode {
        header = header.child(
            div()
                .text_xs()
                .text_color(pal.warning)
                .child("文本过长，已使用快速对齐模式"),
        );
    }

    let container = v_flex()
        .w_full()
        .border_1()
        .border_color(pal.border)
        .rounded_lg()
        .overflow_hidden()
        .bg(pal.border) // 用边框色垫底，让两侧半行的间隙呈现分隔效果
        .child(header);

    if empty_diff {
        return container.child(empty_hint("两段文本均为空", pal));
    }

    let list = v_flex()
        .w_full()
        .py_1()
        .children(pairs.iter().map(|p| render_pair(p, pal)));
    container.child(list)
}

fn placeholder_box(text: &str, pal: &Palette) -> Div {
    h_flex()
        .w_full()
        .h(px(72.))
        .border_1()
        .border_color(pal.border)
        .rounded_lg()
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(pal.muted)
        .child(text.to_string())
}

fn empty_hint(text: &str, pal: &Palette) -> Div {
    h_flex()
        .w_full()
        .h(px(48.))
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(pal.muted)
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
