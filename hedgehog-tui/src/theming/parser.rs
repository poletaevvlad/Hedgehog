#[derive(Clone)]
pub(crate) struct ParsableStr<'a>(&'a str);

impl<'a> ParsableStr<'a> {
    pub(crate) fn new(string: &'a str) -> Self {
        ParsableStr(string)
    }

    pub(crate) fn as_str(&self) -> &'a str {
        self.0
    }

    pub(crate) fn take_while(&mut self, predicate: impl Fn(&char) -> bool) -> &'a str {
        let input = self.0;
        loop {
            let mut chars = self.0.chars();
            match chars.next() {
                Some(ch) if predicate(&ch) => self.0 = chars.as_str(),
                _ => break,
            }
        }

        let taken_length = input.len() - self.0.len();
        &input[..taken_length]
    }

    pub(crate) fn take_token(&mut self, token: &str) -> bool {
        if self.0.starts_with(token) {
            self.0 = &self.0[token.len()..];
            true
        } else {
            false
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn take(&mut self) -> Option<char> {
        let mut chars = self.0.chars();
        if let Some(ch) = chars.next() {
            self.0 = chars.as_str();
            Some(ch)
        } else {
            None
        }
    }
}

macro_rules! match_take {
    ($input:ident, $($token:literal => $value:expr),*, _ => $default:expr $(,)?) => {
        $(if $input.take_token($token) { $value } else)*
        { $default }
    }
}

pub(crate) use match_take;
