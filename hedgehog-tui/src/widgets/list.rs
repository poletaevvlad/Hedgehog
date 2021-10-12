use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::widgets::Widget;

pub(crate) trait ListItemFactory {
    type Item;
    type Widget: Widget;

    fn create_widget(&self, item: Self::Item) -> Self::Widget;
}

pub(crate) struct List<F, I> {
    factory: F,
    items: I,
}

impl<F, I> List<F, I> {
    pub(crate) fn new(factory: F, items: I) -> Self {
        List { factory, items }
    }
}

impl<F: ListItemFactory, I: IntoIterator<Item = F::Item>> Widget for List<F, I> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let mut iterator = self.items.into_iter();
        for y in area.top()..=area.bottom() {
            match iterator.next() {
                Some(item) => {
                    let widget = self.factory.create_widget(item);
                    widget.render(Rect::new(area.x, y, area.width, 1), buf);
                }
                None => break,
            }
        }
    }
}
