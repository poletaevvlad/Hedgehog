use crate::scrolling::DataView;
use actix::{Message, Recipient};
use chrono::{DateTime, Local};
use std::cmp::{Ord, Ordering, PartialOrd};
use std::time::Duration;

pub(crate) const TTL_LONG: Duration = Duration::from_secs(10);
pub(crate) const TTL_MEDIUM: Duration = Duration::from_secs(5);
pub(crate) const TTL_SHORT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) enum Severity {
    Error,
    Warning,
    Information,
}

impl Ord for Severity {
    fn cmp(&self, other: &Severity) -> Ordering {
        match (self, other) {
            (a, b) if a == b => Ordering::Equal,
            (Severity::Error, _) | (_, Severity::Information) => Ordering::Greater,
            (Severity::Information, _) | (_, Severity::Error) => Ordering::Less,
            _ => unreachable!(),
        }
    }
}

impl PartialOrd<Severity> for Severity {
    fn partial_cmp(&self, other: &Severity) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Severity {
    pub(crate) fn enumerate() -> impl IntoIterator<Item = Self> {
        [Severity::Error, Severity::Warning, Severity::Information]
    }

    fn from_log_level(level: log::Level) -> Option<Self> {
        match level {
            log::Level::Error => Some(Severity::Error),
            log::Level::Warn => Some(Severity::Warning),
            log::Level::Info => Some(Severity::Information),
            _ => None,
        }
    }
}

#[derive(PartialEq, Eq)]
enum LogTarget {
    Default,
    CommandsHistory,
    Command,
    KeyMapping,
    DBus,
    Actix,
    Volume,
    Player,
    Playback,
    Sql,
    Io,
    Networking,
    Browser,
    InternalLogControlMessage,
}

impl LogTarget {
    fn from_str(name: &str) -> Self {
        match name {
            "commands_history" => LogTarget::CommandsHistory,
            "command" => LogTarget::Command,
            "key_mapping" => LogTarget::KeyMapping,
            "dbus" => LogTarget::DBus,
            "actix" => LogTarget::Actix,
            "volume" => LogTarget::Volume,
            "player" => LogTarget::Player,
            "playback" => LogTarget::Playback,
            "sql" => LogTarget::Sql,
            "io" => LogTarget::Io,
            "networking" => LogTarget::Networking,
            "browser" => LogTarget::Browser,
            "__logger_ctl" => LogTarget::InternalLogControlMessage,
            _ => LogTarget::Default,
        }
    }
}

#[derive(Message)]
#[rtype(return = "()")]
pub(crate) struct LogEntry {
    severity: Severity,
    target: LogTarget,
    message: String,
    timestamp: DateTime<Local>,
}

impl LogEntry {
    fn store_in_history(&self) -> bool {
        match (&self.severity, &self.target) {
            (Severity::Information, _) => false,
            (_, LogTarget::Command) => false,
            (_, _) => true,
        }
    }

    pub(crate) fn display_ttl(&self) -> Option<Duration> {
        match self.target {
            LogTarget::Playback => Some(TTL_LONG),
            LogTarget::Browser => Some(TTL_MEDIUM),
            LogTarget::KeyMapping | LogTarget::Volume => Some(TTL_SHORT),
            _ => None,
        }
    }

    pub(crate) fn severity(&self) -> Severity {
        self.severity
    }

    pub(crate) fn variant_label(&self) -> Option<&'static str> {
        match self.target {
            LogTarget::Default => None,
            LogTarget::CommandsHistory => Some("Command history error"),
            LogTarget::Command => Some("Invalid command"),
            LogTarget::KeyMapping => None,
            LogTarget::DBus => Some("MPRIS/DBus error"),
            LogTarget::Actix => Some("Internal error"),
            LogTarget::Volume => None,
            LogTarget::Player => Some("Internal audio player error"),
            LogTarget::Playback => Some("Playback error:"),
            LogTarget::Sql => Some("Internal database error"),
            LogTarget::Io => Some("I/O error"),
            LogTarget::Networking => Some("Network error"),
            LogTarget::Browser => None,
            LogTarget::InternalLogControlMessage => unreachable!(),
        }
    }

    pub(crate) fn message(&self) -> &str {
        &self.message
    }

    pub(crate) fn timestamp(&self) -> DateTime<Local> {
        self.timestamp
    }
}

pub(crate) struct ActorLogger {
    recipient: Recipient<LogEntry>,
}

impl ActorLogger {
    pub(crate) fn new(recipient: Recipient<LogEntry>) -> Self {
        ActorLogger { recipient }
    }
}

impl log::Log for ActorLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record) {
        if let Some(severity) = Severity::from_log_level(record.level()) {
            let message = LogEntry {
                severity,
                target: LogTarget::from_str(record.target()),
                message: format!("{}", record.args()),
                timestamp: Local::now(),
            };
            let _ = self.recipient.do_send(message);
        }
    }

    fn flush(&self) {}
}

enum LogDisplay {
    Last,
    Special(LogEntry),
}

pub(crate) struct LogHistory {
    log: Vec<LogEntry>,
    display: Option<LogDisplay>,
    level: Severity,
}

impl Default for LogHistory {
    fn default() -> Self {
        LogHistory {
            log: Vec::new(),
            display: None,
            level: Severity::Information,
        }
    }
}

impl LogHistory {
    pub(crate) fn set_level(&mut self, level: Severity) {
        self.level = level;
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.log.is_empty()
    }

    pub(crate) fn push(&mut self, entry: LogEntry) {
        if entry.target == LogTarget::InternalLogControlMessage {
            match entry.message.as_str() {
                "set_level:info" => self.set_level(Severity::Information),
                "set_level:warning" => self.set_level(Severity::Warning),
                "set_level:error" => self.set_level(Severity::Error),
                _ => {}
            }
            return;
        }

        if entry.severity < self.level {
            return;
        }
        self.display = if entry.store_in_history() {
            self.log.push(entry);
            Some(LogDisplay::Last)
        } else {
            Some(LogDisplay::Special(entry))
        }
    }

    pub(crate) fn display_entry(&self) -> Option<&LogEntry> {
        match self.display {
            Some(LogDisplay::Last) => self.log.last(),
            Some(LogDisplay::Special(ref status)) => Some(status),
            None => None,
        }
    }

    pub(crate) fn clear_display(&mut self) {
        self.display = None;
    }

    pub(crate) fn clear_playback_display_error(&mut self) {
        if let Some(entry) = self.display_entry() {
            if entry.target == LogTarget::Playback {
                self.display = None;
            }
        }
    }
}

impl DataView for LogHistory {
    type Item = LogEntry;

    fn size(&self) -> usize {
        self.log.len()
    }

    fn item_at(&self, index: usize) -> Option<&Self::Item> {
        self.log.get(self.log.size().saturating_sub(index + 1))
    }

    fn find(&self, p: impl Fn(&Self::Item) -> bool) -> Option<usize> {
        self.log
            .iter()
            .enumerate()
            .find(|(_, item)| p(item))
            .map(|(index, _)| index)
    }
}

macro_rules! log_set_level {
    (Information) => {
        log::error!(target: "__logger_ctl", "set_level:info")
    };
    (Warning) => {
        log::error!(target: "__logger_ctl", "set_level:warning")
    };
    (Error) => {
        log::error!(target: "__logger_ctl", "set_level:error")
    };
}

pub(crate) use log_set_level;
