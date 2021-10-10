use super::cmdparser;
use std::fmt;

pub(crate) enum Status {
    CommandParsingError(cmdparser::Error),
}

impl Status {
    pub(crate) fn severity(&self) -> Severity {
        match self {
            Status::CommandParsingError(_) => Severity::Error,
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::CommandParsingError(error) => {
                f.write_fmt(format_args!("Invalid command: {}", error))
            }
        }
    }
}

impl From<cmdparser::Error> for Status {
    fn from(error: cmdparser::Error) -> Self {
        Status::CommandParsingError(error)
    }
}

pub(crate) enum Severity {
    Error,
    Warning,
    Information,
}
