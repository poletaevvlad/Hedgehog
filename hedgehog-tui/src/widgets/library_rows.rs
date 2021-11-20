use super::list::ListItemRenderingDelegate;
use super::utils::number_width;
use crate::options::Options;
use crate::theming;
use crate::widgets::utils::DurationFormatter;
use hedgehog_library::model::{
    EpisodeId, EpisodeSummaryStatus, EpisodesListMetadata, FeedId, FeedStatus, FeedSummary,
};
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

#[derive(Debug, PartialEq)]
pub(crate) struct EpisodesListSizing {
    date_width: u16,
    episode_number_width: u16,
    position_date: u16,
}

impl EpisodesListSizing {
    pub(crate) fn compute(_options: &Options, metadata: &EpisodesListMetadata) -> Self {
        let episode_number_width = match metadata.max_episode_number {
            Some(episode_number) => {
                let mut width = number_width(episode_number) + 2;
                if let Some(season_number) = metadata.max_season_number {
                    width += number_width(season_number) + 1;
                }
                width
            }
            None => 0,
        };
        EpisodesListSizing {
            date_width: 0,
            episode_number_width,
            position_date: 0,
        }
    }
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
        if item.map(|item| matches!(item.status, EpisodeSummaryStatus::New)) == Some(true) {
            item_state |= theming::ListState::NEW;
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

                if let Some(duration) = item.duration {
                    let formatted = format!(" {} ", DurationFormatter(duration));
                    let width = formatted.width() as u16;
                    buf.set_span(
                        area.right().saturating_sub(width),
                        area.y,
                        &Span::styled(
                            formatted,
                            self.theme.get(theming::List::Item(
                                item_state,
                                Some(theming::ListSubitem::Duration),
                            )),
                        ),
                        width,
                    );
                    area.width = area.width.saturating_sub(width);
                }

                let episode_number = match (item.season_number, item.episode_number) {
                    (None, Some(episode_number)) => Some(format!(" {}.", episode_number)),
                    (Some(season_number), Some(episode_number)) => {
                        Some(format!(" {}x{}.", season_number, episode_number))
                    }
                    _ => None,
                };
                if let Some(episode_number) = episode_number {
                    let width = episode_number.width() as u16;
                    buf.set_span(
                        area.x,
                        area.y,
                        &Span::styled(
                            episode_number,
                            self.theme.get(theming::List::Item(
                                item_state,
                                Some(theming::ListSubitem::EpisodeNumber),
                            )),
                        ),
                        width,
                    );
                    area.x += width;
                    area.width = area.width.saturating_sub(width);
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

                if matches!(item.status, EpisodeSummaryStatus::New) {
                    buf.set_string(
                        area.x,
                        area.y,
                        "*",
                        self.theme.get(theming::List::Item(
                            item_state,
                            Some(theming::ListSubitem::NewIndicator),
                        )),
                    );
                }
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

#[cfg(test)]
mod tests {
    mod episodes_list_sizing {
        use super::super::{number_width, EpisodesListSizing};
        use crate::options::Options;
        use hedgehog_library::model::EpisodesListMetadata;

        #[test]
        fn number_width_test() {
            fn test_number(number: i64) {
                let str_repr = number.to_string();
                let width = number_width(number);
                assert_eq!(width, str_repr.len() as u16, "{}", str_repr);
            }

            let powers_of_10 = [
                0,
                10,
                100,
                1000,
                10000,
                100000,
                1000000,
                10000000,
                100000000,
                1000000000,
                10000000000,
                100000000000,
                1000000000000,
                10000000000000,
                100000000000000,
                1000000000000000,
                10000000000000000,
                100000000000000000,
                1000000000000000000,
            ];

            for num in &powers_of_10 {
                test_number(*num);
                test_number(*num - 1);
                test_number(-*num);
                test_number(-(*num - 1));
            }
        }

        macro_rules! episode_number {
            ($($name:ident($max_ep_number:expr, $max_season_number:expr, $expected:literal)),*$(,)?) => {
                mod episode_number_width {
                    use super::*;

                    $(
                    #[test]
                    fn $name() {
                        let metadata = EpisodesListMetadata {
                            max_episode_number: $max_ep_number,
                            max_season_number: $max_season_number,
                            ..Default::default()
                        };
                        let options = Options::default();
                        let sizing = EpisodesListSizing::compute(&options, &metadata);
                        assert_eq!(sizing.episode_number_width, $expected);
                    }
                    )*
                }
            };
        }

        episode_number! {
            no_number(None, None, 0),
            season_only(None, Some(5), 0),
            episode_only_single_digit(Some(1), None, 3),
            episode_only_single_digit_last(Some(9), None, 3),
            episode_only_multiple_digits(Some(10), None, 4),
            episode_and_season(Some(5), Some(3), 5),
            episode_and_season_two_digits(Some(14), Some(11), 7),
        }
    }
}
