use std::collections::VecDeque;
use std::ops::Range;

trait DataSource<'a> {
    type Item: 'a;
    type Iter: Iterator<Item = Option<&'a Self::Item>>;

    fn iter(&'a self, offset: usize) -> Option<Self::Iter>;
    fn size(&self) -> Option<usize>;
    fn update(&mut self, range: Range<usize>);
}

struct ListData<T> {
    items: Option<Vec<T>>,
}

impl<'a, T> DataSource<'a> for ListData<T>
where
    Self: 'a,
{
    type Item = T;
    type Iter = ListDataIterator<'a, T>;

    fn iter(&'a self, offset: usize) -> Option<Self::Iter> {
        self.items
            .as_ref()
            .map(|items| ListDataIterator(items[offset..].iter()))
    }

    fn size(&self) -> Option<usize> {
        self.items.as_ref().map(Vec::len)
    }

    fn update(&mut self, _range: Range<usize>) {}
}

struct ListDataIterator<'a, T>(std::slice::Iter<'a, T>);

impl<'a, T> Iterator for ListDataIterator<'a, T> {
    type Item = Option<&'a T>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(Some)
    }
}

struct PaginatedData<T> {
    page_size: usize,
    margin_size: usize,
    size: Option<usize>,
    first_chunk_index: usize,
    chunks: VecDeque<Option<Vec<T>>>,
}

impl<T> PaginatedData<T> {
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
}

impl<'a, T> DataSource<'a> for PaginatedData<T>
where
    Self: 'a,
{
    type Item = T;
    type Iter = PaginatedDataIterator<'a, T>;

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

    fn update(&mut self, range: Range<usize>) {
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
            }

            if self.chunks.len() > indices_count {
                self.chunks.drain(indices_count..);
            }
        } else {
            self.first_chunk_index = first_required_page;
        }
        while self.chunks.len() < indices_count {
            self.chunks.push_back(None);
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
