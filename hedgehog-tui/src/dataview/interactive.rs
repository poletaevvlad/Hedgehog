use super::{
    request_data, CursorCommand, DataProvider, DataView, DataViewOptions, EditableDataView,
    UpdatableDataView, Versioned,
};
use hedgehog_library::model::Identifiable;

pub(crate) struct InteractiveList<T: DataView, P: DataProvider<Request = T::Request>>
where
    T::Item: Identifiable,
{
    provider: Versioned<Option<P>>,
    pub(super) data: T,
    options: DataViewOptions,
    selection: usize,
    offset: usize,
    window_size: usize,
    updating_index: Option<(usize, usize, <T::Item as Identifiable>::Id)>,
}

impl<T: DataView, P: DataProvider<Request = T::Request>> InteractiveList<T, P>
where
    T::Item: Identifiable,
{
    pub(crate) fn new(window_size: usize) -> Self {
        Self::new_with_options(window_size, DataViewOptions::default())
    }

    pub(crate) fn new_with_options(window_size: usize, options: DataViewOptions) -> Self {
        InteractiveList {
            provider: Versioned::new(None),
            data: T::init(|_| (), options.clone()),
            options,
            selection: 0,
            offset: 0,
            window_size,
            updating_index: None,
        }
    }

    pub(crate) fn set_provider(&mut self, provider: P) {
        if let Some(selected) = self.data.item_at(self.selection) {
            self.updating_index = Some((self.offset, self.selection, selected.id()));
        }
        self.provider = self.provider.update(Some(provider));
        self.offset = 0;
        self.selection = 0;
        self.data = T::init(
            |request| request_data(&self.provider, request),
            self.options.clone(),
        );
    }

    pub(crate) fn update_provider(&mut self, update: impl FnOnce(&mut P)) -> bool {
        let provider = self.provider.take();
        if let Some(mut provider) = provider {
            update(&mut provider);
            self.set_provider(provider);
            true
        } else {
            false
        }
    }

    pub(crate) fn invalidate(&mut self) -> bool {
        self.update_provider(|_| {})
    }

    pub(crate) fn provider(&self) -> Option<&P> {
        self.provider.as_inner().as_ref()
    }

    pub(crate) fn selection(&self) -> Option<&T::Item> {
        self.data.item_at(self.selection)
    }

    fn update(&mut self) {
        let provider = &self.provider;
        self.data
            .update(self.offset..(self.offset + self.window_size), |request| {
                request_data(provider, request);
            });
    }

    pub(crate) fn set_window_size(&mut self, window_size: usize) {
        self.window_size = window_size;
        self.move_cursor(0);
    }

    pub(crate) fn handle_data(&mut self, msg: Versioned<T::Message>) -> bool {
        let previous_size = self.data.size();
        if !self.provider.same_version(&msg) || !self.data.handle(msg.into_inner()) {
            return false;
        };
        if previous_size.is_none() && self.data.size().is_some() {
            if let Some((offset, selection, _)) = self.updating_index {
                self.offset = offset;
                self.selection = selection;
            }
            self.update();
        }

        if self.updating_index.is_some() && self.data.has_data() {
            let (_, _, id) = self.updating_index.take().unwrap();
            let index = self.data.index_of(id);
            if let Some(index) = index {
                self.selection = index;
            } else {
                self.offset = 0;
                self.selection = 0;
            }
        }

        true
    }

    fn set_cursor(&mut self, position: usize) {
        let size = match self.data.size() {
            Some(size) => size,
            None => return,
        };
        self.selection = position;

        let new_offset = if self.selection < self.offset + self.options.scroll_margins {
            Some(self.selection.saturating_sub(self.options.scroll_margins))
        } else if self.selection
            > (self.offset + self.window_size).saturating_sub(self.options.scroll_margins + 1)
        {
            Some(
                (self.selection + self.options.scroll_margins + 1).saturating_sub(self.window_size),
            )
        } else {
            None
        };

        if let Some(offset) = new_offset {
            let provider = &self.provider;
            self.offset = offset.min(size.saturating_sub(self.window_size));
            self.data
                .update(offset..(offset + self.window_size), |request| {
                    request_data(provider, request);
                });
        }
    }

    pub(crate) fn move_cursor(&mut self, offset: isize) {
        let size = match self.data.size() {
            Some(size) => size,
            None => return,
        };
        if offset < 0 {
            self.set_cursor(self.selection.saturating_sub(offset.abs() as usize));
        } else {
            self.set_cursor(
                (self.selection)
                    .saturating_add(offset as usize)
                    .min(size.saturating_sub(1)),
            );
        }
    }

    pub(crate) fn move_cursor_first(&mut self) {
        self.set_cursor(0);
    }

    pub(crate) fn move_cursor_last(&mut self) {
        if let Some(size) = self.data.size() {
            self.set_cursor(size.saturating_sub(1));
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.data.size().map(|size| size == 0).unwrap_or(false)
    }

    pub(crate) fn iter(&self) -> Option<impl Iterator<Item = (Option<&T::Item>, bool)>> {
        let window_size = self.window_size;
        let offset = self.offset;
        let selection = self.selection;
        self.data.size().map(move |size| {
            (offset..(offset + window_size).min(size))
                .map(move |index| (self.data.item_at(index), index == selection))
        })
    }

    pub(crate) fn handle_command(&mut self, command: CursorCommand) {
        match command {
            CursorCommand::Next => self.move_cursor(1),
            CursorCommand::Previous => self.move_cursor(-1),
            CursorCommand::PageUp => self.move_cursor(-(self.window_size as isize)),
            CursorCommand::PageDown => self.move_cursor(self.window_size as isize),
            CursorCommand::First => self.move_cursor_first(),
            CursorCommand::Last => self.move_cursor_last(),
        }
    }

    pub(crate) fn add_item(&mut self, item: <T as EditableDataView>::Item)
    where
        T: EditableDataView<Item = <T as DataView>::Item>,
    {
        self.data.add(item);
    }

    pub(crate) fn remove_item(&mut self, id: <T as EditableDataView>::Id)
    where
        T: EditableDataView<Item = <T as DataView>::Item>,
    {
        if let Some(removed_index) = self.data.remove(id) {
            if self.selection > removed_index {
                self.selection = self.selection.saturating_sub(1);
            }
            if let Some(size) = self.data.size() {
                self.selection = self.selection.min(size.saturating_sub(1));
            }
        }
    }

    pub(crate) fn update_item(
        &mut self,
        id: <T as UpdatableDataView>::Id,
        callback: impl FnOnce(&mut <T as UpdatableDataView>::Item),
    ) where
        T: UpdatableDataView<Item = <T as DataView>::Item>,
    {
        UpdatableDataView::update(&mut self.data, id, callback);
    }

    pub(crate) fn update_all(&mut self, callback: impl Fn(&mut <T as UpdatableDataView>::Item))
    where
        T: UpdatableDataView<Item = <T as DataView>::Item>,
    {
        UpdatableDataView::update_all(&mut self.data, callback);
    }

    pub(crate) fn update_selection(
        &mut self,
        callback: impl FnOnce(&mut <T as UpdatableDataView>::Item),
    ) where
        T: UpdatableDataView<Item = <T as DataView>::Item>,
    {
        UpdatableDataView::update_at(&mut self.data, self.selection, callback);
    }

    pub(crate) fn replace_item(&mut self, item: <T as UpdatableDataView>::Item)
    where
        T: UpdatableDataView<Item = <T as DataView>::Item>,
        <T as UpdatableDataView>::Item: Identifiable<Id = <T as UpdatableDataView>::Id>,
    {
        self.update_item(Identifiable::id(&item), |current| *current = item);
    }

    pub(crate) fn window_size(&self) -> usize {
        self.window_size
    }
}
