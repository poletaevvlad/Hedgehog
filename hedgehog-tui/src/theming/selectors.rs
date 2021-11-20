use crate::status::Severity;
use bitflags::bitflags;
use cmd_parser::CmdParsable;
use hedgehog_player::state::PlaybackStatus;
use std::{borrow::Borrow, str::FromStr};

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

fn split_selector_str(mut input: &str) -> Vec<&str> {
    let mut sections = Vec::new();
    while let Some(position) = input[1..].find(|ch| ch == ':' || ch == '.') {
        sections.push(&input[..=position]);
        input = &input[(position + 1)..];
        if sections.len() > 4 {
            break;
        }
    }
    sections.push(input);
    sections
}

impl StatusBar {
    fn parse(input: &[&str]) -> Result<StatusBar, SelectorParsingError> {
        match input {
            [".empty"] => Ok(StatusBar::Empty),
            [".command"] => Ok(StatusBar::Command),
            [".command", ".prompt"] => Ok(StatusBar::CommandPrompt),
            [".status"] => Ok(StatusBar::Status(None)),
            [".status", ".error"] => Ok(StatusBar::Status(Some(Severity::Error))),
            [".status", ".warning"] => Ok(StatusBar::Status(Some(Severity::Warning))),
            [".status", ".information"] => Ok(StatusBar::Status(Some(Severity::Information))),
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
        const FOCUSED = 0b0001;
        const ACTIVE = 0b0010;
        const SELECTED = 0b0100;
        const NEW = 0b1000;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ListSubitem {
    MissingTitle,
    ErrorIndicator,
    LoadingIndicator,
    UpdateIndicator,
    Date,
    NewIndicator,
    Duration,
    EpisodeNumber,
}

impl ListSubitem {
    pub(crate) fn enumerate() -> impl IntoIterator<Item = Self> {
        [
            ListSubitem::MissingTitle,
            ListSubitem::ErrorIndicator,
            ListSubitem::LoadingIndicator,
            ListSubitem::UpdateIndicator,
            ListSubitem::Date,
            ListSubitem::NewIndicator,
            ListSubitem::Duration,
            ListSubitem::EpisodeNumber,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum List {
    Divider,
    Item(ListState, Option<ListSubitem>),
}

impl List {
    fn parse(mut input: &[&str]) -> Result<List, SelectorParsingError> {
        match input {
            [".divider"] => Ok(List::Divider),
            [".item", ..] => {
                let mut state = ListState::empty();
                input = &input[1..];
                while let Some(item) = input.get(0) {
                    let state_item = match *item {
                        ":focused" => ListState::FOCUSED,
                        ":active" => ListState::ACTIVE,
                        ":selected" => ListState::SELECTED,
                        ":new" => ListState::NEW,
                        _ => break,
                    };
                    state |= state_item;
                    input = &input[1..];
                }

                let subitem = match input {
                    [] => None,
                    [".missing"] => Some(ListSubitem::MissingTitle),
                    [".loading"] => Some(ListSubitem::LoadingIndicator),
                    [".update"] => Some(ListSubitem::LoadingIndicator),
                    [".error"] => Some(ListSubitem::ErrorIndicator),
                    [".date"] => Some(ListSubitem::Date),
                    [".new"] => Some(ListSubitem::NewIndicator),
                    [".duration"] => Some(ListSubitem::Duration),
                    [".episodenumber"] => Some(ListSubitem::EpisodeNumber),
                    _ => return Err(SelectorParsingError),
                };
                Ok(List::Item(state, subitem))
            }
            _ => Err(SelectorParsingError),
        }
    }
}

impl StyleSelector for List {
    fn for_each_overrides(&self, mut callback: impl FnMut(Self)) {
        let mut callback = |selector| {
            if &selector != self {
                callback(selector);
            }
        };

        if let List::Item(item, subitem) = self {
            for bits in 0..ListState::all().bits {
                let current = ListState::from_bits_truncate(bits);
                if current.contains(*item) {
                    if subitem.is_some() {
                        callback(List::Item(current, *subitem));
                    } else {
                        callback(List::Item(current, None));
                        for subitem in ListSubitem::enumerate() {
                            callback(List::Item(current, Some(subitem)));
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Player {
    Title,
    Status(Option<PlaybackStatus>),
    Timing,
}

impl Player {
    fn parse(input: &[&str]) -> Result<Player, SelectorParsingError> {
        match input {
            [".title"] => Ok(Player::Title),
            [".timing"] => Ok(Player::Timing),
            [".status"] => Ok(Player::Status(None)),
            [".status", ".none"] => Ok(Player::Status(Some(PlaybackStatus::None))),
            [".status", ".buffering"] => Ok(Player::Status(Some(PlaybackStatus::Buffering))),
            [".status", ".playing"] => Ok(Player::Status(Some(PlaybackStatus::Playing))),
            [".status", ".paused"] => Ok(Player::Status(Some(PlaybackStatus::Paused))),
            _ => Err(SelectorParsingError),
        }
    }
}

impl StyleSelector for Player {
    fn for_each_overrides(&self, mut callback: impl FnMut(Self)) {
        if let Player::Status(None) = self {
            for status in PlaybackStatus::enumerate() {
                callback(Player::Status(Some(status)));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Selector {
    StatusBar(StatusBar),
    List(List),
    Player(Player),
}

impl CmdParsable for Selector {
    fn parse_cmd_raw(input: &str) -> Result<(Self, &str), cmd_parser::ParseError<'_>> {
        let (token, input) = cmd_parser::take_token(input);
        match token
            .as_ref()
            .map(|selector| Selector::from_str(selector.borrow()))
        {
            None => Err(cmd_parser::ParseError {
                kind: cmd_parser::ParseErrorKind::TokenRequired,
                expected: "selector".into(),
            }),
            Some(Ok(selector)) => Ok((selector, input)),
            Some(Err(_)) => Err(cmd_parser::ParseError {
                kind: cmd_parser::ParseErrorKind::TokenParse(token.unwrap(), None),
                expected: "selector".into(),
            }),
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
            Selector::Player(selector) => {
                selector.for_each_overrides(|sel| callback(Selector::Player(sel)))
            }
        }
    }
}

impl FromStr for Selector {
    type Err = SelectorParsingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = split_selector_str(s);
        match split.get(0) {
            Some(&"statusbar") => StatusBar::parse(&split[1..]).map(Selector::StatusBar),
            Some(&"list") => List::parse(&split[1..]).map(Selector::List),
            Some(&"player") => Player::parse(&split[1..]).map(Selector::Player),
            _ => Err(SelectorParsingError),
        }
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

impl From<Player> for Selector {
    fn from(player: Player) -> Self {
        Selector::Player(player)
    }
}

#[cfg(test)]
mod tests {
    use super::{List, ListState, ListSubitem, Player, Selector, StatusBar};
    use crate::status::Severity;
    use cmd_parser::CmdParsable;
    use hedgehog_player::state::PlaybackStatus;

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
            Selector::parse_cmd_full("list.divider").unwrap(),
            Selector::List(List::Divider)
        );
        assert_eq!(
            "player.timing".parse(),
            Ok(Selector::Player(Player::Timing))
        );
        assert_eq!(
            "player.status".parse(),
            Ok(Selector::Player(Player::Status(None)))
        );
        assert_eq!(
            "player.status.playing".parse(),
            Ok(Selector::Player(Player::Status(Some(
                PlaybackStatus::Playing
            ))))
        );
    }

    #[test]
    fn parse_cmd_selector() {
        assert_eq!(
            Selector::parse_cmd("statusbar.status").unwrap(),
            (Selector::StatusBar(StatusBar::Status(None)), "")
        );
    }

    #[test]
    fn parse_item_state() {
        assert_eq!(
            "list.item".parse(),
            Ok(Selector::List(List::Item(ListState::empty(), None)))
        );
        assert_eq!(
            "list.item:active".parse(),
            Ok(Selector::List(List::Item(ListState::ACTIVE, None)))
        );
        assert_eq!(
            "list.item:focused:selected".parse(),
            Ok(Selector::List(List::Item(
                ListState::FOCUSED | ListState::SELECTED,
                None
            )))
        );
        assert_eq!(
            "list.item.missing".parse(),
            Ok(Selector::List(List::Item(
                ListState::empty(),
                Some(ListSubitem::MissingTitle)
            )))
        );
        assert_eq!(
            "list.item:selected.missing".parse(),
            Ok(Selector::List(List::Item(
                ListState::SELECTED,
                Some(ListSubitem::MissingTitle)
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
