use std::collections::VecDeque;

#[derive(Debug)]
pub(crate) struct CommandsHistory {
    items: VecDeque<String>,
}

impl CommandsHistory {
    const DEFAULT_CAPACITY: usize = 512;

    pub(crate) fn new() -> Self {
        Self::with_capacity(Self::DEFAULT_CAPACITY)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        CommandsHistory {
            items: VecDeque::with_capacity(capacity),
        }
    }

    pub(crate) fn push(&mut self, command: &str) {
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
            if self.items.len() == self.items.capacity() {
                self.items.pop_back();
            }
            command.to_string()
        };
        self.items.push_front(command);
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
    use super::CommandsHistory;

    #[test]
    fn pushing_lines() {
        let mut history = CommandsHistory::default();
        assert_eq!(history.get(0), None);

        history.push("first");
        assert_eq!(history.get(0), Some("first"));
        assert_eq!(history.get(1), None);

        history.push("second");
        assert_eq!(history.get(0), Some("second"));
        assert_eq!(history.get(1), Some("first"));
        assert_eq!(history.get(2), None);

        history.push("first");
        assert_eq!(history.get(0), Some("first"));
        assert_eq!(history.get(1), Some("second"));
        assert_eq!(history.get(2), None);
    }

    fn init_for_find() -> CommandsHistory {
        let mut history = CommandsHistory::default();
        history.push("aa"); // 4
        history.push("abcd"); // 3
        history.push("acd"); // 2
        history.push("abc"); // 1
        history.push("ac"); // 0
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
        history.push("a");
        history.push("b");
        history.push("c");
        assert_eq!(history.iter().collect::<Vec<&str>>(), vec!["c", "b", "a"]);

        history.push("b");
        assert_eq!(history.iter().collect::<Vec<&str>>(), vec!["b", "c", "a"]);

        history.push("d");
        assert_eq!(history.iter().collect::<Vec<&str>>(), vec!["d", "b", "c"]);
    }
}
