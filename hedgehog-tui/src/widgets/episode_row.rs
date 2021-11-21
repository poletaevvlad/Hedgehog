use super::list::ListItemRenderingDelegate;
use super::utils::{date_width, number_width};
use crate::options::Options;
use crate::theming;
use crate::widgets::layout::{split_left, split_right};
use crate::widgets::utils::DurationFormatter;
use hedgehog_library::model::{EpisodeId, EpisodeSummaryStatus, EpisodesListMetadata};
use tui::buffer::Buffer;
use tui::layout::{Alignment, Rect};
use tui::text::Span;
use tui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

pub(crate) struct EpisodesListRowRenderer<'t> {
    theme: &'t theming::Theme,
    default_item_state: theming::ListState,
    playing_id: Option<EpisodeId>,
    options: &'t Options,
    sizing: EpisodesListSizing,
}

#[derive(Debug, PartialEq)]
pub(crate) struct EpisodesListSizing {
    date_width: u16,
    episode_number_width: u16,
    duration_width: u16,
}

impl EpisodesListSizing {
    pub(crate) fn compute(options: &Options, metadata: &EpisodesListMetadata) -> Self {
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

        let duration_width = metadata
            .max_duration
            .map(|duration| DurationFormatter(duration).width() + 2)
            .unwrap_or(0);

        EpisodesListSizing {
            date_width: if metadata.has_publication_date {
                date_width(&options.date_format) + 2
            } else {
                0
            },
            episode_number_width,
            duration_width,
        }
    }
}

impl<'t> EpisodesListRowRenderer<'t> {
    pub(crate) fn new(
        theme: &'t theming::Theme,
        is_focused: bool,
        options: &'t Options,
        sizing: EpisodesListSizing,
    ) -> Self {
        EpisodesListRowRenderer {
            theme,
            default_item_state: if is_focused {
                theming::ListState::FOCUSED
            } else {
                theming::ListState::empty()
            },
            playing_id: None,
            options,
            sizing,
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

        if self.sizing.date_width > 0 {
            let date = item.and_then(|item| item.publication_date);
            let style = self.theme.get(theming::List::Item(
                item_state,
                Some(theming::ListSubitem::Date),
            ));
            area = if let Some(date) = date {
                let formatted = format!(" {} ", date.format(&self.options.date_format));
                let width = (formatted.width() as u16).max(self.sizing.date_width);
                let (rest, date_area) = split_right(area, width);
                let paragraph = Paragraph::new(formatted).style(style);
                paragraph.render(date_area, buf);
                rest
            } else {
                let (rest, date_area) = split_right(area, self.sizing.date_width);
                buf.set_style(date_area, style);
                rest
            }
        }

        if self.sizing.duration_width > 0 {
            let style = self.theme.get(theming::List::Item(
                item_state,
                Some(theming::ListSubitem::Duration),
            ));
            let duration = item.and_then(|item| item.duration);
            let (rest, duration_area) = split_right(area, self.sizing.duration_width);
            if let Some(duration) = duration {
                let formatted = format!(" {} ", DurationFormatter(duration));
                let paragraph = Paragraph::new(formatted)
                    .style(style)
                    .alignment(Alignment::Right);
                paragraph.render(duration_area, buf);
            } else {
                buf.set_style(duration_area, style);
            }
            area = rest;
        }

        if self.sizing.episode_number_width > 0 {
            let style = self.theme.get(theming::List::Item(
                item_state,
                Some(theming::ListSubitem::EpisodeNumber),
            ));
            let number = item
                .map(|item| (item.season_number, item.episode_number))
                .unwrap_or((None, None));
            let number = match number {
                (None, Some(episode)) => Some(format!(" {}.", episode)),
                (Some(season), Some(episode)) => Some(format!(" {}x{}.", season, episode)),
                _ => None,
            };
            let (number_area, rest) = split_left(area, self.sizing.episode_number_width);
            if let Some(number) = number {
                let paragraph = Paragraph::new(number)
                    .style(style)
                    .alignment(Alignment::Right);
                paragraph.render(number_area, buf);
            } else {
                buf.set_style(number_area, style);
            }
            area = rest;
        }

        if let Some(item) = item {
            let subtitle = match item.title.is_none() {
                true => Some(theming::ListSubitem::MissingTitle),
                false => None,
            };
            let style = self.theme.get(theming::List::Item(item_state, subtitle));
            let title = item.title.as_deref().unwrap_or("Untitled");
            buf.set_style(area, style);
            buf.set_span(
                area.x + 1,
                area.y,
                &Span::raw(title),
                area.width.saturating_sub(2),
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

        /*if matches!(item.status, EpisodeSummaryStatus::New) {
            buf.set_string(
                area.x,
                area.y,
                "*",
                self.theme.get(theming::List::Item(
                    item_state,
                    Some(theming::ListSubitem::NewIndicator),
                )),
            );
        }*/
    }

    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let state = self.default_item_state;
        let (number_rect, area) = split_left(area, self.sizing.episode_number_width);
        let (area, date_rect) = split_right(area, self.sizing.date_width);
        let (area, duration_rect) = split_right(area, self.sizing.duration_width);

        buf.set_style(area, self.theme.get(theming::List::Item(state, None)));
        buf.set_style(
            number_rect,
            self.theme.get(theming::List::Item(
                state,
                Some(theming::ListSubitem::EpisodeNumber),
            )),
        );
        buf.set_style(
            date_rect,
            self.theme
                .get(theming::List::Item(state, Some(theming::ListSubitem::Date))),
        );
        buf.set_style(
            duration_rect,
            self.theme.get(theming::List::Item(
                state,
                Some(theming::ListSubitem::Duration),
            )),
        );
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
