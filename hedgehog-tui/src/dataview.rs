use std::collections::VecDeque;
use std::ops::Range;

// TODO: remove lifetime when GATs are stabilized
trait DataView<'a> {
    type Item: 'a;
    type Iter: Iterator<Item = Option<&'a Self::Item>>;
    type Request;
    type Message;

    fn init(request_data: impl Fn(Self::Request)) -> Self;
    fn iter(&'a self, offset: usize) -> Option<Self::Iter>;
    fn size(&self) -> Option<usize>;
    fn update(&mut self, range: Range<usize>, request_data: impl Fn(Self::Request));
    fn handle(&mut self, msg: Self::Message) -> bool;
}

struct ListDataRequest;

struct ListData<T> {
    items: Option<Vec<T>>,
}

impl<'a, T> DataView<'a> for ListData<T>
where
    Self: 'a,
{
    type Item = T;
    type Iter = ListDataIterator<'a, T>;
    type Request = ListDataRequest;
    type Message = Vec<T>;

    fn init(request_data: impl Fn(Self::Request)) -> Self {
        request_data(ListDataRequest);
        Self { items: None }
    }

    fn iter(&'a self, offset: usize) -> Option<Self::Iter> {
        self.items
            .as_ref()
            .map(|items| ListDataIterator(items[offset..].iter()))
    }

    fn size(&self) -> Option<usize> {
        self.items.as_ref().map(Vec::len)
    }

    fn handle(&mut self, msg: Self::Message) -> bool {
        if self.items.is_none() {
            self.items = Some(msg);
            true
        } else {
            false
        }
    }

    fn update(&mut self, _range: Range<usize>, _request_data: impl Fn(Self::Request)) {}
}

struct ListDataIterator<'a, T>(std::slice::Iter<'a, T>);

impl<'a, T> Iterator for ListDataIterator<'a, T> {
    type Item = Option<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(Some)
    }
}

enum PaginagedDataRequest {
    Size,
    Page { index: usize, range: Range<usize> },
}

enum PaginatedDataMessage<T> {
    Size(usize),
    Page { index: usize, values: Vec<T> },
}

struct PaginatedData<T> {
    page_size: usize,
    margin_size: usize,
    size: Option<usize>,
    first_chunk_index: usize,
    chunks: VecDeque<Option<Vec<T>>>,
}

impl<T> PaginatedData<T> {
    const DEFAULT_PAGE_SIZE: usize = 128;
    const DEFAULT_MARGIN_SIZE: usize = 32;

    fn page_index(&self, index: usize) -> usize {
        index / self.page_size
    }

    fn page_item_index(&self, index: usize) -> usize {
        index % self.page_size
    }

    fn item_at(&self, index: usize) -> Option<&T> {
        let page_index = self.page_index(index);
        if page_index < self.first_chunk_index {
            return None;
        }
        self.chunks
            .get(page_index - self.first_chunk_index)
            .and_then(|page| page.as_ref())
            .and_then(|page| page.get(self.page_item_index(index)))
    }

    fn request_page(&self, index: usize, request_data: &impl Fn(PaginagedDataRequest)) {
        request_data(PaginagedDataRequest::Page {
            index,
            range: (index * self.page_size)..((index + 1) * self.page_size),
        });
    }
}

impl<'a, T> DataView<'a> for PaginatedData<T>
where
    Self: 'a,
{
    type Item = T;
    type Iter = PaginatedDataIterator<'a, T>;
    type Request = PaginagedDataRequest;
    type Message = PaginatedDataMessage<T>;

    fn init(request_data: impl Fn(Self::Request)) -> Self {
        request_data(PaginagedDataRequest::Size);
        PaginatedData {
            page_size: Self::DEFAULT_PAGE_SIZE,
            margin_size: Self::DEFAULT_MARGIN_SIZE,
            size: None,
            first_chunk_index: 0,
            chunks: VecDeque::new(),
        }
    }

    fn iter(&'a self, offset: usize) -> Option<Self::Iter> {
        self.size.map(|size| PaginatedDataIterator {
            data: self,
            index: offset,
            size,
        })
    }

    fn size(&self) -> Option<usize> {
        self.size
    }

    fn update(&mut self, range: Range<usize>, request_data: impl Fn(Self::Request)) {
        let size = match self.size {
            Some(size) => size,
            None => return,
        };
        let first_required_page = self.page_index(range.start.saturating_sub(self.margin_size));
        let last_required_page =
            self.page_item_index(((range.end + self.margin_size).saturating_sub(1)).min(size));
        let indices_count = last_required_page - first_required_page + 1;

        if !self.chunks.is_empty() {
            while self.first_chunk_index < first_required_page {
                self.chunks.pop_front();
                self.first_chunk_index += 1;
            }
            while self.first_chunk_index > first_required_page {
                self.chunks.push_front(None);
                self.first_chunk_index += 1;
                self.request_page(self.first_chunk_index, &request_data);
            }

            if self.chunks.len() > indices_count {
                self.chunks.drain(indices_count..);
            }
        } else {
            self.first_chunk_index = first_required_page;
        }
        while self.chunks.len() < indices_count {
            self.request_page(self.first_chunk_index + self.chunks.len(), &request_data);
            self.chunks.push_back(None);
        }
    }

    fn handle(&mut self, msg: Self::Message) -> bool {
        match msg {
            PaginatedDataMessage::Size(size) => {
                self.size = Some(size);
                true
            }
            PaginatedDataMessage::Page { index, values } => {
                if index < self.first_chunk_index {
                    return false;
                }
                if let Some(page) = self.chunks.get_mut(index - self.first_chunk_index) {
                    *page = Some(values);
                    true
                } else {
                    false
                }
            }
        }
    }
}

