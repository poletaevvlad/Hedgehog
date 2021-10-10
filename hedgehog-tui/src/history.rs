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

    pub(crate) fn push(&mut self, command: String) {
        if self.items.len() == self.items.capacity() {
            self.items.pop_back();
        }
        self.items.push_front(command)
    }

    pub(crate) fn get(&self, index: usize) -> Option<&str> {
        self.items.get(index).map(String::as_str)
    }

    pub(crate) fn find_before(&self, mut index: usize, prefix: &str) -> Option<(usize, &str)> {
        while let Some(line) = self.get(index) {
            if line.starts_with(prefix) {
                return Some((index, line));
            }
            index += 1;
        }
        None
    }

    pub(crate) fn find_after(&self, mut index: usize, prefix: &str) -> Option<(usize, &str)> {
        while let Some(line) = self.get(index) {
            if line.starts_with(prefix) {
                return Some((index, line));
            }
            if index == 0 {
                break;
            }
            index -= 1;
        }
        None
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

        history.push("first".to_string());
        assert_eq!(history.get(0), Some("first"));
        assert_eq!(history.get(1), None);

        history.push("second".to_string());
        assert_eq!(history.get(0), Some("second"));
        assert_eq!(history.get(1), Some("first"));
        assert_eq!(history.get(2), None);
    }

    fn init_for_find() -> CommandsHistory {
        let mut history = CommandsHistory::default();
        history.push("aa".to_string()); // 4
        history.push("abcd".to_string()); // 3
        history.push("acd".to_string()); // 2
        history.push("abc".to_string()); // 1
        history.push("ac".to_string()); // 0
        history
    }

    #[test]
    fn finding_before() {
        let history = init_for_find();
        assert_eq!(history.find_before(1, "ac"), Some((2, "acd")));
        assert_eq!(history.find_before(1, "aa"), Some((4, "aa")));
        assert_eq!(history.find_before(1, "ae"), None);
    }

    #[test]
    fn finding_after() {
        let history = init_for_find();
        assert_eq!(history.find_after(3, "ac"), Some((2, "acd")));
        assert_eq!(history.find_after(1, "ac"), Some((0, "ac")));
        assert_eq!(history.find_after(2, "aa"), None);
    }
}
