#[allow(dead_code)]
pub(crate) const REFERENCE_CARD_DEFAULT_WIDTH: f64 = 560.0;
#[allow(dead_code)]
pub(crate) const REFERENCE_CARD_DEFAULT_HEIGHT: f64 = 360.0;
pub(crate) const REFERENCE_CARD_MIN_WIDTH: f64 = 360.0;
pub(crate) const REFERENCE_CARD_MIN_HEIGHT: f64 = 220.0;

const WORK_AREA_RATIO: f64 = 0.30;
const TAB_WIDTH: usize = 4;
const COLUMN_WIDTH: f64 = 8.0;
const LINE_HEIGHT: f64 = 19.0;
const HORIZONTAL_PADDING: f64 = 32.0;
const VERTICAL_PADDING: f64 = 24.0;
const TOOLBAR_HEIGHT: f64 = 38.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct CardSize {
    pub(crate) width: f64,
    pub(crate) height: f64,
}

pub(crate) fn adaptive_card_size(text: &str, work_area: CardSize) -> CardSize {
    let max_width = (work_area.width * WORK_AREA_RATIO).max(REFERENCE_CARD_MIN_WIDTH);
    let longest_line = text.split('\n').map(display_columns).max().unwrap_or(0) as f64;
    let desired_width = longest_line * COLUMN_WIDTH + HORIZONTAL_PADDING;
    let width = desired_width.max(REFERENCE_CARD_MIN_WIDTH).min(max_width);

    let columns_per_row = ((width - HORIZONTAL_PADDING) / COLUMN_WIDTH) as usize;
    let rows = wrapped_rows(text, columns_per_row) as f64;
    let desired_height = rows * LINE_HEIGHT + VERTICAL_PADDING + TOOLBAR_HEIGHT;
    let max_height = (work_area.height * WORK_AREA_RATIO).max(REFERENCE_CARD_MIN_HEIGHT);
    let height = desired_height
        .max(REFERENCE_CARD_MIN_HEIGHT)
        .min(max_height);

    CardSize { width, height }
}

fn display_columns(text: &str) -> usize {
    text.chars().fold(0, |columns, character| {
        if character == '\t' {
            columns + TAB_WIDTH - columns % TAB_WIDTH
        } else if character.is_ascii_control() {
            columns
        } else if character.is_ascii() {
            columns + 1
        } else {
            columns + 2
        }
    })
}

fn wrapped_rows(text: &str, columns_per_row: usize) -> usize {
    text.split('\n')
        .map(|line| display_columns(line).div_ceil(columns_per_row).max(1))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::{adaptive_card_size, display_columns, CardSize};

    fn size(width: f64, height: f64) -> CardSize {
        CardSize { width, height }
    }

    #[test]
    fn short_text_uses_minimum_size() {
        assert_eq!(
            adaptive_card_size("short", size(1920.0, 1080.0)),
            size(360.0, 220.0)
        );
    }

    #[test]
    fn sixty_ascii_columns_expand_width() {
        assert_eq!(
            adaptive_card_size(&"x".repeat(60), size(1920.0, 1080.0)),
            size(512.0, 220.0)
        );
    }

    #[test]
    fn twelve_lines_expand_height() {
        assert_eq!(
            adaptive_card_size(&vec!["line"; 12].join("\n"), size(1920.0, 1080.0)),
            size(360.0, 290.0)
        );
    }

    #[test]
    fn long_line_wraps_using_final_width() {
        assert_eq!(
            adaptive_card_size(&"x".repeat(700), size(1920.0, 1080.0)),
            size(576.0, 271.0)
        );
    }

    #[test]
    fn display_columns_handles_tabs_wide_characters_and_carriage_returns() {
        assert_eq!(display_columns("a\t中\r"), 6);
    }

    #[test]
    fn very_long_hundred_line_text_reaches_work_area_cap() {
        let text = vec!["x".repeat(2000); 100].join("\n");
        assert_eq!(
            adaptive_card_size(&text, size(1920.0, 1080.0)),
            size(576.0, 324.0)
        );
    }

    #[test]
    fn minimum_size_takes_priority_over_work_area_cap() {
        assert_eq!(
            adaptive_card_size(&"x".repeat(2000), size(800.0, 600.0)),
            size(360.0, 220.0)
        );
    }
}
