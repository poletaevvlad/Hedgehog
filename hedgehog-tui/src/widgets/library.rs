use super::empty::EmptyView;
use super::episode_row::{EpisodesListRowRenderer, EpisodesListSizing};
use super::feed_row::FeedsListRowRenderer;
use super::list::List;
use crate::mouse::WidgetPositions;
use crate::options::Options;
use crate::screen::{FocusedPane, LibraryViewModel};
use crate::scrolling::DataView;
use crate::theming::{self, Theme};
use hedgehog_library::model::{FeedStatus, FeedView};
use tui::layout::{Constraint, Direction, Layout};
use tui::widgets::{Block, Borders, Widget};

pub(crate) struct LibraryWidget<'a> {
    theme: &'a Theme,
    options: &'a Options,
    data: &'a LibraryViewModel,
    layout: &'a mut WidgetPositions,
}

impl<'a> LibraryWidget<'a> {
    pub(crate) fn new(
        data: &'a LibraryViewModel,
        options: &'a Options,
        theme: &'a Theme,
        layout: &'a mut WidgetPositions,
    ) -> Self {
        LibraryWidget {
            data,
            options,
            theme,
            layout,
        }
    }
}

impl<'a> Widget for LibraryWidget<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        if self.data.feeds.data().size() == 2 && self.data.feeds_loaded {
            EmptyView::new(self.theme)
                .title("Hedgehog Podcast Player")
                .subtitle(
                    "Welcome.\nAdd podcasts by their RSS feeds by typing :add [feed-url]<Enter>",
                )
                .focused(true)
                .render(area, buf);

            return;
        }

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(24), Constraint::Percentage(75)].as_ref())
            .split(area);

        let feeds_border = Block::default()
            .borders(Borders::RIGHT)
            .border_style(self.theme.get(theming::List::Divider));
        let feeds_area = feeds_border.inner(layout[0]);
        feeds_border.render(layout[0], buf);

        self.layout.set_feeds_list(feeds_area);
        List::new(
            FeedsListRowRenderer::new(
                self.theme,
                self.options,
                self.data.focus == FocusedPane::FeedsList,
                &self.data.updating_feeds,
            )
            .playing(
                self.data
                    .playing_episode
                    .as_ref()
                    .map(|episode| episode.feed_id),
            ),
            self.data.feeds.visible_iter(),
        )
        .render(feeds_area, buf);

        if let Some(metadata) = self.data.episodes_list_metadata.as_ref() {
            let selected_feed_index = self.data.feeds.viewport().selected_index();
            let state = self
                .data
                .feeds
                .data()
                .item_at(selected_feed_index)
                .map(|item| item.as_ref().map(|feed| feed.status));

            if self.data.episodes.data().size() == 0 {
                match state {
                    Some(FeedView::All) => {}
                    Some(FeedView::New) => {
                        EmptyView::new(self.theme)
                            .title("There are no new episodes.")
                            .subtitle("New episodes will appear here automatically. You can mark an episode as new by typing :mark new<Enter>")
                            .focused(self.data.focus == FocusedPane::EpisodesList)
                            .render(layout[1], buf);
                    }
                    Some(FeedView::Feed(FeedStatus::Pending)) => {
                        EmptyView::new(self.theme)
                            .title("This feed's episodes aren't loaded yet")
                            .focused(self.data.focus == FocusedPane::EpisodesList)
                            .render(layout[1], buf);
                    }
                    Some(FeedView::Feed(FeedStatus::Loaded)) => {
                        EmptyView::new(self.theme)
                            .title("This feed is empty")
                            .subtitle(
                                "There are no episodes in this feed. Perhaps, it is not a podcast?",
                            )
                            .focused(self.data.focus == FocusedPane::EpisodesList)
                            .render(layout[1], buf);
                    }
                    Some(FeedView::Feed(FeedStatus::Error(error))) => {
                        let subtitle =
                            format!("\n{}\n\nType :update<Enter> to reload this feed.", error);
                        EmptyView::new(self.theme)
                            .title("Could not load a feed")
                            .subtitle(&subtitle)
                            .focused(self.data.focus == FocusedPane::EpisodesList)
                            .render(layout[1], buf);
                    }
                    None => {}
                }
            } else {
                let mut sizing = EpisodesListSizing::compute(self.options, metadata);
                if state == Some(FeedView::All) {
                    sizing.hide_episode_numbers();
                }

                self.layout.set_episodes_list(layout[1]);
                List::new(
                    EpisodesListRowRenderer::new(
                        self.theme,
                        self.data.focus == FocusedPane::EpisodesList,
                        self.options,
                        sizing.with_width(layout[1].width),
                    )
                    .with_playing_id(self.data.playing_episode.as_ref().map(|episode| episode.id)),
                    self.data.episodes.visible_iter_partial(),
                )
                .render(layout[1], buf);
            }
        }
    }
}
