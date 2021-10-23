use crate::cmdparser;
use serde::Deserialize;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("file reading error: {0}")]
    Io(#[from] io::Error),

    #[error("invalid command at line {1}: {0}")]
    Parsing(#[source] cmdparser::Error, usize),
}

#[derive(Debug)]
pub(crate) struct CommandReader {
    reader: BufReader<File>,
    line_no: usize,
    buffer: String,
}

impl CommandReader {
    pub(crate) fn open<P: AsRef<Path>>(path: P) -> io::Result<CommandReader> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(CommandReader {
            reader,
            line_no: 0,
            buffer: String::new(),
        })
    }

    pub(crate) fn read<'de, C: Deserialize<'de>>(&'de mut self) -> Result<Option<C>, Error> {
        self.buffer.clear();
        let read_count = self.reader.read_line(&mut self.buffer)?;
        if read_count == 0 {
            return Ok(None);
        }

        self.line_no += 1;
        match cmdparser::from_str(&self.buffer) {
            Ok(command) => Ok(Some(command)),
            Err(error) => Err(Error::Parsing(error, self.line_no)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandReader, Error};
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[derive(Debug, serde::Deserialize, PartialEq, Eq)]
    enum MockCmd {
        First(usize),
        Second(String),
    }

    #[test]
    fn read_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cmdrc");

        let mut file = File::create(&path).unwrap();
        writeln!(file, "First 4").unwrap();
        writeln!(file, "Second four").unwrap();
        drop(file);

        let mut reader = CommandReader::open(path).unwrap();
        assert_eq!(reader.read().unwrap(), Some(MockCmd::First(4)));
        assert_eq!(
            reader.read().unwrap(),
            Some(MockCmd::Second("four".to_string()))
        );
        assert_eq!(reader.read::<MockCmd>().unwrap(), None);
    }

    #[test]
    fn read_file_invalid_command() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cmdrc");

        let mut file = File::create(&path).unwrap();
        writeln!(file, "First 4").unwrap();
        writeln!(file, "Third 4.0").unwrap();
        drop(file);

        let mut reader = CommandReader::open(path).unwrap();
        assert_eq!(reader.read().unwrap(), Some(MockCmd::First(4)));
        assert!(matches!(
            reader.read::<MockCmd>().unwrap_err(),
            Error::Parsing(_, 2)
        ));
    }
}
