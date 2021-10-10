use std::collections::VecDeque;

use serde::{de, Deserialize};
use shlex::Shlex;

#[derive(thiserror::Error, Debug)]
pub(crate) enum Error {
    #[error("unexpected component")]
    UnexpectedComponent,

    #[error("component required")]
    ComponentRequired,

    #[error("boolean required")]
    BooleanRequired,

    #[error("integer required ({0})")]
    InvalidInt(#[from] std::num::ParseIntError),

    #[error("floating point number required ({0})")]
    InvalidFloat(#[from] std::num::ParseFloatError),

    #[error("character required")]
    InvalidChar,

    #[error("deserialization unsupported for data type {0}")]
    UnsupportedDataType(&'static str),

    #[error("{0}")]
    Custom(String),
}

impl de::Error for Error {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Error::Custom(msg.to_string())
    }
}

type Result<T> = std::result::Result<T, Error>;

pub(crate) struct Deserializer {
    tokens: VecDeque<String>,
}

impl Deserializer {
    pub(crate) fn from_str(input: &str) -> Self {
        Deserializer {
            tokens: Shlex::new(input).collect(),
        }
    }

    fn consume(&mut self) -> Result<String> {
        self.tokens.pop_front().ok_or(Error::ComponentRequired)
    }
}

pub(crate) fn from_str<'a, T>(input: &'a str) -> Result<T>
where
    T: Deserialize<'a>,
{
    let mut deserializer = Deserializer::from_str(input);
    let result = T::deserialize(&mut deserializer)?;
    match deserializer.tokens.is_empty() {
        false => Err(Error::UnexpectedComponent),
        true => Ok(result),
    }
}

macro_rules! deserialize_parsable {
    ($deserialize:ident, $visit:ident) => {
        fn $deserialize<V>(self, visitor: V) -> Result<V::Value>
        where
            V: de::Visitor<'de>,
        {
            let component = self.consume()?;
            visitor.$visit(component.parse()?)
        }
    };
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_string(self.consume()?)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.consume()?.as_str() {
            "true" | "t" | "yes" | "y" => visitor.visit_bool(true),
            "false" | "f" | "no" | "n" => visitor.visit_bool(false),
            _ => Err(Error::BooleanRequired),
        }
    }

    deserialize_parsable!(deserialize_i8, visit_i8);
    deserialize_parsable!(deserialize_i16, visit_i16);
    deserialize_parsable!(deserialize_i32, visit_i32);
    deserialize_parsable!(deserialize_i64, visit_i64);

    deserialize_parsable!(deserialize_u8, visit_u8);
    deserialize_parsable!(deserialize_u16, visit_u16);
    deserialize_parsable!(deserialize_u32, visit_u32);
    deserialize_parsable!(deserialize_u64, visit_u64);

    deserialize_parsable!(deserialize_f32, visit_f32);
    deserialize_parsable!(deserialize_f64, visit_f64);

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        let string = self.consume()?;
        let mut chars = string.chars();
        if let (Some(ch), None) = (chars.next(), chars.next()) {
            visitor.visit_char(ch)
        } else {
            Err(Error::InvalidChar)
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_string(self.consume()?)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_string(self.consume()?)
    }

    fn deserialize_bytes<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::UnsupportedDataType("bytes"))
    }

    fn deserialize_byte_buf<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::UnsupportedDataType("bytes_buf"))
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        match self.tokens.is_empty() {
            true => visitor.visit_none(),
            false => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_unit()
    }

    fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(SeqAccess::new(self))
    }

    fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(TupleAccess::new(self, len))
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(TupleAccess::new(self, len))
    }

    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        Err(Error::UnsupportedDataType("map"))
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(TupleAccess::new(self, fields.len()))
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_enum(EnumAccess::new(self))
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_string(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

struct TupleAccess<'a> {
    deserializer: &'a mut Deserializer,
    count: usize,
}

impl<'a> TupleAccess<'a> {
    fn new(deserializer: &'a mut Deserializer, count: usize) -> Self {
        TupleAccess {
            deserializer,
            count,
        }
    }
}

impl<'a, 'de> de::SeqAccess<'de> for TupleAccess<'a> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        if self.count == 0 {
            return Ok(None);
        }
        self.count -= 1;

        seed.deserialize(&mut *self.deserializer).map(Some)
    }
}

struct SeqAccess<'a> {
    deserializer: &'a mut Deserializer,
}

impl<'a> SeqAccess<'a> {
    fn new(deserializer: &'a mut Deserializer) -> Self {
        SeqAccess { deserializer }
    }
}

