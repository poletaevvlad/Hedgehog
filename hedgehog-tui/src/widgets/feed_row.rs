use super::{layout::split_right, list::ListItemRenderingDelegate};
use crate::options::Options;
use crate::theming::{self, Theme};
use hedgehog_library::model::{FeedId, FeedStatus, FeedSummary};
use std::collections::HashSet;
use tui::buffer::Buffer;
use tui::layout::{Alignment, Rect};
use tui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

pub(crate) struct FeedsListRowRenderer<'t> {
    theme: &'t Theme,
    focused: bool,
    options: &'t Options,
    updating_feeds: &'t HashSet<FeedId>,
}

enum FeedsListStatusIndicator {
    Error,
    Update,
}

impl FeedsListStatusIndicator {
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
        focused: bool,
        updating_feeds: &'t HashSet<FeedId>,
    ) -> Self {
        FeedsListRowRenderer {
            theme,
            options,
            focused,
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

        if let Some(item) = item {
            let status_indicator = self.get_status_indicator(item);
            let item_selector = theming::ListItem {
                selected,
                focused: self.focused,
                missing_title: item.has_title,
                state: Some(match status_indicator {
                    Some(FeedsListStatusIndicator::Error) => theming::ListState::FeedError,
                    Some(FeedsListStatusIndicator::Update) => theming::ListState::FeedUpdating,
                    None => theming::ListState::Feed,
                }),
                column: None,
            };

            if let Some(status_indicator) = self.get_status_indicator(item) {
                let style = self.theme.get(theming::List::Item(
                    item_selector.with_column(theming::ListColumn::StateIndicator),
                ));
                let label = status_indicator.label(self.options);

                let (rest, indicator_area) = split_right(area, label.width() as u16);
                area = rest;
                buf.set_string(indicator_area.x, indicator_area.y, label, style);
            }

            let style = self.theme.get(theming::List::Item(
                item_selector.with_column(theming::ListColumn::Title),
            ));
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
            let item_selector = theming::ListItem {
                selected,
                focused: self.focused,
                column: Some(theming::ListColumn::Loading),
                ..Default::default()
            };
            let style = self.theme.get(theming::List::Item(item_selector));
            let paragraph = Paragraph::new(".  .  .")
                .style(style)
                .alignment(Alignment::Center);
            paragraph.render(area, buf);
        }
    }

    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let item_selector = theming::ListItem {
            focused: self.focused,
            ..Default::default()
        };
        let style = self.theme.get(theming::List::Item(item_selector));
        buf.set_style(area, style);
    }
}
