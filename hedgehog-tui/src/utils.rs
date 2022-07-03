use std::iter::Peekable;

pub(crate) fn iter_take_while<'a, T, I: Iterator<Item = T>, F: Fn(&T) -> bool + 'a>(
    iter: &'a mut Peekable<I>,
    predicate: F,
) -> impl Iterator<Item = T> + 'a {
    TakeWhile { iter, predicate }
}

struct TakeWhile<'a, I: Iterator, F> {
    iter: &'a mut Peekable<I>,
    predicate: F,
}

impl<'a, I: Iterator, F: Fn(&I::Item) -> bool + 'a> Iterator for TakeWhile<'a, I, F> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        match self.iter.peek() {
            Some(item) if (self.predicate)(item) => self.iter.next(),
            Some(_) | None => None,
        }
    }
}
