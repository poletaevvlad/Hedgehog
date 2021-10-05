use std::cmp::Ordering;
use std::collections::HashMap;

pub trait Identifiable {
    type Id: Eq + std::hash::Hash + Clone;

    fn as_id(&self) -> Self::Id;

    fn same_as(&self, other: &Self) -> bool {
        self.as_id() == other.as_id()
    }
}

pub trait OrderProvider {
    type Type;

    fn compare(&self, first: &Self::Type, second: &Self::Type) -> Ordering;
    fn ordering_changed(&self, before: &Self::Type, after: &Self::Type) -> bool;
}

pub enum UpdateOperation<T: Identifiable> {
    Set(T),
    Delete(T::Id),
}

pub struct UpdatableCollection<T: Identifiable, O> {
    items: HashMap<T::Id, T>,
    ordering: Vec<T::Id>,
    order_provider: O,
}

fn result_value<T>(result: Result<T, T>) -> T {
    match result {
        Ok(value) => value,
        Err(value) => value,
    }
}

impl<T: Identifiable, O: OrderProvider<Type = T>> UpdatableCollection<T, O> {
    pub fn new(order_provider: O) -> Self {
        UpdatableCollection {
            items: HashMap::new(),
            ordering: Vec::new(),
            order_provider,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        UpdatableCollectionIter {
            items: &self.items,
            order: self.ordering.iter(),
        }
    }

    pub fn update(&mut self, update: UpdateOperation<T>) {
        match update {
            UpdateOperation::Set(item) => {
                // TODO: check wheather the item needs repositioning
                let id = item.as_id();
                if let Some(existing) = self.items.get(&id) {
                    if self.order_provider.ordering_changed(existing, &item) {
                        let position = self
                            .ordering
                            .iter()
                            .position(|item_id| item_id == &id)
                            .unwrap();
                        self.ordering.remove(position);
                    }
                }
                let ordering_index = result_value(self.ordering.binary_search_by(|other| {
                    self.order_provider
                        .compare(self.items.get(other).unwrap(), &item)
                }));
                self.ordering.insert(ordering_index, id.clone());
                self.items.insert(id, item);
            }
            UpdateOperation::Delete(id) => {
                self.items.remove(&id);
                let position = self
                    .ordering
                    .iter()
                    .position(|item_id| item_id == &id)
                    .unwrap();
                self.ordering.remove(position);
            }
        }
    }

    pub fn set_ordered(&mut self, items: impl IntoIterator<Item = T>) {
        self.items.clear();
        self.ordering.clear();

        let iterator = items.into_iter();
        let size_hint = iterator.size_hint().0;
        self.ordering.reserve(size_hint);
        self.items.reserve(size_hint);

        for item in iterator {
            let id = item.as_id();
            self.ordering.push(id.clone());
            self.items.insert(id, item);
        }
    }
}

struct UpdatableCollectionIter<'a, T: Identifiable, I> {
    items: &'a HashMap<T::Id, T>,
    order: I,
}

impl<'a, T: Identifiable, I: Iterator<Item = &'a T::Id>> Iterator
    for UpdatableCollectionIter<'a, T, I>
{
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        self.order.next().map(|id| self.items.get(id).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::{Identifiable, OrderProvider, UpdatableCollection, UpdateOperation};

    #[derive(Debug, PartialEq, Eq, Clone)]
    struct Item(u32, &'static str);

    impl Identifiable for Item {
        type Id = u32;

        fn as_id(&self) -> Self::Id {
            self.0
        }
    }

    struct ItemOrderProvider;

    impl OrderProvider for ItemOrderProvider {
        type Type = Item;

        fn compare(&self, first: &Self::Type, second: &Self::Type) -> std::cmp::Ordering {
            first.1.cmp(second.1)
        }

        fn ordering_changed(&self, before: &Self::Type, after: &Self::Type) -> bool {
            before.1 != after.1
        }
    }

    fn assert_collection(
        collection: &UpdatableCollection<Item, ItemOrderProvider>,
        items: &[Item],
    ) {
        let actual: Vec<Item> = collection.iter().cloned().collect();
        assert_eq!(&actual, items);
    }

    #[test]
    fn updating_values() {
        let mut collection = UpdatableCollection::new(ItemOrderProvider);
        assert_collection(&collection, &[]);

        collection.update(UpdateOperation::Set(Item(10, "a")));
        collection.update(UpdateOperation::Set(Item(20, "c")));
        collection.update(UpdateOperation::Set(Item(30, "b")));
        assert_collection(&collection, &[Item(10, "a"), Item(30, "b"), Item(20, "c")]);

        collection.update(UpdateOperation::Set(Item(30, "d")));
        assert_collection(&collection, &[Item(10, "a"), Item(20, "c"), Item(30, "d")]);

        collection.update(UpdateOperation::Delete(20));
        assert_collection(&collection, &[Item(10, "a"), Item(30, "d")]);
    }

    #[test]
    fn set_ordered() {
        let mut collection = UpdatableCollection::new(ItemOrderProvider);
        collection.set_ordered(vec![Item(10, "a"), Item(30, "b"), Item(20, "c")]);
        assert_collection(&collection, &[Item(10, "a"), Item(30, "b"), Item(20, "c")]);
    }
}
