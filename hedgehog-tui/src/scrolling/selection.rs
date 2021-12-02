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

pub(crate) struct FindPrevious;

impl<D: DataView> UpdateStrategy<D> for FindPrevious
where
    D::Item: Identifiable,
{
    type Temp = Option<<D::Item as Identifiable>::Id>;

    fn before_update(viewport: &Viewport, data: &D) -> Self::Temp {
        data.item_at(viewport.selected_index())
            .map(Identifiable::id)
    }

    fn update(viewport: &mut Viewport, data: &D, id: Self::Temp) {
        let selection = id
            .and_then(|id| data.find(|item| item.id() == id))
            .unwrap_or(0);
        viewport.update(selection, data.size());
    }
}
