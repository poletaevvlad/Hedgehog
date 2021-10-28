use gstreamer as gst;
use gstreamer::prelude::*;
use std::borrow::Cow;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub(crate) enum GstError {
    Error(Box<dyn Error + Send + 'static>),
    Text(Cow<'static, str>),
}

impl GstError {
    pub(crate) fn from_err<E: Error + Send + 'static>(error: E) -> Self {
        GstError::Error(Box::new(error))
    }

    pub(crate) fn from_str<T: Into<Cow<'static, str>>>(text: T) -> Self {
        GstError::Text(text.into())
    }
}

impl Error for GstError {}

impl fmt::Display for GstError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GstError::Error(error) => error.fmt(f),
            GstError::Text(text) => text.fmt(f),
        }
    }
}

pub(crate) fn get_property<T>(element: &gst::Element, name: &str) -> Result<T, GstError>
where
    for<'a> T: gstreamer::glib::value::FromValue<'a>,
{
    element
        .property(name)
        .map_err(GstError::from_err)
        .and_then(|value| value.get().map_err(GstError::from_err))
}

pub(crate) fn set_property<V>(
    element: &mut gst::Element,
    name: &str,
    value: V,
) -> Result<(), GstError>
where
    V: gstreamer::glib::value::ToValue,
{
    element
        .set_property(name, value)
        .map_err(GstError::from_err)
}
