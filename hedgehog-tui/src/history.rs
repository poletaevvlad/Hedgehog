use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader};
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) struct CommandsHistory {
    items: VecDeque<String>,
    capacity: usize,
    file_path: Option<PathBuf>,
}

impl CommandsHistory {
    const DEFAULT_CAPACITY: usize = 512;

    pub(crate) fn load_file(&mut self, path: PathBuf) -> io::Result<()> {
        let file = OpenOptions::new().read(true).write(true).open(&path);

        let mut file = match file {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                self.file_path = Some(path);
                return Ok(());
            }
            Err(err) => return Err(err),
        };

        let mut lines_count = 0;
        for line in BufReader::new(&file).lines() {
            let line = line?;
            self.push(&line)?;
            lines_count += 1;
        }

        if lines_count > Self::DEFAULT_CAPACITY {
            file.seek(SeekFrom::Start(0))?;
            file.set_len(0)?;
            for line in &self.items {
                writeln!(&mut file, "{}", line)?;
            }
        }

        self.file_path = Some(path);
        Ok(())
    }

    pub(crate) fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        CommandsHistory {
            capacity,
            items: VecDeque::with_capacity(capacity),
            file_path: None,
        }
    }

    pub(crate) fn push(&mut self, command: &str) -> io::Result<()> {
        let mut index = None;
        for i in 0..self.items.len() {
            if self.items[i] == command {
                index = Some(i);
                break;
            }
        }
        let command = if let Some(index) = index {
            self.items.remove(index).unwrap()
        } else {
            if self.items.len() == self.capacity {
                self.items.pop_back();
            }
            command.to_string()
        };
        self.items.push_front(command);

        if let Some(path) = self.file_path.as_ref() {
            let result = OpenOptions::new()
                .append(true)
                .create(true)
                .open(path)
                .and_then(|mut file| writeln!(&mut file, "{}", self.get(0).unwrap()));
            if let Err(error) = result {
                self.file_path = None;
                return Err(error);
            }
        }
        Ok(())
    }

    pub(crate) fn get(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(String::as_str)
    }

    pub(crate) fn find_before(&self, mut index: usize, prefix: &str) -> Option<usize> {
        while let Some(line) = self.get(index) {
            if line.starts_with(prefix) {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    pub(crate) fn find_after(&self, mut index: usize, prefix: &str) -> Option<usize> {
        while let Some(line) = self.get(index) {
            if line.starts_with(prefix) {
                return Some(index);
            }
            if index == 0 {
                break;
            }
            index -= 1;
        }
        None
    }

    #[cfg(test)]
    fn iter(&self) -> impl Iterator<Item = &str> {
        self.items.iter().map(String::as_str)
    }
}

impl Default for CommandsHistory {
    fn default() -> Self {
        CommandsHistory::new()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{BufRead, BufReader},
    };

    use super::CommandsHistory;

    #[test]
    fn pushing_lines() {
        let mut history = CommandsHistory::default();
        assert_eq!(history.get(0), None);

        history.push("first").unwrap();
        assert_eq!(history.get(0), Some("first"));
        assert_eq!(history.get(1), None);

        history.push("second").unwrap();
        assert_eq!(history.get(0), Some("second"));
        assert_eq!(history.get(1), Some("first"));
        assert_eq!(history.get(2), None);

        history.push("first").unwrap();
        assert_eq!(history.get(0), Some("first"));
        assert_eq!(history.get(1), Some("second"));
        assert_eq!(history.get(2), None);
    }

    fn init_for_find() -> CommandsHistory {
        let mut history = CommandsHistory::default();
        history.push("aa").unwrap(); // 4
        history.push("abcd").unwrap(); // 3
        history.push("acd").unwrap(); // 2
        history.push("abc").unwrap(); // 1
        history.push("ac").unwrap(); // 0
        history
    }

    #[test]
    fn finding_before() {
        let history = init_for_find();
        assert_eq!(history.find_before(1, "ac"), Some(2));
        assert_eq!(history.find_before(1, "aa"), Some(4));
        assert_eq!(history.find_before(1, "ae"), None);
    }

    #[test]
    fn finding_after() {
        let history = init_for_find();
        assert_eq!(history.find_after(3, "ac"), Some(2));
        assert_eq!(history.find_after(1, "ac"), Some(0));
        assert_eq!(history.find_after(2, "aa"), None);
    }

    #[test]
    fn removes_old_entries() {
        let mut history = CommandsHistory::with_capacity(3);
        history.push("a").unwrap();
        history.push("b").unwrap();
        history.push("c").unwrap();
        assert_eq!(history.iter().collect::<Vec<&str>>(), vec!["c", "b", "a"]);

        history.push("b").unwrap();
        assert_eq!(history.iter().collect::<Vec<&str>>(), vec!["b", "c", "a"]);

        history.push("d").unwrap();
        assert_eq!(history.iter().collect::<Vec<&str>>(), vec!["d", "b", "c"]);
    }

    #[test]
    fn reading_history_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut path = dir.path().to_path_buf();
        path.push("commands");

        let mut history = CommandsHistory::new();
        history.load_file(path.clone()).unwrap();
        assert!(history.iter().next().is_none());
        history.push("a").unwrap();
        history.push("b").unwrap();
        history.push("c").unwrap();

        let mut history = CommandsHistory::new();
        history.load_file(path.clone()).unwrap();
        assert_eq!(history.iter().collect::<Vec<&str>>(), vec!["c", "b", "a"]);
        history.push("b").unwrap();
        history.push("d").unwrap();
        history.push("a").unwrap();

        let mut history = CommandsHistory::new();
        history.load_file(path.clone()).unwrap();
        assert_eq!(
            history.iter().collect::<Vec<&str>>(),
            vec!["a", "d", "b", "c"]
        );
    }

    #[test]
    fn trancates_file_on_open() {
        let dir = tempfile::tempdir().unwrap();
        let mut path = dir.path().to_path_buf();
        path.push("commands");

        let mut history = CommandsHistory::new();
        history.load_file(path.clone()).unwrap();
        for i in 0..1000 {
            history.push(&format!("{}", i)).unwrap();
        }
        assert_eq!(
            BufReader::new(File::open(&path).unwrap()).lines().count(),
            1000
        );

        CommandsHistory::new().load_file(path.clone()).unwrap();
        assert_eq!(
            BufReader::new(File::open(&path).unwrap()).lines().count(),
            512
        );
    }
}
