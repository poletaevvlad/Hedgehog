use super::{DataView, Viewport};
use hedgehog_library::model::Identifiable;

pub(crate) trait UpdateStrategy<D: DataView> {
    type Temp;

    fn before_update(viewport: &Viewport, data: &D) -> Self::Temp;
    fn update(viewport: &mut Viewport, data: &D, temp: Self::Temp);
}

pub(crate) struct Keep;

impl<D: DataView> UpdateStrategy<D> for Keep {
    type Temp = ();

    fn before_update(_viewport: &Viewport, _data: &D) -> Self::Temp {}

    fn update(viewport: &mut Viewport, data: &D, _temp: Self::Temp) {
        let items_count = data.size();
        viewport.update(
            viewport.selected_index().min(items_count.saturating_sub(1)),
            items_count,
        );
    }
}

pub(crate) struct Reset;

impl<D: DataView> UpdateStrategy<D> for Reset {
    type Temp = ();

    fn before_update(_viewport: &Viewport, _data: &D) -> Self::Temp {}

    fn update(viewport: &mut Viewport, data: &D, _temp: Self::Temp) {
        viewport.update(0, data.size());
    }
}

pub(crate) struct DoNotUpdate;

impl<D: DataView> UpdateStrategy<D> for DoNotUpdate {
    type Temp = ();

    fn before_update(_viewport: &Viewport, _data: &D) -> Self::Temp {}

    fn update(_viewport: &mut Viewport, _data: &D, _temp: Self::Temp) {}
}

pub(crate) struct FindPrevious<F = Reset>(F);

impl<D: DataView, F: UpdateStrategy<D>> UpdateStrategy<D> for FindPrevious<F>
where
    D::Item: Identifiable,
{
    type Temp = (Option<<D::Item as Identifiable>::Id>, F::Temp);

    fn before_update(viewport: &Viewport, data: &D) -> Self::Temp {
        let item_id = data
            .item_at(viewport.selected_index())
            .map(Identifiable::id);
        let fallback = F::before_update(viewport, data);
        (item_id, fallback)
    }

    fn update(viewport: &mut Viewport, data: &D, temp: Self::Temp) {
        let found_selection = temp.0.and_then(|id| data.find(|item| item.id() == id));
        match found_selection {
            Some(selection) => viewport.update(selection, data.size()),
            None => F::update(viewport, data, temp.1),
        }
    }
}
