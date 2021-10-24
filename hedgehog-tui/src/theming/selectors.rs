use super::parser::{match_take, ParsableStr};
use crate::status::Severity;
use bitflags::bitflags;
use std::str::FromStr;

pub(crate) trait StyleSelector {
    fn for_each_overrides(&self, callback: impl FnMut(Self))
    where
        Self: Sized;
}

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

impl StyleSelector for StatusBar {
    fn for_each_overrides(&self, mut callback: impl FnMut(Self)) {
        match self {
            StatusBar::Command => callback(StatusBar::CommandPrompt),
            StatusBar::Status(None) => {
                for severity in Severity::enumerate() {
                    callback(StatusBar::Status(Some(severity)));
                }
            }
            _ => (),
        }
    }
}

bitflags! {
    pub(crate) struct ListState: usize {
        const FOCUSED = 0b001;
        const ACTIVE = 0b010;
        const SELECTED = 0b100;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum List {
    Divider,
    Item(ListState),
}

impl List {
    fn parse(input: &mut ParsableStr<'_>) -> Result<List, SelectorParsingError> {
        input.take_token(".");
        match_take! {
            input,
            "divider" => Ok(List::Divider),
            "item" => {
                let mut state = ListState::empty();
                loop {
                    let state_item = match_take! {
                        input,
                        ":focused" => ListState::FOCUSED,
                        ":active" => ListState::ACTIVE,
                        ":selected" => ListState::SELECTED,
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

impl StyleSelector for List {
    fn for_each_overrides(&self, mut callback: impl FnMut(Self)) {
        if let List::Item(item) = self {
            for bits in 0..ListState::all().bits {
                let current = ListState::from_bits_truncate(bits);
                if current != *item && current.contains(*item) {
                    callback(List::Item(current))
                }
            }
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

impl StyleSelector for Selector {
    fn for_each_overrides(&self, mut callback: impl FnMut(Self)) {
        match self {
            Selector::StatusBar(selector) => {
                selector.for_each_overrides(|sel| callback(Selector::StatusBar(sel)))
            }
            Selector::List(selector) => {
                selector.for_each_overrides(|sel| callback(Selector::List(sel)))
            }
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

impl From<StatusBar> for Selector {
    fn from(status_bar: StatusBar) -> Self {
        Selector::StatusBar(status_bar)
    }
}

impl From<List> for Selector {
    fn from(list: List) -> Self {
        Selector::List(list)
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
    use super::{List, ListState, Selector, StatusBar};
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
            Ok(Selector::List(List::Item(ListState::empty())))
        );
        assert_eq!(
            "list.item:active".parse(),
            Ok(Selector::List(List::Item(ListState::ACTIVE)))
        );
        assert_eq!(
            "list.item:focused:selected".parse(),
            Ok(Selector::List(List::Item(
                ListState::FOCUSED | ListState::SELECTED
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
