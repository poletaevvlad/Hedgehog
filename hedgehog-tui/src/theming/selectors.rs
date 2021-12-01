use crate::status::Severity;
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
    Confirmation,
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
            [".confirmation"] => Ok(StatusBar::Confirmation),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ListColumn {
    StateIndicator,
    Title,
    EpisodeNumber,
    Duration,
    Date,
    Loading,
}

impl ListColumn {
    fn enumerate() -> impl IntoIterator<Item = Self> {
        [
            ListColumn::StateIndicator,
            ListColumn::Title,
            ListColumn::EpisodeNumber,
            ListColumn::Duration,
            ListColumn::Date,
            ListColumn::Loading,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum ListState {
    Feed,
    FeedUpdating,
    FeedError,
    FeedSpecial,
    Episode,
    EpisodeError,
    EpisodePlaying,
    EpisodeFinished,
    EpisodeNew,
    EpisodeStarted,
}

impl ListState {
    fn for_each(state: Option<Self>, mut callback: impl FnMut(Option<Self>)) {
        callback(state);
        match state {
            None => {
                callback(Some(ListState::Feed));
                callback(Some(ListState::FeedUpdating));
                callback(Some(ListState::FeedError));
                callback(Some(ListState::FeedSpecial));
                callback(Some(ListState::Episode));
                callback(Some(ListState::EpisodeError));
                callback(Some(ListState::EpisodePlaying));
                callback(Some(ListState::EpisodeNew));
                callback(Some(ListState::EpisodeStarted));
                callback(Some(ListState::EpisodeFinished));
            }
            Some(ListState::Feed) => {
                callback(Some(ListState::FeedUpdating));
                callback(Some(ListState::FeedError));
                callback(Some(ListState::FeedSpecial));
            }
            Some(ListState::Episode) => {
                callback(Some(ListState::EpisodeError));
                callback(Some(ListState::EpisodePlaying));
                callback(Some(ListState::EpisodeNew));
                callback(Some(ListState::EpisodeStarted));
                callback(Some(ListState::EpisodeFinished));
            }
            _ => {}
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ListItem {
    pub(crate) selected: bool,
    pub(crate) focused: bool,
    pub(crate) missing_title: bool,
    pub(crate) state: Option<ListState>,
    pub(crate) column: Option<ListColumn>,
}

impl ListItem {
    pub(crate) fn with_column(&self, column: ListColumn) -> Self {
        ListItem {
            column: Some(column),
            ..*self
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum List {
    Divider,
    Item(ListItem),
}

impl List {
    fn parse(mut input: &[&str]) -> Result<List, SelectorParsingError> {
        match input {
            [".divider"] => Ok(List::Divider),
            [".item", ..] => {
                let mut list_item = ListItem::default();

                input = &input[1..];
                while let Some(item) = input.get(0) {
                    match *item {
                        ":focused" => list_item.focused = true,
                        ":selected" => list_item.selected = true,
                        ":missing-title" => list_item.missing_title = true,
                        item => {
                            let new_state = match item {
                                ":feed" => ListState::Feed,
                                ":feed-updating" => ListState::FeedUpdating,
                                ":feed-error" => ListState::FeedError,
                                ":feed-special" => ListState::FeedSpecial,
                                ":episode" => ListState::Episode,
                                ":episode-error" => ListState::EpisodeError,
                                ":episode-playing" => ListState::EpisodePlaying,
                                ":episode-new" => ListState::EpisodeNew,
                                ":episode-started" => ListState::EpisodeStarted,
                                _ => break,
                            };
                            if list_item.state.is_some() {
                                return Err(SelectorParsingError);
                            }
                            list_item.state = Some(new_state);
                        }
                    };
                    input = &input[1..];
                }

                list_item.column = match input {
                    [] => None,
                    [".state"] => Some(ListColumn::StateIndicator),
                    [".title"] => Some(ListColumn::Title),
                    [".episode-number"] => Some(ListColumn::EpisodeNumber),
                    [".duration"] => Some(ListColumn::Duration),
                    [".date"] => Some(ListColumn::Date),
                    [".loading"] => Some(ListColumn::Loading),
                    _ => return Err(SelectorParsingError),
                };

                Ok(List::Item(list_item))
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

        if let List::Item(item) = self {
            let selected_variants: &[bool] = if item.selected {
                &[true]
            } else {
                &[true, false]
            };
            let focused_variants: &[bool] = if item.focused {
                &[true]
            } else {
                &[true, false]
            };
            let missing_variants: &[bool] = if item.missing_title {
                &[true]
            } else {
                &[true, false]
            };

            for selected in selected_variants {
                for focused in focused_variants {
                    for missing in missing_variants {
                        ListState::for_each(item.state, |state| {
                            let new_item = ListItem {
                                selected: *selected,
                                focused: *focused,
                                missing_title: *missing,
                                state,
                                column: None,
                            };

                            if let Some(column) = item.column {
                                callback(List::Item(ListItem {
                                    column: Some(column),
                                    ..new_item
                                }));
                            } else {
                                callback(List::Item(new_item));
                                for column in ListColumn::enumerate() {
                                    callback(List::Item(ListItem {
                                        column: Some(column),
                                        ..new_item
                                    }));
                                }
                            }
                        });
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Empty {
    View,
    Title,
    Subtitle,
}

impl Empty {
    fn parse(input: &[&str]) -> Result<Empty, SelectorParsingError> {
        match input {
            [] => Ok(Empty::View),
            [".title"] => Ok(Empty::Title),
            [".subtitle"] => Ok(Empty::Subtitle),
            _ => Err(SelectorParsingError),
        }
    }
}

impl StyleSelector for Empty {
    fn for_each_overrides(&self, mut callback: impl FnMut(Self))
    where
        Self: Sized,
    {
        if let Empty::View = self {
            callback(Empty::Title);
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
    Empty(Empty),
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
                selector.for_each_overrides(|sel| callback(Selector::StatusBar(sel)));
            }
            Selector::List(selector) => {
                selector.for_each_overrides(|sel| callback(Selector::List(sel)));
            }
            Selector::Empty(selector) => {
                selector.for_each_overrides(|sel| callback(Selector::Empty(sel)));
            }
            Selector::Player(selector) => {
                selector.for_each_overrides(|sel| callback(Selector::Player(sel)));
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
            Some(&"empty") => Empty::parse(&split[1..]).map(Selector::Empty),
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

impl From<Empty> for Selector {
    fn from(empty: Empty) -> Self {
        Selector::Empty(empty)
    }
}

impl From<Player> for Selector {
    fn from(player: Player) -> Self {
        Selector::Player(player)
    }
}

#[cfg(test)]
mod tests {
    use super::{Empty, List, ListColumn, ListItem, ListState, Player, Selector, StatusBar};
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
        assert_eq!("empty".parse(), Ok(Selector::Empty(Empty::View)));
        assert_eq!("empty.title".parse(), Ok(Selector::Empty(Empty::Title)));
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
            Ok(Selector::List(List::Item(ListItem::default())))
        );
        assert_eq!(
            "list.item:episode-playing".parse(),
            Ok(Selector::List(List::Item(ListItem {
                state: Some(ListState::EpisodePlaying),
                ..Default::default()
            })))
        );
        assert_eq!(
            "list.item:focused:selected".parse(),
            Ok(Selector::List(List::Item(ListItem {
                focused: true,
                selected: true,
                ..Default::default()
            })))
        );
        assert_eq!(
            "list.item:missing-title".parse(),
            Ok(Selector::List(List::Item(ListItem {
                missing_title: true,
                ..Default::default()
            })))
        );
        assert_eq!(
            "list.item:selected.title".parse(),
            Ok(Selector::List(List::Item(ListItem {
                selected: true,
                column: Some(ListColumn::Title),
                ..Default::default()
            })))
        );
    }

    #[test]
    fn parse_error() {
        assert!("list.abcdef".parse::<Selector>().is_err());
        assert!("list.divider.unknown".parse::<Selector>().is_err());
        assert!("list.item:unknown".parse::<Selector>().is_err());
    }
}
