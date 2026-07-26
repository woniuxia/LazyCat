#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalRect {
    pub(crate) x: i32,
    pub(crate) y: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PhysicalSize {
    pub(crate) width: i32,
    pub(crate) height: i32,
}
pub(crate) fn card_position(
    work_area: PhysicalRect,
    window: PhysicalSize,
    ordinal: usize,
) -> (i32, i32) {
    let remaining_x = (i64::from(work_area.width) - i64::from(window.width)).max(0);
    let remaining_y = (i64::from(work_area.height) - i64::from(window.height)).max(0);
    let offset = ordinal.min(5) as i64 * 28;
    let relative_x = (remaining_x * 2 / 3 + offset).clamp(0, remaining_x);
    let relative_y = (remaining_y / 3 + offset).clamp(0, remaining_y);
    let x = (i64::from(work_area.x) + relative_x).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let y = (i64::from(work_area.y) + relative_y).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::{card_position, PhysicalRect, PhysicalSize};
    fn is_visible(area: PhysicalRect, size: PhysicalSize, position: (i32, i32)) -> bool {
        position.0 >= area.x
            && position.1 >= area.y
            && position.0 + size.width <= area.x + area.width
            && position.1 + size.height <= area.y + area.height
    }
    #[test]
    fn negative_monitor_positions_are_visible_and_staggered() {
        let area = PhysicalRect {
            x: -1920,
            y: -1040,
            width: 1920,
            height: 1040,
        };
        let size = PhysicalSize {
            width: 560,
            height: 360,
        };
        let first = card_position(area, size, 0);
        let sixth = card_position(area, size, 5);
        assert!(is_visible(area, size, first));
        assert!(is_visible(area, size, sixth));
        assert_ne!(first, sixth);
    }
    #[test]
    fn smaller_work_area_returns_safe_origin_without_panicking() {
        let area = PhysicalRect {
            x: -300,
            y: -200,
            width: 320,
            height: 180,
        };
        let size = PhysicalSize {
            width: 560,
            height: 360,
        };
        assert_eq!(card_position(area, size, 5), (area.x, area.y));
    }

    #[test]
    fn insufficient_width_only_falls_back_on_x_axis() {
        let area = PhysicalRect {
            x: -300,
            y: 100,
            width: 320,
            height: 900,
        };
        let size = PhysicalSize {
            width: 560,
            height: 360,
        };
        assert_eq!(card_position(area, size, 0), (area.x, 280));
    }

    #[test]
    fn insufficient_height_only_falls_back_on_y_axis() {
        let area = PhysicalRect {
            x: 100,
            y: -200,
            width: 1920,
            height: 180,
        };
        let size = PhysicalSize {
            width: 560,
            height: 360,
        };
        assert_eq!(card_position(area, size, 0), (1006, area.y));
    }

    #[test]
    fn extreme_i32_dimensions_do_not_panic() {
        let area = PhysicalRect {
            x: i32::MAX,
            y: i32::MIN,
            width: i32::MAX,
            height: i32::MAX,
        };
        let size = PhysicalSize {
            width: 0,
            height: 0,
        };
        let position = card_position(area, size, usize::MAX);
        assert_eq!(position.0, i32::MAX);
        assert!(position.1 >= i32::MIN);
    }

    #[test]
    fn large_remaining_width_uses_exact_two_thirds_position() {
        let area = PhysicalRect {
            x: -1_000_000_000,
            y: 0,
            width: i32::MAX,
            height: 360,
        };
        let size = PhysicalSize {
            width: 0,
            height: 360,
        };
        assert_eq!(card_position(area, size, 0).0, 431_655_764);
    }
}
