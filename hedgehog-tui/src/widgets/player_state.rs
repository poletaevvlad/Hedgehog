use crate::options::Options;
use crate::theming;
use crate::widgets::utils::PlaybackTimingFormatter;
use hedgehog_library::model::EpisodeSummary;
use hedgehog_player::state::{PlaybackState, PlaybackStatus};
use tui::buffer::Buffer;
use tui::layout::Rect;
use tui::text::Span;
use tui::widgets::Widget;

pub(crate) struct PlayerState<'a> {
    state: &'a PlaybackState,
    theme: &'a theming::Theme,
    options: &'a Options,
    episode: Option<&'a EpisodeSummary>,
}

impl<'a> PlayerState<'a> {
    pub(crate) fn new(
        state: &'a PlaybackState,
        theme: &'a theming::Theme,
        options: &'a Options,
        episode: Option<&'a EpisodeSummary>,
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
        let status_style = self.theme.get(theming::Player::Status(Some(status)));
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
            let style = self.theme.get(theming::Player::Timing);
            let width = formatted.len() as u16; // PlaybackTiming's Display implementation produces ASCII characters only
            buf.set_span(
                (area.x + area.width).saturating_sub(width),
                area.y,
                &Span::styled(formatted, style),
                width,
            );
            area.width -= width;
        }

        buf.set_style(area, self.theme.get(theming::Player::Title));
        if let Some(title) = self.episode.and_then(|ep| ep.title.as_deref()) {
            buf.set_span(
                area.x + 1,
                area.y,
                &Span::raw(title),
                area.width.saturating_sub(2),
            );
        }
    }
}
