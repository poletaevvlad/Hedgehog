use super::list::ListItemRenderingDelegate;
use super::utils::{date_width, number_width};
use crate::options::Options;
use crate::theming;
use crate::widgets::layout::{split_left, split_right};
use crate::widgets::utils::DurationFormatter;
use hedgehog_library::model::{
    EpisodeId, EpisodeSummary, EpisodeSummaryStatus, EpisodesListMetadata,
};
use tui::buffer::Buffer;
use tui::layout::{Alignment, Rect};
use tui::text::Span;
use tui::widgets::{Paragraph, Widget};
use unicode_width::UnicodeWidthStr;

pub(crate) struct EpisodesListRowRenderer<'t> {
    theme: &'t theming::Theme,
    focused: bool,
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

enum EpisodeState {
    NotStarted,
    New,
    Playing,
    Started,
    Finished,
    Error,
}

impl EpisodeState {
    fn label<'a>(&self, options: &'a Options) -> &'a str {
        match self {
            EpisodeState::NotStarted => options.label_episode_seen.as_str(),
            EpisodeState::New => options.label_episode_new.as_str(),
            EpisodeState::Playing => options.label_episode_playing.as_str(),
            EpisodeState::Started => options.label_episode_started.as_str(),
            EpisodeState::Finished => options.label_episode_finished.as_str(),
            EpisodeState::Error => options.label_episode_error.as_str(),
        }
    }

    fn as_theme_state(&self) -> theming::ListState {
        match self {
            EpisodeState::NotStarted => theming::ListState::Episode,
            EpisodeState::New => theming::ListState::EpisodeNew,
            EpisodeState::Playing => theming::ListState::EpisodePlaying,
            EpisodeState::Started => theming::ListState::EpisodeStarted,
            EpisodeState::Finished => theming::ListState::EpisodeFinished,
            EpisodeState::Error => theming::ListState::EpisodeError,
        }
    }
}

impl EpisodesListSizing {
    const TITLE_WIDTH: u16 = 32;

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

    pub(crate) fn with_width(mut self, width: u16) -> Self {
        let mut fields_width = self.date_width + self.episode_number_width + self.duration_width;

        if self.duration_width > 0 && fields_width + Self::TITLE_WIDTH > width {
            fields_width -= self.duration_width;
            self.duration_width = 0;
        }
        if self.episode_number_width > 0 && fields_width + Self::TITLE_WIDTH > width {
            fields_width -= self.episode_number_width;
            self.episode_number_width = 0;
        }
        if self.date_width > 0 && fields_width + Self::TITLE_WIDTH > width {
            self.date_width = 0;
        }

        self
    }

    pub(crate) fn hide_episode_numbers(&mut self) {
        self.episode_number_width = 0;
    }
}

impl<'t> EpisodesListRowRenderer<'t> {
    pub(crate) fn new(
        theme: &'t theming::Theme,
        focused: bool,
        options: &'t Options,
        sizing: EpisodesListSizing,
    ) -> Self {
        EpisodesListRowRenderer {
            theme,
            focused,
            playing_id: None,
            options,
            sizing,
        }
    }

    pub(crate) fn with_playing_id(mut self, playing_id: impl Into<Option<EpisodeId>>) -> Self {
        self.playing_id = playing_id.into();
        self
    }

    fn episode_status(&self, episode: &EpisodeSummary) -> EpisodeState {
        if Some(episode.id) == self.playing_id {
            EpisodeState::Playing
        } else {
            match episode.status {
                EpisodeSummaryStatus::New => EpisodeState::New,
                EpisodeSummaryStatus::NotStarted => EpisodeState::NotStarted,
                EpisodeSummaryStatus::Finished => EpisodeState::Finished,
                EpisodeSummaryStatus::Started => EpisodeState::Started,
                EpisodeSummaryStatus::Error => EpisodeState::Error,
            }
        }
    }
}

impl<'t, 'a> ListItemRenderingDelegate<'a> for EpisodesListRowRenderer<'t> {
    type Item = (Option<&'a EpisodeSummary>, bool);

    fn render_item(&self, mut area: Rect, item: Self::Item, buf: &mut Buffer) {
        let (item, selected) = item;

        let item_selector = theming::ListItem {
            selected,
            focused: self.focused,
            missing_title: item.map(|item| item.title.is_none()).unwrap_or(false),
            state: Some(
                item.map(|item| self.episode_status(item))
                    .unwrap_or(EpisodeState::NotStarted)
                    .as_theme_state(),
            ),
            column: None,
        };

        if self.sizing.date_width > 0 {
            let style = self.theme.get(theming::List::Item(
                item_selector.with_column(theming::ListColumn::Date),
            ));
            let date = item.and_then(|item| item.publication_date);
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
                item_selector.with_column(theming::ListColumn::Duration),
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
                item_selector.with_column(theming::ListColumn::EpisodeNumber),
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
            let status = self.episode_status(item);
            let status_label = status.label(self.options);
            if !status_label.is_empty() {
                let label_width = status_label.width();
                let (rest, status_area) = split_right(area, label_width as u16);
                let style = self.theme.get(theming::List::Item(
                    item_selector.with_column(theming::ListColumn::StateIndicator),
                ));
                buf.set_stringn(
                    status_area.x,
                    status_area.y,
                    status_label,
                    label_width,
                    style,
                );
                area = rest;
            }

            let style = self.theme.get(theming::List::Item(
                item_selector.with_column(theming::ListColumn::Title),
            ));
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
                item_selector.with_column(theming::ListColumn::Loading),
            ));
            let paragraph = Paragraph::new(".  .  .")
                .style(style)
                .alignment(Alignment::Center);
            paragraph.render(area, buf);
        }
    }

    fn render_empty(&self, area: Rect, buf: &mut Buffer) {
        let item_selector = theming::ListItem {
            state: Some(theming::ListState::Episode),
            ..Default::default()
        };

        let (number_rect, area) = split_left(area, self.sizing.episode_number_width);
        let (area, date_rect) = split_right(area, self.sizing.date_width);
        let (area, duration_rect) = split_right(area, self.sizing.duration_width);

        buf.set_style(
            area,
            self.theme.get(theming::List::Item(
                item_selector.with_column(theming::ListColumn::Title),
            )),
        );
        buf.set_style(
            number_rect,
            self.theme.get(theming::List::Item(
                item_selector.with_column(theming::ListColumn::EpisodeNumber),
            )),
        );
        buf.set_style(
            date_rect,
            self.theme.get(theming::List::Item(
                item_selector.with_column(theming::ListColumn::Date),
            )),
        );
        buf.set_style(
            duration_rect,
            self.theme.get(theming::List::Item(
                item_selector.with_column(theming::ListColumn::Duration),
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
