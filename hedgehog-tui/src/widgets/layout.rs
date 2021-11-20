use tui::layout::Rect;

pub(super) fn split_right(rect: Rect, width: u16) -> (Rect, Rect) {
    let width = width.min(rect.width);
    (
        Rect::new(rect.x, rect.y, rect.width - width, rect.height),
        Rect::new(rect.x + rect.width - width, rect.y, width, rect.height),
    )
}

pub(super) fn split_left(rect: Rect, width: u16) -> (Rect, Rect) {
    let width = width.min(rect.width);
    (
        Rect::new(rect.x, rect.y, width, rect.height),
        Rect::new(rect.x + width, rect.y, rect.width - width, rect.height),
    )
}

#[cfg(test)]
mod tests {
    use super::{split_left, split_right};
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
}
