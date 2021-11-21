use super::episode_row::{EpisodesListRowRenderer, EpisodesListSizing};
use super::feed_row::FeedsListRowRenderer;
use super::list::List;
use crate::options::Options;
use crate::screen::{FocusedPane, LibraryViewModel};
use crate::theming::{self, Theme};
use tui::layout::{Constraint, Direction, Layout};
use tui::widgets::{Block, Borders, Widget};

pub(crate) struct LibraryWidget<'a> {
    theme: &'a Theme,
    options: &'a Options,
    data: &'a LibraryViewModel,
}

impl<'a> LibraryWidget<'a> {
    pub(crate) fn new(data: &'a LibraryViewModel, options: &'a Options, theme: &'a Theme) -> Self {
        LibraryWidget {
            data,
            options,
            theme,
        }
    }
}

impl<'a> Widget for LibraryWidget<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(24), Constraint::Percentage(75)].as_ref())
            .split(area);

        let feeds_border = Block::default()
            .borders(Borders::RIGHT)
            .border_style(self.theme.get(theming::List::Divider));
        let feeds_area = feeds_border.inner(layout[0]);
        feeds_border.render(layout[0], buf);

        if let Some(iter) = self.data.feeds.iter() {
            List::new(
                FeedsListRowRenderer::new(
                    self.theme,
                    self.data.focus == FocusedPane::FeedsList,
                    &self.data.updating_feeds,
                ),
                iter,
            )
            .render(feeds_area, buf);
        }

        if let (Some(iter), Some(metadata)) = (
            self.data.episodes.iter(),
            self.data.episodes_list_metadata.as_ref(),
        ) {
            let sizing = EpisodesListSizing::compute(self.options, metadata);
            List::new(
                EpisodesListRowRenderer::new(
                    self.theme,
                    self.data.focus == FocusedPane::EpisodesList,
                    self.options,
                    sizing,
                )
                .with_playing_id(self.data.playing_episode.as_ref().map(|episode| episode.id)),
                iter,
            )
            .render(layout[1], buf);
        }
    }
}
