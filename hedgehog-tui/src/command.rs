use crate::cmdcontext::CommandContext;
use crate::keymap::Key;
use crate::logger::Severity;
use crate::options::OptionsUpdate;
use crate::scrolling::ScrollAction;
use crate::theming::ThemeCommand;
use cmdparse::Parsable;
use hedgehog_library::model::{EpisodeStatus, EpisodeSummaryStatus};
use hedgehog_player::volume::VolumeCommand;
use hedgehog_player::PlaybackCommand;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Parsable)]
#[cmd(ctx = "CommandContext<'_>")]
pub(crate) enum Command {
    #[cmd(rename = "line")]
    Cursor(ScrollAction),
    Map(Key, Box<Command>),
    Unmap(Key),
    Theme(ThemeCommand),
    Exec(PathBuf),
    Confirm(Box<CommandConfirmation>),
    #[cmd(transparent)]
    Volume(VolumeCommand),
    PlayCurrent,
    #[cmd(transparent)]
    Playback(PlaybackCommand),
    Finish,
    #[cmd(alias = "enable", alias = "disable")]
    SetFeedEnabled(
        #[cmd(
            alias_value(alias = "enable", value = "true"),
            alias_value(alias = "disable", value = "false")
        )]
        bool,
    ),
    #[cmd(alias = "q")]
    Quit,
    #[cmd(rename = "focus", alias = "log")]
    SetFocus(#[cmd(alias_value(alias = "log", value = "FocusedPane::ErrorsLog"))] FocusedPane),
    #[cmd(rename = "set")]
    SetOption(OptionsUpdate),
    #[cmd(rename = "add")]
    AddFeed(String),
    AddGroup(#[cmd(parser = "crate::cmdcontext::GroupNameParser")] String),
    SetGroup(#[cmd(parser = "crate::cmdcontext::GroupNameParser")] String),
    UnsetGroup,
    PlaceGroup(usize),
    #[cmd(alias = "delete-feed")]
    Delete,
    Reverse,
    Rename(#[cmd(parser = "hedgehog_library::search::SearchQueryParser")] String),
    #[cmd(alias = "u")]
    Update {
        #[cmd(attr(this = "true"))]
        current_only: bool,
    },
    AddArchive(String),
    Mark {
        status: EpisodeStatus,
        #[cmd(attr(all = "true"))]
        update_all: bool,
        #[cmd(attr(if))]
        condition: Option<EpisodeSummaryStatus>,
    },
    #[cmd(ignore, alias = "hide", alias = "unhide")]
    SetEpisodeHidden(
        #[cmd(
            alias_value(alias = "hide", value = "true"),
            alias_value(alias = "unhide", value = "false")
        )]
        bool,
    ),
    #[cmd(alias = "s")]
    Search(#[cmd(parser = "hedgehog_library::search::SearchQueryParser")] String),
    SearchAdd,
    OpenLink(LinkType),

    RepeatCommand,
    Refresh,

    Chain(Vec<Command>),
    #[cmd(rename = "if")]
    Conditional {
        predicate: Predicate,
        command: Box<Command>,
        #[cmd(attr(else))]
        otherwise: Option<Box<Command>>,
    },
    #[cmd(rename = "msg")]
    WriteMessage {
        message: String,
        #[cmd(
            default = "Severity::Information",
            attr(
                info = "Severity::Information",
                warn = "Severity::Warning",
                error = "Severity::Error",
            )
        )]
        severity: Severity,
    },
}

#[derive(Debug, Clone, PartialEq, Parsable)]
pub(crate) enum LinkType {
    Feed,
    Episode,
}

#[derive(Debug, Clone, PartialEq, Parsable)]
#[cmd(ctx = "CommandContext<'_>")]
pub(crate) struct CommandConfirmation {
    pub(crate) prompt: String,
    pub(crate) action: Command,
    #[cmd(attr(default))]
    pub(crate) default: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Parsable)]
pub(crate) enum FocusedPane {
    #[cmd(rename = "feeds")]
    FeedsList,
    #[cmd(rename = "episodes")]
    EpisodesList,
    Search,
    #[cmd(rename = "log")]
    ErrorsLog,
}

#[derive(Debug, Clone, Copy, Parsable, PartialEq, Eq)]
pub(crate) enum SelectedItem {
    SpecialFeed,
    Feed,
    Group,
    Episode,
    LogEntry,
    SearchResult,
    Nothing,
}

#[derive(Debug, Clone, Parsable, PartialEq)]
pub(crate) enum Predicate {
    Not(Box<Predicate>),
    Either(Vec<Predicate>),
    Both(Vec<Predicate>),
    Focused(FocusedPane),
    Selected(SelectedItem),
}
