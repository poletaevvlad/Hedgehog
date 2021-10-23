use crate::cmdparser;
use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

pub(crate) struct FileResolver {
    suffixes: Vec<&'static str>,
}

impl FileResolver {
    pub(crate) fn new() -> Self {
        FileResolver {
            suffixes: Vec::new(),
        }
    }

    pub(crate) fn with_suffix(mut self, suffix: &'static str) -> Self {
        self.suffixes.push(suffix);
        self
    }

    pub(crate) fn resolve<P: AsRef<Path>>(&self, file_path: P) -> Option<PathBuf> {
        if file_path.as_ref().is_absolute() {
            return Some(file_path.as_ref().into());
        }

        // TODO: Non-UNIX OS paths
        let paths_env =
            env::var("HEDGEHOG_PATH").unwrap_or_else(|_| "/usr/share/hedgehog".to_string());
        let paths = paths_env.split(':');

        for path in paths {
            let mut path: PathBuf = path.to_string().into();
            path.push(file_path.as_ref());
            if path.is_file() {
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
                if path.is_file() {
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
    Parsing(#[source] cmdparser::Error, usize),

    #[error("cannot find file")]
    Resolution,
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
    use super::{CommandReader, Error, FileResolver};
    use std::fs::{remove_file, File};
    use std::io::Write;
    use std::path::{Path, PathBuf};
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

    #[test]
    fn resolving_path_absolute() {
        let resolver = FileResolver::new();
        assert_eq!(
            resolver.resolve("/usr/share/hedgehog/default.theme"),
            Some("/usr/share/hedgehog/default.theme".to_string().into())
        )
    }

    #[test]
    fn resolving_path_relative() {
        let dir1 = tempdir().unwrap();
        let dir2 = tempdir().unwrap();
        let env_path = format!("{}:{}", dir1.path().display(), dir2.path().display());
        std::env::set_var("HEDGEHOG_PATH", env_path);

        let resolver = FileResolver::new()
            .with_suffix(".theme")
            .with_suffix(".style");

        fn push_order_entry(order: &mut Vec<PathBuf>, dir: &Path, filename: &str) {
            let mut path = dir.to_path_buf();
            path.push(filename);
            order.push(path);
        }

        let mut resolution_order = vec![];
        push_order_entry(&mut resolution_order, dir1.path(), "file");
        push_order_entry(&mut resolution_order, dir1.path(), "file.theme");
        push_order_entry(&mut resolution_order, dir1.path(), "file.style");
        push_order_entry(&mut resolution_order, dir2.path(), "file");
        push_order_entry(&mut resolution_order, dir2.path(), "file.theme");
        push_order_entry(&mut resolution_order, dir2.path(), "file.style");

        for path in &resolution_order {
            File::create(path).unwrap();
        }

        while !resolution_order.is_empty() {
            let path = resolver.resolve("file");
            assert_eq!(path.as_ref(), resolution_order.get(0));
            remove_file(resolution_order.remove(0)).unwrap();
        }

        assert!(resolver.resolve("file").is_none());
    }
}