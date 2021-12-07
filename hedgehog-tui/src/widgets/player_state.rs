use crate::options::Options;
use crate::theming;
use crate::widgets::layout::shrink_h;
use crate::widgets::utils::PlaybackTimingFormatter;
use hedgehog_library::model::EpisodePlaybackData;
use hedgehog_player::state::{PlaybackState, PlaybackStatus};
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::text::{Span, Spans};
use tui::widgets::{Paragraph, Widget};

pub(crate) struct PlayerState<'a> {
    state: &'a PlaybackState,
    theme: &'a theming::Theme,
    options: &'a Options,
    episode: Option<&'a EpisodePlaybackData>,
}

impl<'a> PlayerState<'a> {
    pub(crate) fn new(
        state: &'a PlaybackState,
        theme: &'a theming::Theme,
        options: &'a Options,
        episode: Option<&'a EpisodePlaybackData>,
    ) -> Self {
        PlayerState {
            state,
            theme,
            options,
            episode,
        }
    }
}

impl<'a> Widget for PlayerState<'a> {
    fn render(self, mut area: Rect, buf: &mut Buffer) {
        let status = self.state.status();
        let status_style = self.theme.get(theming::Player {
            status: Some(status),
            subitem: Some(theming::PlayerItem::Status),
        });
        let status_label = match status {
            PlaybackStatus::None => &self.options.label_playback_status_none,
            PlaybackStatus::Buffering => &self.options.label_playback_status_buffering,
            PlaybackStatus::Playing => &self.options.label_playback_status_playing,
            PlaybackStatus::Paused => &self.options.label_playback_status_paused,
        };
        let (x, _) = buf.set_span(
            area.x,
            area.y,
            &Span::styled(status_label, status_style),
            area.width,
        );
        area.width -= x - area.x;
        area.x = x;

        if let Some(timing) = self.state.timing() {
            let formatted = format!(" {} ", PlaybackTimingFormatter(timing));
            let style = self.theme.get(theming::Player {
                status: Some(status),
                subitem: Some(theming::PlayerItem::Timing),
            });
            let width = formatted.len() as u16; // PlaybackTiming's Display implementation produces ASCII characters only
            buf.set_span(
                (area.x + area.width).saturating_sub(width),
                area.y,
                &Span::styled(formatted, style),
                width,
            );
            area.width -= width;
        }

        let title_style = self.theme.get(theming::Player {
            status: Some(status),
            subitem: Some(theming::PlayerItem::EpisodeTitle),
        });
        buf.set_style(area, title_style);

        let mut text = Vec::new();
        let episode_title = self.episode.and_then(|ep| ep.episode_title.as_deref());
        if let Some(title) = episode_title {
            text.push(Span::raw(title));
        }
        if let Some(title) = self.episode.and_then(|ep| ep.feed_title.as_deref()) {
            let style = self.theme.get(theming::Player {
                status: Some(status),
                subitem: Some(theming::PlayerItem::FeedTitle),
            });

            if episode_title.is_some() {
                text.push(Span::styled(" / ", style));
            }
            text.push(Span::styled(title, style));
        }
        if !text.is_empty() {
            Paragraph::new(vec![Spans::from(text)]).render(shrink_h(area, 1), buf);
        }
    }
}
