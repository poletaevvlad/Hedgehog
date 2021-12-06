use super::empty::EmptyView;
use super::episode_row::{EpisodesListRowRenderer, EpisodesListSizing};
use super::feed_row::FeedsListRowRenderer;
use super::list::List;
use crate::options::Options;
use crate::screen::{FocusedPane, LibraryViewModel, SearchState};
use crate::scrolling::DataView;
use crate::theming::{self, Theme};
use crate::widgets::search_row::SearchResultRowRenderer;
use hedgehog_library::model::{FeedStatus, FeedView};
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

    fn render_library(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        if self.data.feeds.data().size() == 1 && self.data.feeds_loaded {
            EmptyView::new(self.theme)
                .title("Hedgehog Podcast Player")
                .subtitle(
                    "Welcome.\nAdd podcasts by their RSS feeds by typing :add [feed-url]<Enter>",
                )
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

        List::new(
            FeedsListRowRenderer::new(
                self.theme,
                self.options,
                self.data.focus == FocusedPane::FeedsList,
                &self.data.updating_feeds,
            ),
            self.data.feeds.visible_iter(),
        )
        .render(feeds_area, buf);

        if let Some(metadata) = self.data.episodes_list_metadata.as_ref() {
            if self.data.episodes.data().size() == 0 {
                let selected_feed_index = self.data.feeds.viewport().selected_index();
                let state = self
                    .data
                    .feeds
                    .data()
                    .item_at(selected_feed_index)
                    .map(|item| item.as_ref().map(|feed| feed.status));

                match state {
                    Some(FeedView::All) => {}
                    Some(FeedView::Feed(FeedStatus::Pending)) => {
                        EmptyView::new(self.theme)
                            .title("This feed's episodes aren't loaded yet")
                            .render(layout[1], buf);
                    }
                    Some(FeedView::Feed(FeedStatus::Loaded)) => {
                        EmptyView::new(self.theme)
                            .title("This feed is empty")
                            .subtitle(
                                "There are no episodes in this feed. Perhaps, it is not a podcast?",
                            )
                            .render(layout[1], buf);
                    }
                    Some(FeedView::Feed(FeedStatus::Error(error))) => {
                        let subtitle =
                            format!("\n{}\n\nType :update<Enter> to reload this feed.", error);
                        EmptyView::new(self.theme)
                            .title("Could not load a feed")
                            .subtitle(&subtitle)
                            .render(layout[1], buf);
                    }
                    None => {}
                }
            } else {
                let sizing =
                    EpisodesListSizing::compute(self.options, metadata).with_width(layout[1].width);
                List::new(
                    EpisodesListRowRenderer::new(
                        self.theme,
                        self.data.focus == FocusedPane::EpisodesList,
                        self.options,
                        sizing,
                    )
                    .with_playing_id(self.data.playing_episode.as_ref().map(|episode| episode.id)),
                    self.data.episodes.visible_iter_partial(),
                )
                .render(layout[1], buf);
            }
        }
    }

    fn render_search(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        match &self.data.search {
            SearchState::Loaded(list) if list.data().is_empty() => EmptyView::new(self.theme)
                .title("Nothing is found")
                .subtitle("Please make sure that your query is correct")
                .render(area, buf),
            SearchState::Loaded(list) => List::new(
                SearchResultRowRenderer::new(self.theme),
                list.visible_iter(),
            )
            .render(area, buf),
            SearchState::Loading => EmptyView::new(self.theme)
                .title("Searching...")
                .render(area, buf),
            SearchState::Error(err) => EmptyView::new(self.theme)
                .title("Search request failed")
                .subtitle(&err.to_string())
                .render(area, buf),
        }
    }
}

impl<'a> Widget for LibraryWidget<'a> {
    fn render(self, area: tui::layout::Rect, buf: &mut tui::buffer::Buffer) {
        match &self.data.focus {
            FocusedPane::FeedsList | FocusedPane::EpisodesList => {
                self.render_library(area, buf);
            }
            FocusedPane::Search => self.render_search(area, buf),
        }
    }
}
