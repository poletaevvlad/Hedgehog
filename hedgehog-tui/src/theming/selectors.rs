use super::parser::{match_take, ParsableStr};
use crate::status::Severity;
use bitflags::bitflags;
use std::str::FromStr;

#[derive(Debug, thiserror::Error, Clone, Copy, PartialEq, Eq)]
#[error("selector is not recognized")]
pub(crate) struct SelectorParsingError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StatusBar {
    Empty,
    Command,
    CommandPrompt,
    Status(Option<Severity>),
}

impl StatusBar {
    fn parse(input: &mut ParsableStr<'_>) -> Result<StatusBar, SelectorParsingError> {
        input.take_token(".");
        match_take! {
            input,
            "empty" => Ok(StatusBar::Empty),
            "command.prompt" => Ok(StatusBar::CommandPrompt),
            "command" => Ok(StatusBar::Command),
            "status.error" => Ok(StatusBar::Status(Some(Severity::Error))),
            "status.warning" => Ok(StatusBar::Status(Some(Severity::Warning))),
            "status.information" => Ok(StatusBar::Status(Some(Severity::Information))),
            "status" => Ok(StatusBar::Status(None)),
            _ => Err(SelectorParsingError),
        }
    }
}

bitflags! {
    pub(crate) struct ListItem: usize {
        const FOCUSED = 0b001;
        const ACTIVE = 0b010;
        const SELECTED = 0b100;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum List {
    Divider,
    Item(ListItem),
}

impl List {
    fn parse(input: &mut ParsableStr<'_>) -> Result<List, SelectorParsingError> {
        input.take_token(".");
        match_take! {
            input,
            "divider" => Ok(List::Divider),
            "item" => {
                let mut state = ListItem::empty();
                loop {
                    let state_item = match_take! {
                        input,
                        ":focused" => ListItem::FOCUSED,
                        ":active" => ListItem::ACTIVE,
                        ":selected" => ListItem::SELECTED,
                        _ => break,
                    };
                    state |= state_item;
                }
                Ok(List::Item(state))
            },
            _ => Err(SelectorParsingError),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Selector {
    StatusBar(StatusBar),
    List(List),
}

impl Selector {
    fn parse(input: &mut ParsableStr<'_>) -> Result<Selector, SelectorParsingError> {
        match_take! {
            input,
            "statusbar" => StatusBar::parse(input).map(Selector::StatusBar),
            "list" => List::parse(input).map(Selector::List),
            _ => Err(SelectorParsingError),
        }
    }
}

impl FromStr for Selector {
    type Err = SelectorParsingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut input = ParsableStr::new(s);
        let selector = Selector::parse(&mut input)?;
        if !input.is_empty() {
            return Err(SelectorParsingError);
        }
        Ok(selector)
    }
}

struct SelectorDeserializeVisitor;

impl<'de> serde::de::Visitor<'de> for SelectorDeserializeVisitor {
    type Value = Selector;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("selector")
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        v.parse().map_err(E::custom)
    }
}

impl<'de> serde::Deserialize<'de> for Selector {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(SelectorDeserializeVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::{List, ListItem, Selector, StatusBar};
    use crate::cmdparser;
    use crate::status::Severity;

    #[test]
    fn parse_selectors() {
        assert_eq!(
            "statusbar.status".parse(),
            Ok(Selector::StatusBar(StatusBar::Status(None)))
        );
        assert_eq!(
            "statusbar.status.warning".parse(),
            Ok(Selector::StatusBar(StatusBar::Status(Some(
                Severity::Warning
            ))))
        );
        assert_eq!("list.divider".parse(), Ok(Selector::List(List::Divider)));

        assert_eq!(
            cmdparser::from_str::<Selector>("list.divider").unwrap(),
            Selector::List(List::Divider)
        );
    }

    #[test]
    fn parse_item_state() {
        assert_eq!(
            "list.item".parse(),
            Ok(Selector::List(List::Item(ListItem::empty())))
        );
        assert_eq!(
            "list.item:active".parse(),
            Ok(Selector::List(List::Item(ListItem::ACTIVE)))
        );
        assert_eq!(
            "list.item:focused:selected".parse(),
            Ok(Selector::List(List::Item(
                ListItem::FOCUSED | ListItem::SELECTED
            )))
        );
    }

    #[test]
    fn parse_error() {
        assert!("list.abcdef".parse::<Selector>().is_err());
        assert!("list.divider.unknown".parse::<Selector>().is_err());
        assert!("list.item:unknown".parse::<Selector>().is_err());
    }
}
