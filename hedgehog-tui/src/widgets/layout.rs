use tui::layout::Rect;

pub(crate) fn split_right(rect: Rect, width: u16) -> (Rect, Rect) {
    let width = width.min(rect.width);
    (
        Rect::new(rect.x, rect.y, rect.width - width, rect.height),
        Rect::new(rect.x + rect.width - width, rect.y, width, rect.height),
    )
}

pub(crate) fn split_left(rect: Rect, width: u16) -> (Rect, Rect) {
    let width = width.min(rect.width);
    (
        Rect::new(rect.x, rect.y, width, rect.height),
        Rect::new(rect.x + width, rect.y, rect.width - width, rect.height),
    )
}

pub(crate) fn split_bottom(rect: Rect, height: u16) -> (Rect, Rect) {
    let height = height.min(rect.height);
    (
        Rect::new(rect.x, rect.y, rect.width, rect.height - height),
        Rect::new(rect.x, rect.y + rect.height - height, rect.width, height),
    )
}

#[cfg(test)]
mod tests {
    use super::{split_bottom, split_left, split_right};
    use tui::layout::Rect;

    #[test]
    fn split_right_normal() {
        let original = Rect::new(2, 3, 10, 2);
        let (left, right) = split_right(original, 4);
        assert_eq!(left, Rect::new(2, 3, 6, 2));
        assert_eq!(right, Rect::new(8, 3, 4, 2));
    }

    #[test]
    fn split_left_normal() {
        let original = Rect::new(2, 3, 10, 2);
        let (left, right) = split_left(original, 4);
        assert_eq!(left, Rect::new(2, 3, 4, 2));
        assert_eq!(right, Rect::new(6, 3, 6, 2));
    }

    #[test]
    fn split_right_full_width() {
        let original = Rect::new(2, 3, 10, 2);
        let (left, right) = split_right(original, 10);
        assert_eq!(right, original);
        assert_eq!(left, Rect::new(2, 3, 0, 2));
    }

    #[test]
    fn split_left_full_width() {
        let original = Rect::new(2, 3, 10, 2);
        let (left, right) = split_left(original, 10);
        assert_eq!(left, original);
        assert_eq!(right, Rect::new(12, 3, 0, 2));
    }

    #[test]
    fn split_right_wider() {
        let original = Rect::new(2, 3, 10, 2);
        let (left, right) = split_right(original, 15);
        assert_eq!(right, original);
        assert_eq!(left, Rect::new(2, 3, 0, 2));
    }

    #[test]
    fn split_left_wider() {
        let original = Rect::new(2, 3, 10, 2);
        let (left, right) = split_left(original, 15);
        assert_eq!(left, original);
        assert_eq!(right, Rect::new(12, 3, 0, 2));
    }

    #[test]
    fn split_bottom_normat() {
        let original = Rect::new(2, 3, 10, 8);
        let (top, bottom) = split_bottom(original, 3);
        assert_eq!(top, Rect::new(2, 3, 10, 5));
        assert_eq!(bottom, Rect::new(2, 8, 10, 3));
    }

    #[test]
    fn split_bottom_full_height() {
        let original = Rect::new(2, 3, 10, 8);
        let (top, bottom) = split_bottom(original, 8);
        assert_eq!(top, Rect::new(2, 3, 10, 0));
        assert_eq!(bottom, original);
    }

    #[test]
    fn split_bottom_higher() {
        let original = Rect::new(2, 3, 10, 8);
        let (top, bottom) = split_bottom(original, 12);
        assert_eq!(top, Rect::new(2, 3, 10, 0));
        assert_eq!(bottom, original);
    }
}
