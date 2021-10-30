use gstreamer_base::gst::prelude::*;
use gstreamer_base::{glib, gst};
use std::borrow::Cow;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum GstError {
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
    for<'a> T: glib::value::FromValue<'a>,
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
    V: glib::value::ToValue,
{
    element
        .set_property(name, value)
        .map_err(GstError::from_err)
}

pub(crate) fn build_flags<'a>(
    type_name: &str,
    flags: impl IntoIterator<Item = &'a str>,
) -> Result<glib::Value, GstError> {
    let flags_type = glib::Type::from_name(type_name)
        .ok_or_else(|| GstError::from_str(format!("{} type not found", type_name)))?;
    let flags_class = glib::FlagsClass::new(flags_type)
        .ok_or_else(|| GstError::from_str(format!("Cannot construct flags from {}", type_name)))?;

    let mut builder = flags_class.builder();
    for flag in flags {
        builder = builder.set_by_nick(flag);
    }

    builder
        .build()
        .ok_or_else(|| GstError::from_str("Cannot construct flags"))
}
