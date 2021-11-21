use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::widgets::Widget;

pub(crate) trait ListItemRenderingDelegate<'a> {
    type Item: 'a;

    fn render_item(&self, area: Rect, item: Self::Item, buf: &mut Buffer);
    fn render_empty(&self, area: Rect, buf: &mut Buffer);
}

pub(crate) struct List<F, I> {
    delegate: F,
    items: I,
}

impl<F, I> List<F, I> {
    pub(crate) fn new(delegate: F, items: I) -> Self {
        List { delegate, items }
    }
}

impl<'a, F: ListItemRenderingDelegate<'a>, I: IntoIterator<Item = F::Item>> Widget for List<F, I> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut iterator = self.items.into_iter();
        for y in area.top()..=area.bottom() {
            match iterator.next() {
                Some(item) => {
                    self.delegate
                        .render_item(Rect::new(area.x, y, area.width, 1), item, buf);
                }
                None => {
                    self.delegate.render_empty(
                        Rect::new(area.x, y, area.width, area.height - (y - area.y)),
                        buf,
                    );
                    break;
                }
            }
        }
    }
}