struct PaginatedDataIterator<'a, T> {
    data: &'a PaginatedData<T>,
    index: usize,
    size: usize,
}

impl<'a, T> Iterator for PaginatedDataIterator<'a, T> {
    type Item = Option<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.size {
            return None;
        } else {
            let value = self.data.item_at(self.index);
            self.index += 1;
            Some(value)
        }
    }
}

#[derive(Clone)]
struct Versioned<T>(usize, T);

impl<T> Versioned<T> {
    fn new(value: T) -> Self {
        Versioned(0, value)
    }

    fn update<R>(&self, new_value: R) -> Versioned<R> {
        Versioned(self.0.wrapping_add(1), new_value)
    }

    fn with_data<R>(&self, new_value: R) -> Versioned<R> {
        Versioned(self.0, new_value)
    }

    fn same_version<R>(&self, other: &Versioned<R>) -> bool {
        self.0 != other.0
    }

    fn as_ref(&self) -> Versioned<&T> {
        Versioned(self.0, &self.1)
    }

    fn map<R>(self, f: impl FnOnce(T) -> R) -> Versioned<R> {
        Versioned(self.0, f(self.1))
    }

    fn get(&self) -> &T {
        &self.1
    }

    fn unwrap(self) -> T {
        self.1
    }
}

trait DataProvider {
    type Request;

    fn request(&self, request: Versioned<Self::Request>);
}

struct InteractiveList<'a, T: DataView<'a>, P: DataProvider<Request = T::Request>> {
    _lifetime: std::marker::PhantomData<&'a ()>,
    provider: Versioned<Option<P>>,
    data: T,
    selection: usize,
    offset: usize,
    window_size: usize,
    scroll_margin: usize,
}

impl<'a, T: DataView<'a>, P: DataProvider<Request = T::Request>> InteractiveList<'a, T, P> {
    const DEFAULT_SCROLL_MARGIN: usize = 3;

    fn new(window_size: usize) -> Self {
        InteractiveList {
            _lifetime: std::marker::PhantomData,
            provider: Versioned::new(None),
            data: T::init(|_| ()),
            selection: 0,
            offset: 0,
            window_size,
            scroll_margin: Self::DEFAULT_SCROLL_MARGIN,
        }
    }

    fn set_provider(&mut self, provider: P) {
        self.provider.update(Some(provider));
        self.offset = 0;
        self.data = T::init(|request| request_data(&self.provider, request));
    }

    fn handle_data(&mut self, msg: Versioned<T::Message>) -> bool {
        let previous_size = self.data.size();
        if !self.provider.same_version(&msg) || self.data.handle(msg.unwrap()) {
            return false;
        };
        if previous_size.is_none() && self.data.size().is_some() {
            let provider = &self.provider;
            self.data
                .update(self.offset..(self.offset + self.window_size), |request| {
                    request_data(provider, request)
                });
        }
        true
    }

    fn move_cursor(&mut self, offset: isize) {
        let size = match self.data.size() {
            Some(size) => size,
            None => return,
        };
        if offset < 0 {
            self.selection = self.selection.saturating_sub(offset.abs() as usize);
        } else {
            self.selection = (self.selection)
                .saturating_add(offset as usize)
                .max(size.saturating_sub(1));
        }

        let new_offset = if self.selection < self.offset + self.scroll_margin {
            Some(self.selection.saturating_sub(self.scroll_margin))
        } else if self.selection
            > (self.offset + self.window_size).saturating_sub(self.scroll_margin + 1)
        {
            Some((self.selection + self.scroll_margin + 1).saturating_sub(self.window_size))
        } else {
            None
        };

        if let Some(offset) = new_offset {
            let provider = &self.provider;
            self.data
                .update(offset..(offset + self.window_size), |request| {
                    request_data(provider, request)
                });
        }
    }
}

fn request_data<P: DataProvider>(provider: &Versioned<Option<P>>, message: P::Request) {
    let message = provider.with_data(message);
    provider.as_ref().map(|provider| {
        if let Some(provider) = provider {
            provider.request(message)
        }
    });
}
