use super::screen::EpisodesListProvider;
use crate::dataview::{InteractiveList, PaginatedData};
use hedgehog_library::model::EpisodeSummary;

pub(crate) struct ViewModel {
    pub(crate) episodes_list: InteractiveList<PaginatedData<EpisodeSummary>, EpisodesListProvider>,
}

impl ViewModel {
    pub(crate) fn new(size: (u16, u16)) -> Self {
        ViewModel {
            episodes_list: InteractiveList::new(size.1 as usize - 1),
        }
    }

    pub(crate) fn set_size(&mut self, _width: u16, height: u16) {
        self.episodes_list.set_window_size(height as usize - 1)
    }
}
