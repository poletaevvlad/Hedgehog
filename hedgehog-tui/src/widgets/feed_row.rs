use super::{layout::split_right, list::ListItemRenderingDelegate};
use crate::options::Options;
use crate::theming::{self, Theme};
use hedgehog_library::model::{FeedId, FeedStatus, FeedSummary, FeedView, GroupSummary};
use std::collections::HashSet;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

pub(crate) struct FeedsListRowRenderer<'t> {
    theme: &'t Theme,
    focused: bool,
    options: &'t Options,
    updating_feeds: &'t HashSet<FeedId>,
    playing_feed: Option<FeedId>,
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
            playing_feed: None,
        }
    }

    pub(crate) fn playing(mut self, playing: impl Into<Option<FeedId>>) -> Self {
        self.playing_feed = playing.into();
        self
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
    type Item = (&'a FeedView<FeedSummary, GroupSummary>, bool);

    fn render_item(&self, mut area: Rect, item: Self::Item, buf: &mut tui::buffer::Buffer) {
        let (item, selected) = item;

        match item {
            FeedView::All | FeedView::New | FeedView::Group(_) => {
                let item_selector = theming::ListItem {
                    selected,
                    focused: self.focused,
                    missing_title: false,
                    state: Some(theming::ListState::FeedSpecial),
                    column: None,
                    playing: false,
                    hidden: false,
                };
                let style = self.theme.get(theming::List::Item(item_selector));
                buf.set_style(area, style);

                let paragraph = Paragraph::new(match item {
                    FeedView::All => "All episodes",
                    FeedView::New => "New",
                    FeedView::Group(group) => &group.name,
                    FeedView::Feed(_) => unreachable!(),
                });
                paragraph.render(
                    Rect::new(
                        area.x + 1,
                        area.y,
                        area.width.saturating_sub(2),
                        area.height,
                    ),
                    buf,
                );
            }
            FeedView::Feed(item) => {
                let status_indicator = self.get_status_indicator(item);
                let item_selector = theming::ListItem {
                    selected,
                    focused: self.focused,
                    missing_title: !item.has_title,
                    state: Some(match status_indicator {
                        Some(FeedsListStatusIndicator::Error) => theming::ListState::FeedError,
                        Some(FeedsListStatusIndicator::Update) => theming::ListState::FeedUpdating,
                        None => theming::ListState::Feed,
                    }),
                    column: None,
                    playing: self.playing_feed == Some(item.id),
                    hidden: false,
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

                if item.new_count > 0 {
                    let formatted = format!(" {} ", item.new_count);
                    let width = formatted.width();
                    let (title_area, count_area) = split_right(area, width as u16);
                    let style = self.theme.get(theming::List::Item(
                        item_selector.with_column(theming::ListColumn::NewCount),
                    ));
                    buf.set_string(count_area.x, count_area.y, formatted, style);
                    area = title_area;
                }

                let style = self.theme.get(theming::List::Item(
                    item_selector.with_column(theming::ListColumn::Title),
                ));
                buf.set_style(area, style);
                let paragraph = Paragraph::new(item.title.as_str());
                paragraph.render(
                    Rect::new(
                        area.x + 2,
                        area.y,
                        area.width.saturating_sub(3),
                        area.height,
                    ),
                    buf,
                );
            }
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
