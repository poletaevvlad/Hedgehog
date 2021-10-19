use super::cmdparser;
use std::borrow::Cow;
use std::fmt;

#[derive(Debug)]
pub(crate) enum Status {
    CommandParsingError(cmdparser::Error),
    Custom(Cow<'static, str>, Severity),
}

impl Status {
    pub(crate) fn severity(&self) -> Severity {
        match self {
            Status::CommandParsingError(_) => Severity::Error,
            Status::Custom(_, severity) => *severity,
        }
    }

    pub(crate) fn new_custom(text: impl Into<Cow<'static, str>>, severity: Severity) -> Self {
        Status::Custom(text.into(), severity)
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::CommandParsingError(error) => {
                f.write_fmt(format_args!("Invalid command: {}", error))
            }
            Status::Custom(error, _) => f.write_str(&error),
        }
    }
}

impl From<cmdparser::Error> for Status {
    fn from(error: cmdparser::Error) -> Self {
        Status::CommandParsingError(error)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Severity {
    Error,
    Warning,
    Information,
}
