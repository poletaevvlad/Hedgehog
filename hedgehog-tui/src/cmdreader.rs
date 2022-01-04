use cmd_parser::CmdParsable;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

pub(crate) struct FileResolver {
    suffixes: Vec<&'static str>,
    reverse_order: bool,
}

impl FileResolver {
    pub(crate) fn new() -> Self {
        FileResolver {
            suffixes: Vec::new(),
            reverse_order: false,
        }
    }

    pub(crate) fn visit_all<P: AsRef<Path>>(
        &self,
        file_path: P,
        paths: &[PathBuf],
        mut visitor: impl FnMut(&Path) -> bool,
    ) -> Option<PathBuf> {
        if file_path.as_ref().is_absolute() {
            return if visitor(file_path.as_ref()) {
                Some(file_path.as_ref().to_path_buf())
            } else {
                None
            };
        }

        let mut paths = Vec::from(paths);
        if self.reverse_order {
            paths.reverse();
        }

        for mut path in paths {
            path.push(file_path.as_ref());
            if path.is_file() && visitor(&path) {
                return Some(path);
            }

            let file_name = match path.file_name() {
                Some(os_str) => os_str.to_os_string(),
                None => continue,
            };

            for suffix in &self.suffixes {
                let mut file_name_with_suffix = file_name.clone();
                file_name_with_suffix.push(suffix);

                path.set_file_name(file_name_with_suffix.clone());
                if path.is_file() && visitor(&path) {
                    return Some(path);
                }
            }
        }
        None
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("file reading error: {0}")]
    Io(#[from] io::Error),

    #[error("invalid command at line {1}: {0}")]
    Parsing(#[source] cmd_parser::ParseError<'static>, usize),
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

    pub(crate) fn read<C: CmdParsable>(&mut self) -> Result<Option<C>, Error> {
        loop {
            self.buffer.clear();
            let read_count = self.reader.read_line(&mut self.buffer)?;
            if read_count == 0 {
                return Ok(None);
            }

            self.line_no += 1;
            return match <Option<C>>::parse_cmd_full(&self.buffer) {
                Ok(Some(command)) => Ok(Some(command)),
                Ok(None) => continue,
                Err(error) => Err(Error::Parsing(error.into_static(), self.line_no)),
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CommandReader, Error};
    use cmd_parser::CmdParsable;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[derive(Debug, PartialEq, Eq, CmdParsable)]
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
        writeln!(file, "first 4").unwrap();
        writeln!(file, "third 4.0").unwrap();
        drop(file);

        let mut reader = CommandReader::open(path).unwrap();
        assert_eq!(reader.read().unwrap(), Some(MockCmd::First(4)));
        assert!(matches!(
            reader.read::<MockCmd>().unwrap_err(),
            Error::Parsing(_, 2)
        ));
    }
}
