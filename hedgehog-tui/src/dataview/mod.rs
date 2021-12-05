pub(crate) mod interactive;
pub(crate) mod linear;
mod version;

use hedgehog_library::model::Identifiable;
use std::ops::Range;
pub(crate) use version::Versioned;

#[derive(Debug, Clone)]
pub(crate) struct DataViewOptions {
    page_size: usize,
    load_margins: usize,
    scroll_margins: usize,
}

impl Default for DataViewOptions {
    fn default() -> Self {
        DataViewOptions {
            page_size: 128,
            load_margins: 32,
            scroll_margins: 3,
        }
    }
}

pub(crate) trait DataView {
    type Item;
    type Request;
    type Message;

    fn init(request_data: impl Fn(Self::Request), options: DataViewOptions) -> Self;
    fn item_at(&self, index: usize) -> Option<&Self::Item>;
    fn size(&self) -> Option<usize>;
    fn update(&mut self, range: Range<usize>, request_data: impl Fn(Self::Request));
    fn handle(&mut self, msg: Self::Message) -> bool;
    fn has_data(&self) -> bool;
    fn index_of<ID: Eq>(&self, id: ID) -> Option<usize>
    where
        Self::Item: Identifiable<Id = ID>;
}

pub(crate) trait EditableDataView {
    type Id;
    type Item: Identifiable<Id = Self::Id>;

    fn remove(&mut self, id: Self::Id) -> Option<usize>;
    fn add(&mut self, item: Self::Item);
}

pub(crate) trait UpdatableDataView {
    type Id;
    type Item: Identifiable<Id = Self::Id>;

    fn update(&mut self, id: Self::Id, callback: impl FnOnce(&mut Self::Item));
    fn update_all(&mut self, callback: impl Fn(&mut Self::Item));
    fn update_at(&mut self, index: usize, callback: impl FnOnce(&mut Self::Item));
}

fn index_with_id<'a, T: Identifiable + 'a>(
    items: impl Iterator<Item = &'a T>,
    id: T::Id,
) -> Option<usize> {
    items
        .enumerate()
        .filter(|(_, item)| item.id() == id)
        .map(|(index, _)| index)
        .next()
}

pub(crate) trait DataProvider {
    type Request;

    fn request(&self, request: Versioned<Self::Request>);
}

fn request_data<P: DataProvider>(provider: &Versioned<Option<P>>, message: P::Request) {
    let message = provider.with_data(message);
    provider.as_ref().map(|provider| {
        if let Some(provider) = provider {
            provider.request(message);
        }
    });
}
