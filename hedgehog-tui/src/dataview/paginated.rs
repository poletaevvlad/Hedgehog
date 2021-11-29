use super::{DataView, DataViewOptions, UpdatableDataView};
use hedgehog_library::model::Identifiable;
use hedgehog_library::Page;
use std::{collections::VecDeque, ops::Range};

#[derive(Debug, PartialEq)]
pub(crate) enum PaginatedDataRequest {
    Size,
    Page(Page),
}

#[derive(Debug)]
pub(crate) enum PaginatedDataMessage<T> {
    Size(usize),
    Page { index: usize, values: Vec<T> },
}

impl<T> PaginatedDataMessage<T> {
    pub(crate) fn size(size: usize) -> Self {
        PaginatedDataMessage::Size(size)
    }

    pub(crate) fn page(index: usize, values: Vec<T>) -> Self {
        PaginatedDataMessage::Page { index, values }
    }
}

#[derive(Debug)]
pub(crate) struct PaginatedData<T> {
    page_size: usize,
    load_margins: usize,
    size: Option<usize>,
    first_page_index: usize,
    pages: VecDeque<Option<Vec<T>>>,
}

impl<T> PaginatedData<T> {
    fn page_index(&self, index: usize) -> usize {
        index / self.page_size
    }

    fn page_item_index(&self, index: usize) -> usize {
        index % self.page_size
    }

    fn request_page(&self, index: usize, request_data: &impl Fn(PaginatedDataRequest)) {
        request_data(PaginatedDataRequest::Page(Page::new(index, self.page_size)));
    }

    #[cfg(test)]
    pub(super) fn pages_range(&self) -> (usize, usize) {
        (self.first_page_index, self.pages.len())
    }
}

impl<T> DataView for PaginatedData<T> {
    type Item = T;
    type Request = PaginatedDataRequest;
    type Message = PaginatedDataMessage<T>;

    fn init(request_data: impl Fn(Self::Request), options: DataViewOptions) -> Self {
        request_data(PaginatedDataRequest::Size);
        PaginatedData {
            page_size: options.page_size,
            load_margins: options.load_margins,
            size: None,
            first_page_index: 0,
            pages: VecDeque::new(),
        }
    }

    fn size(&self) -> Option<usize> {
        self.size
    }

    fn update(&mut self, range: Range<usize>, request_data: impl Fn(Self::Request)) {
        let size = match self.size {
            Some(size) => size,
            None => return,
        };
        let first_required_page = self.page_index(range.start.saturating_sub(self.load_margins));
        let last_required_page =
            self.page_index(((range.end + self.load_margins).saturating_sub(1)).min(size));
        let indices_count = last_required_page - first_required_page + 1;

        if !self.pages.is_empty() {
            while self.first_page_index < first_required_page {
                self.pages.pop_front();
                self.first_page_index += 1;
            }
            while self.first_page_index > first_required_page {
                self.pages.push_front(None);
                self.first_page_index -= 1;
                self.request_page(self.first_page_index, &request_data);
            }

            if self.pages.len() > indices_count {
                self.pages.drain(indices_count..);
            }
        } else {
            self.first_page_index = first_required_page;
        }
        while self.pages.len() < indices_count {
            self.request_page(self.first_page_index + self.pages.len(), &request_data);
            self.pages.push_back(None);
        }
    }

    fn handle(&mut self, msg: Self::Message) -> bool {
        match msg {
            PaginatedDataMessage::Size(size) => {
                self.size = Some(size);
                true
            }
            PaginatedDataMessage::Page { index, values } => {
                if index < self.first_page_index {
                    return false;
                }
                if let Some(page) = self.pages.get_mut(index - self.first_page_index) {
                    *page = Some(values);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn item_at(&self, index: usize) -> Option<&T> {
        let page_index = self.page_index(index);
        if page_index < self.first_page_index {
            return None;
        }
        self.pages
            .get(page_index - self.first_page_index)
            .and_then(|page| page.as_ref())
            .and_then(|page| page.get(self.page_item_index(index)))
    }

    fn has_data(&self) -> bool {
        !self.pages.is_empty() && self.pages.iter().all(Option::is_some)
    }

    fn index_of<ID: Eq>(&self, id: ID) -> Option<usize>
    where
        Self::Item: Identifiable<Id = ID>,
    {
        for (page_index, page) in self.pages.iter().enumerate() {
            if let Some(page_items) = page {
                for (item_index, item) in page_items.iter().enumerate() {
                    if item.id() == id {
                        return Some(
                            item_index + (self.first_page_index + page_index) * self.page_size,
                        );
                    }
                }
            }
        }
        None
    }
}

impl<T: Identifiable> UpdatableDataView for PaginatedData<T> {
    type Id = T::Id;
    type Item = T;

    fn update(&mut self, id: Self::Id, callback: impl FnOnce(&mut Self::Item)) {
        for page in self.pages.iter_mut() {
            if let Some(page) = page.as_mut() {
                for item in page {
                    if item.id() == id {
                        callback(item);
                        return;
                    }
                }
            }
        }
    }

    fn update_all(&mut self, callback: impl Fn(&mut Self::Item)) {
        for page in self.pages.iter_mut() {
            if let Some(ref mut page) = page.as_mut() {
                for item in page.iter_mut() {
                    callback(item);
                }
            }
        }
    }

    fn update_at(&mut self, index: usize, callback: impl FnOnce(&mut Self::Item)) {
        let page_index = self.page_index(index);
        let index_in_page = self.page_item_index(index);

        if let Some(Some(page)) = self.pages.get_mut(page_index) {
            if let Some(item) = page.get_mut(index_in_page) {
                callback(item);
            }
        }
    }
}
