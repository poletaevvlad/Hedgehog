use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("cannot read file: {0}")]
    Io(#[from] io::Error),

    #[error("invalid command at line {1}: {0}")]
    Parsing(String, usize),
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

    pub(crate) fn read<Ctx: Clone, P: cmdparse::Parsable<Ctx>>(
        &mut self,
        ctx: Ctx,
    ) -> Result<Option<P>, Error> {
        loop {
            self.buffer.clear();
            let read_count = self.reader.read_line(&mut self.buffer)?;
            if read_count == 0 {
                return Ok(None);
            }

            self.line_no += 1;
            return match cmdparse::parse::<_, Option<P>>(&self.buffer, ctx.clone()) {
                Ok(Some(command)) => Ok(Some(command)),
                Ok(None) => continue,
                Err(error) => Err(Error::Parsing(error.to_string(), self.line_no)),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandReader, Error};
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[derive(Debug, PartialEq, Eq, cmdparse::Parsable)]
    enum MockCmd {
        First(usize),
        Second(String),
    }

    #[test]
    fn read_file_with_comments() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cmdrc");

        let mut file = File::create(&path).unwrap();
        writeln!(file, "# commend").unwrap();
        writeln!(file, "first 4").unwrap();
        writeln!(file).unwrap();
        writeln!(file, "# commend").unwrap();
        writeln!(file, "second four").unwrap();
        drop(file);

        let mut reader = CommandReader::open(path).unwrap();
        assert_eq!(reader.read(()).unwrap(), Some(MockCmd::First(4)));
        assert_eq!(
            reader.read(()).unwrap(),
            Some(MockCmd::Second("four".to_string()))
        );
        assert_eq!(reader.read::<(), MockCmd>(()).unwrap(), None);
    }

    #[test]
    fn read_file_invalid_command() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("cmdrc");

        let mut file = File::create(&path).unwrap();
        writeln!(file, "first 4").unwrap();
        writeln!(file, "third 4.0").unwrap();
        drop(file);

        let mut reader = CommandReader::open(path).unwrap();
        assert_eq!(reader.read(()).unwrap(), Some(MockCmd::First(4)));
        assert!(matches!(
            reader.read::<(), MockCmd>(()).unwrap_err(),
            Error::Parsing(_, 2)
        ));
    }
}
