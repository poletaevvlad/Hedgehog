use super::list::ListItemRenderingDelegate;
use crate::options::Options;
use crate::theming;
use hedgehog_library::model::{EpisodeId, FeedId, FeedStatus, FeedSummary};
use std::collections::HashSet;
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::style::Style;
use tui::text::Span;
use tui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

pub(crate) struct EpisodesListRowRenderer<'t> {
    theme: &'t theming::Theme,
    default_item_state: theming::ListState,
    playing_id: Option<EpisodeId>,
    options: &'t Options,
}

impl<'t> EpisodesListRowRenderer<'t> {
    pub(crate) fn new(theme: &'t theming::Theme, is_focused: bool, options: &'t Options) -> Self {
        EpisodesListRowRenderer {
            theme,
            default_item_state: if is_focused {
                theming::ListState::FOCUSED
            } else {
                theming::ListState::empty()
            },
            playing_id: None,
            options,
        }
    }

    pub(crate) fn with_playing_id(mut self, playing_id: impl Into<Option<EpisodeId>>) -> Self {
        self.playing_id = playing_id.into();
        self
    }
}

impl<'t, 'a> ListItemRenderingDelegate<'a> for EpisodesListRowRenderer<'t> {
    type Item = (Option<&'a hedgehog_library::model::EpisodeSummary>, bool);

    fn render_item(&self, mut area: Rect, item: Self::Item, buf: &mut Buffer) {
        let (item, selected) = item;

        let mut item_state = self.default_item_state;
        if selected {
            item_state |= theming::ListState::SELECTED;
        }
        if item.is_some() && self.playing_id == item.map(|item| item.id) {
            item_state |= theming::ListState::ACTIVE;
        }

        match item {
            Some(item) => {
                let subitem = match item.title {
                    None => Some(theming::ListSubitem::MissingTitle),
                    _ => None,
                };

                let date_format = self.options.date_format.as_str();
                if !date_format.is_empty() {
                    if let Some(date) = item.publication_date {
                        let formatted = format!(" {} ", date.format(date_format));
                        let width = formatted.width() as u16;
                        buf.set_span(
                            area.right().saturating_sub(width),
                            area.y,
                            &Span::styled(
                                formatted,
                                self.theme.get(theming::List::Item(
                                    item_state,
                                    Some(theming::ListSubitem::Date),
                                )),
                            ),
                            width,
                        );
                        area.width = area.width.saturating_sub(width);
                    }
                }

                let style = self.theme.get(theming::List::Item(item_state, subitem));
                buf.set_style(area, style);
                let paragraph = Paragraph::new(item.title.as_deref().unwrap_or("Untitled"));
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
            None => {
                buf.set_string(
                    area.x,
                    area.y,
                    " . . . ",
                    self.theme.get(theming::List::Item(item_state, None)),
                );
            }
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
