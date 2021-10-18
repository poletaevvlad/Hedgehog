use super::screen::EpisodesListProvider;
use crate::cmdparser;
use crate::dataview::{InteractiveList, PaginatedData};
use crate::status::Status;
use hedgehog_library::model::EpisodeSummary;

pub(crate) struct ViewModel {
    pub(crate) episodes_list: InteractiveList<PaginatedData<EpisodeSummary>, EpisodesListProvider>,
    pub(crate) status: Option<Status>,
}

impl ViewModel {
    pub(crate) fn new(size: (u16, u16)) -> Self {
        ViewModel {
            episodes_list: InteractiveList::new(size.1 as usize - 1),
            status: None,
        }
    }

    pub(crate) fn set_size(&mut self, _width: u16, height: u16) {
        self.episodes_list.set_window_size(height as usize - 1)
    }

    pub(crate) fn clear_status(&mut self) {
        self.status = None;
    }

    pub(crate) fn handle_command_str(&mut self, command: &str) {
        match cmdparser::from_str::<()>(command) {
            Ok(command) => self.handle_command(command),
            Err(error) => self.status = Some(Status::CommandParsingError(error)),
        }
    }

    pub(crate) fn handle_command(&mut self, _command: ()) {
        todo!()
    }
}
