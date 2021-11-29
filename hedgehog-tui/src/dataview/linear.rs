use super::{index_with_id, DataView, DataViewOptions, EditableDataView, UpdatableDataView};
use hedgehog_library::model::Identifiable;
use std::ops::Range;

#[derive(Debug)]
pub(crate) struct ListDataRequest;

#[derive(Debug)]
pub(crate) struct ListData<T> {
    items: Option<Vec<T>>,
}

impl<T> DataView for ListData<T> {
    type Item = T;
    type Request = ListDataRequest;
    type Message = Vec<T>;

    fn init(request_data: impl Fn(Self::Request), _options: DataViewOptions) -> Self {
        request_data(ListDataRequest);
        Self { items: None }
    }

    fn size(&self) -> Option<usize> {
        self.items.as_ref().map(Vec::len)
    }

    fn update(&mut self, _range: Range<usize>, _request_data: impl Fn(Self::Request)) {}

    fn handle(&mut self, msg: Self::Message) -> bool {
        if self.items.is_none() {
            self.items = Some(msg);
            true
        } else {
            false
        }
    }

    fn item_at(&self, index: usize) -> Option<&Self::Item> {
        self.items.as_ref().and_then(|items| items.get(index))
    }
}

impl<T: Identifiable> EditableDataView for ListData<T> {
    type Id = T::Id;
    type Item = T;

    fn remove(&mut self, id: <T as Identifiable>::Id) -> Option<usize> {
        if let Some(ref mut items) = self.items {
            if let Some(index) = index_with_id(items.iter(), id) {
                items.remove(index);
                return Some(index);
            }
        }
        None
    }

    fn add(&mut self, item: Self::Item) {
        if let Some(ref mut items) = self.items {
            items.push(item);
        }
    }
}

impl<T: Identifiable> UpdatableDataView for ListData<T> {
    type Id = T::Id;
    type Item = T;

    fn update(&mut self, id: Self::Id, callback: impl FnOnce(&mut Self::Item)) {
        if let Some(ref mut items) = self.items {
            if let Some(index) = index_with_id(items.iter(), id) {
                callback(&mut items[index]);
            }
        }
    }

    fn update_all(&mut self, callback: impl Fn(&mut Self::Item)) {
        if let Some(items) = self.items.as_mut() {
            for item in items {
                callback(item);
            }
        }
    }

    fn update_at(&mut self, index: usize, callback: impl FnOnce(&mut Self::Item)) {
        if let Some(ref mut items) = self.items {
            if let Some(index) = items.get_mut(index) {
                callback(index);
            }
        }
    }
}
