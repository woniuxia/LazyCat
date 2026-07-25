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
    let remaining_x = work_area.width.saturating_sub(window.width);
    let remaining_y = work_area.height.saturating_sub(window.height);
    if remaining_x <= 0 || remaining_y <= 0 {
        return (work_area.x, work_area.y);
    }
    let offset = ordinal.min(5) as i32 * 28;
    let x = work_area.x.saturating_add(
        (remaining_x * 2 / 3)
            .saturating_add(offset)
            .clamp(0, remaining_x),
    );
    let y = work_area.y.saturating_add(
        (remaining_y / 3)
            .saturating_add(offset)
            .clamp(0, remaining_y),
    );
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
}
