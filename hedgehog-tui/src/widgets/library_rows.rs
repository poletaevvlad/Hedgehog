use super::list::ListItemRenderingDelegate;
use crate::theming;
use hedgehog_library::model::{FeedId, FeedStatus, FeedSummary};
use std::collections::HashSet;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::Style;
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
    updating_feeds: &'t HashSet<FeedId>,
}

enum FeedsListStatusIndicator {
    Error,
    Loading,
}

impl FeedsListStatusIndicator {
    fn style(&self, theme: &theming::Theme, item_state: theming::ListState) -> Style {
        let subitem_selector = match self {
            FeedsListStatusIndicator::Error => theming::ListSubitem::ErrorIndicator,
            FeedsListStatusIndicator::Loading => theming::ListSubitem::LoadingIndicator,
        };
        theme.get(theming::List::Item(item_state, Some(subitem_selector)))
    }

    fn label(&self) -> &'static str {
        match self {
            FeedsListStatusIndicator::Error => "E",
            FeedsListStatusIndicator::Loading => "U",
        }
    }
}

impl<'t> FeedsListRowRenderer<'t> {
    pub(crate) fn new(
        theme: &'t theming::Theme,
        is_focused: bool,
        updating_feeds: &'t HashSet<FeedId>,
    ) -> Self {
        FeedsListRowRenderer {
            theme,
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
            Some(FeedsListStatusIndicator::Loading)
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

        match item {
            Some(item) => {
                if let Some(status_indicator) = self.get_status_indicator(item) {
                    let style = status_indicator.style(self.theme, item_state);
                    buf.set_style(
                        Rect::new(area.right().saturating_sub(3), area.y, 3, area.height),
                        style,
                    );
                    let label = status_indicator.label();
                    buf.set_string(
                        area.right().saturating_sub(2),
                        area.y,
                        label,
                        Style::default(),
                    );
                    area.width = area.width.saturating_sub(3);
                }

                let subitem = if !item.has_title {
                    Some(theming::ListSubitem::MissingTitle)
                } else {
                    None
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
            }
            None => buf.set_string(
                area.x,
                area.y,
                " . . . ",
                self.theme.get(theming::List::Item(item_state, None)),
            ),
        }
    }
}