impl<'a, 'de> de::SeqAccess<'de> for SeqAccess<'a> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>>
    where
        T: de::DeserializeSeed<'de>,
    {
        if self.deserializer.tokens.is_empty() {
            return Ok(None);
        }
        seed.deserialize(&mut *self.deserializer).map(Some)
    }
}

struct EnumAccess<'a> {
    deserializer: &'a mut Deserializer,
}

impl<'a> EnumAccess<'a> {
    fn new(deserializer: &'a mut Deserializer) -> Self {
        EnumAccess { deserializer }
    }
}

impl<'a, 'de> de::EnumAccess<'de> for EnumAccess<'a> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant)>
    where
        V: de::DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.deserializer)
            .map(|value| (value, self))
    }
}

impl<'a, 'de> de::VariantAccess<'de> for EnumAccess<'a> {
    type Error = Error;

    fn unit_variant(self) -> Result<()> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value>
    where
        T: de::DeserializeSeed<'de>,
    {
        seed.deserialize(&mut *self.deserializer)
    }

    fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(TupleAccess::new(self.deserializer, len))
    }

    fn struct_variant<V>(self, fields: &'static [&'static str], visitor: V) -> Result<V::Value>
    where
        V: de::Visitor<'de>,
    {
        visitor.visit_seq(TupleAccess::new(self.deserializer, fields.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::from_str;
    use serde::Deserialize;

    #[derive(Debug, PartialEq, Deserialize)]
    enum MockList {
        Nil,
        List(u16, Box<MockList>),
    }

    #[derive(Debug, PartialEq, Deserialize)]
    struct UnitStruct;

    #[derive(Debug, PartialEq, Deserialize)]
    struct NonTupleStruct {
        first: u8,
        second: u8,
    }

    #[derive(Debug, PartialEq, Deserialize)]
    enum MockCommand {
        EnumUnion,
        WithPrimitiveTypes(u8, i8, u16, i16, u32, i32, u64, u64, f32, f64, bool),
        WithTuples((usize, usize), (usize, usize), String),
        WithOptional(Option<String>, Option<String>, Option<String>),
        WithNested(MockList),
        WithSequence(Vec<u8>),
        WithUnits((), u8, UnitStruct),
        WithStruct { a: u8, b: NonTupleStruct, c: u8 },
    }

    #[test]
    fn deserialize_enum_union() {
        let command: MockCommand = from_str("EnumUnion").unwrap();
        assert_eq!(command, MockCommand::EnumUnion);
    }

    #[test]
    fn deserialize_primitive_types() {
        let command: MockCommand =
            from_str("WithPrimitiveTypes 1 2 3 4 5 6 7 8 9.0 10.0 true").unwrap();
        assert_eq!(
            command,
            MockCommand::WithPrimitiveTypes(1, 2, 3, 4, 5, 6, 7, 8, 9.0, 10.0, true)
        );
    }

    #[test]
    fn deserialize_with_tuples() {
        let command: MockCommand =
            from_str("WithTuples 10 11 20 21 string-without-quotes").unwrap();
        assert_eq!(
            command,
            MockCommand::WithTuples((10, 11), (20, 21), "string-without-quotes".to_string())
        );
    }

    #[test]
    fn deserialize_with_optional() {
        let command: MockCommand = from_str("WithOptional first 'second with spaces'").unwrap();
        assert_eq!(
            command,
            MockCommand::WithOptional(
                Some("first".to_string()),
                Some("second with spaces".to_string()),
                None
            )
        );
    }

    #[test]
    fn deserialize_with_nested() {
        let command: MockCommand = from_str("WithNested List 1 List 2 Nil").unwrap();
        assert_eq!(
            command,
            MockCommand::WithNested(MockList::List(
                1,
                Box::new(MockList::List(2, Box::new(MockList::Nil)))
            ))
        );
    }

    #[test]
    fn deserialize_sequence() {
        let command: MockCommand = from_str("WithSequence 1 2 3 4 5").unwrap();
        assert_eq!(command, MockCommand::WithSequence(vec![1, 2, 3, 4, 5]));
    }

    #[test]
    fn deserialize_union() {
        let command: MockCommand = from_str("WithUnits 5").unwrap();
        assert_eq!(command, MockCommand::WithUnits((), 5, UnitStruct));
    }

    #[test]
    fn deserialize_struct() {
        let command: MockCommand = from_str("WithStruct 1 2 3 4").unwrap();
        assert_eq!(
            command,
            MockCommand::WithStruct {
                a: 1,
                b: NonTupleStruct {
                    first: 2,
                    second: 3
                },
                c: 4
            }
        );
    }
}
