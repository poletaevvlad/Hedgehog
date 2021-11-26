use super::{layout::split_right, list::ListItemRenderingDelegate};
use crate::options::Options;
use crate::theming::{self, Theme};
use hedgehog_library::model::{FeedId, FeedStatus, FeedSummary};
use std::collections::HashSet;
use tui::buffer::Buffer;
use tui::layout::{Alignment, Rect};
use tui::style::Style;
use tui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

pub(crate) struct FeedsListRowRenderer<'t> {
    theme: &'t Theme,
    default_item_state: theming::ListState,
    options: &'t Options,
    updating_feeds: &'t HashSet<FeedId>,
}

enum FeedsListStatusIndicator {
    Error,
    Update,
}

impl FeedsListStatusIndicator {
    fn style(&self, theme: &theming::Theme, item_state: theming::ListState) -> Style {
        let subitem_selector = match self {
            FeedsListStatusIndicator::Error => theming::ListSubitem::ErrorIndicator,
            FeedsListStatusIndicator::Update => theming::ListSubitem::UpdateIndicator,
        };
        theme.get(theming::List::Item(item_state, Some(subitem_selector)))
    }

    fn label<'a>(&self, options: &'a Options) -> &'a str {
        match self {
            FeedsListStatusIndicator::Error => options.label_feed_error.as_str(),
            FeedsListStatusIndicator::Update => options.label_feed_updating.as_str(),
        }
    }
}

impl<'t> FeedsListRowRenderer<'t> {
    pub(crate) fn new(
        theme: &'t theming::Theme,
        options: &'t Options,
        is_focused: bool,
        updating_feeds: &'t HashSet<FeedId>,
    ) -> Self {
        FeedsListRowRenderer {
            theme,
            options,
            default_item_state: if is_focused {
                theming::ListState::FOCUSED
            } else {
                theming::ListState::empty()
            },
            updating_feeds,
        }
    }

    fn get_status_indicator(&self, item: &FeedSummary) -> Option<FeedsListStatusIndicator> {
        if self.updating_feeds.contains(&item.id) {
            Some(FeedsListStatusIndicator::Update)
        } else if matches!(item.status, FeedStatus::Error(_)) {
            Some(FeedsListStatusIndicator::Error)
        } else {
            None
        }
    }
}

impl<'t, 'a> ListItemRenderingDelegate<'a> for FeedsListRowRenderer<'t> {
    type Item = (Option<&'a FeedSummary>, bool);

    fn render_item(&self, mut area: Rect, item: Self::Item, buf: &mut tui::buffer::Buffer) {
        let (item, selected) = item;

        let mut item_state = self.default_item_state;
        if selected {
            item_state |= theming::ListState::SELECTED;
        }

        if let Some(item) = item {
            if let Some(status_indicator) = self.get_status_indicator(item) {
                let style = status_indicator.style(self.theme, item_state);
                let label = status_indicator.label(self.options);

                let (rest, indicator_area) = split_right(area, label.width() as u16);
                area = rest;
                buf.set_string(indicator_area.x, indicator_area.y, label, style);
            }

            let subitem = match item.has_title {
                true => None,
                false => Some(theming::ListSubitem::MissingTitle),
            };
            let style = self.theme.get(theming::List::Item(item_state, subitem));
            buf.set_style(area, style);

            let paragraph = Paragraph::new(item.title.as_str());
            paragraph.render(
                Rect::new(
                    area.x + 1,
                    area.y,
                    area.width.saturating_sub(2),
                    area.height,
                ),
                buf,
            );
        } else {
            let style = self.theme.get(theming::List::Item(
                item_state,
                Some(theming::ListSubitem::LoadingIndicator),
            ));
            let paragraph = Paragraph::new(".  .  .")
                .style(style)
                .alignment(Alignment::Center);
            paragraph.render(area, buf);
        }
    }

    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let item_state = self.default_item_state;
        let style = self.theme.get(theming::List::Item(item_state, None));
        buf.set_style(area, style);
    }
}
