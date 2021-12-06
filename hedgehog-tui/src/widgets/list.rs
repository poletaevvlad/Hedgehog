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
    item_height: u16,
}

impl<F, I> List<F, I> {
    pub(crate) fn new(delegate: F, items: I) -> Self {
        List {
            delegate,
            items,
            item_height: 1,
        }
    }

    pub(crate) fn item_height(mut self, item_height: u16) -> Self {
        self.item_height = item_height;
        self
    }
}

impl<'a, F: ListItemRenderingDelegate<'a>, I: IntoIterator<Item = F::Item>> Widget for List<F, I> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut iterator = self.items.into_iter();
        let mut y = area.top();
        while y < area.bottom() {
            match iterator.next() {
                Some(item) => {
                    self.delegate.render_item(
                        Rect::new(area.x, y, area.width, self.item_height),
                        item,
                        buf,
                    );
                }
                None => {
                    self.delegate.render_empty(
                        Rect::new(area.x, y, area.width, area.height - (y - area.y)),
                        buf,
                    );
                    break;
                }
            }
            y += self.item_height;
        }
    }
}
