use super::list::ListItemRenderingDelegate;
use crate::theming;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::widgets::{Paragraph, Widget};

pub(crate) struct EpisodesListRowRenderer<'t> {
    theme: &'t theming::Theme,
    default_item_state: theming::ListState,
}

impl<'t> EpisodesListRowRenderer<'t> {
    pub(crate) fn new(theme: &'t theming::Theme, is_focused: bool) -> Self {
        EpisodesListRowRenderer {
            theme,
            default_item_state: if is_focused {
                theming::ListState::FOCUSED
            } else {
                theming::ListState::empty()
            },
        }
    }
}

impl<'t, 'a> ListItemRenderingDelegate<'a> for EpisodesListRowRenderer<'t> {
    type Item = (Option<&'a hedgehog_library::model::EpisodeSummary>, bool);

    fn render_item(&self, area: Rect, item: Self::Item, buf: &mut Buffer) {
        let (item, selected) = item;

        let mut item_state = self.default_item_state;
        if selected {
            item_state |= theming::ListState::SELECTED;
        }
        let subitem = match item.map(|item| item.title.is_some()) {
            Some(false) => Some(theming::ListSubitem::MissingTitle),
            _ => None,
        };
        let style = self.theme.get(theming::List::Item(item_state, subitem));

        buf.set_style(Rect::new(area.x, area.y, 1, area.height), style);
        buf.set_style(
            Rect::new(area.x + area.width - 1, area.y, 1, area.height),
            style,
        );

        let inner_area = Rect::new(area.x + 1, area.y, area.width - 2, area.height);
        match item {
            Some(item) => {
                let paragraph =
                    Paragraph::new(item.title.as_deref().unwrap_or("no title")).style(style);
                paragraph.render(inner_area, buf);
            }
            None => buf.set_string(area.x, area.y, " . . . ", style),
        }
    }
}

pub(crate) struct FeedsListRowRenderer<'t> {
    theme: &'t theming::Theme,
    default_item_state: theming::ListState,
}

impl<'t> FeedsListRowRenderer<'t> {
    pub(crate) fn new(theme: &'t theming::Theme, is_focused: bool) -> Self {
        FeedsListRowRenderer {
            theme,
            default_item_state: if is_focused {
                theming::ListState::FOCUSED
            } else {
                theming::ListState::empty()
            },
        }
    }
}

impl<'t, 'a> ListItemRenderingDelegate<'a> for FeedsListRowRenderer<'t> {
    type Item = (Option<&'a hedgehog_library::model::FeedSummary>, bool);

    fn render_item(&self, area: Rect, item: Self::Item, buf: &mut tui::buffer::Buffer) {
        let (item, selected) = item;

        let mut item_state = self.default_item_state;
        if selected {
            item_state |= theming::ListState::SELECTED;
        }
        let subitem = match item.map(|item| item.has_title) {
            Some(false) => Some(theming::ListSubitem::MissingTitle),
            _ => None,
        };
        let style = self.theme.get(theming::List::Item(item_state, subitem));

        buf.set_style(Rect::new(area.x, area.y, 1, area.height), style);
        buf.set_style(
            Rect::new(area.x + area.width - 1, area.y, 1, area.height),
            style,
        );

        let inner_area = Rect::new(area.x + 1, area.y, area.width - 2, area.height);
        match item {
            Some(item) => {
                let paragraph = Paragraph::new(item.title.as_str()).style(style);
                paragraph.render(inner_area, buf);
            }
            None => buf.set_string(area.x, area.y, " . . . ", style),
        }
    }
}
