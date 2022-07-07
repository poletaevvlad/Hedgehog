use cmdparse::Parsable;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::collections::HashMap;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Eq, PartialEq, Hash, Clone, Copy)]
pub(crate) struct Key(KeyEvent);

impl From<KeyEvent> for Key {
    fn from(key_event: KeyEvent) -> Self {
        Key(key_event)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum KeyParsingError {
    #[error("keybinding cannot be empty")]
    Empty,

    #[error("key is not recognized")]
    UnknownKey,

    #[error("invalid modifier: {0}")]
    UnknownModifier(String),

    #[error("duplicate modifiers are not allowed")]
    DuplicateModifier,
}

impl FromStr for Key {
    type Err = KeyParsingError;

    // Partially compatible with Vim keybinding notation
    // http://vimdoc.sourceforge.net/htmldoc/intro.html#key-notation
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split_iter = s.rsplit('-');
        let key_code = match split_iter.next().unwrap_or("") {
            "Left" => KeyCode::Left,
            "Right" => KeyCode::Right,
            "Up" => KeyCode::Up,
            "Down" => KeyCode::Down,
            "Enter" | "Return" | "CR" => KeyCode::Enter,
            "BS" | "Backspace" => KeyCode::Backspace,
            "Home" => KeyCode::Home,
            "End" => KeyCode::End,
            "PageUp" => KeyCode::PageUp,
            "PageDown" => KeyCode::PageDown,
            "Tab" => KeyCode::Tab,
            "Del" | "Delete" => KeyCode::Delete,
            "Esc" => KeyCode::Esc,
            "Space" => KeyCode::Char(' '),
            "Bar" => KeyCode::Char('|'),
            "Minus" => KeyCode::Char('-'),
            "Nul" => KeyCode::Null,
            "Insert" => KeyCode::Insert,
            key => {
                let mut characters = key.chars();
                let first = characters.next().ok_or(KeyParsingError::Empty)?;
                let tail = characters.as_str();
                if tail.is_empty() {
                    KeyCode::Char(first)
                } else if first == 'F' {
                    tail.parse()
                        .map(KeyCode::F)
                        .map_err(|_| KeyParsingError::UnknownKey)?
                } else {
                    return Err(KeyParsingError::UnknownKey);
                }
            }
        };

        let mut modifiers = KeyModifiers::NONE;
        for modifier_str in split_iter {
            let modifier = match modifier_str {
                "S" | "Shift" => KeyModifiers::SHIFT,
                "C" | "Ctrl" | "Control" => KeyModifiers::CONTROL,
                "A" | "Alt" | "M" | "Meta" => KeyModifiers::ALT,
                modifier_str => {
                    return Err(KeyParsingError::UnknownModifier(modifier_str.to_string()))
                }
            };
            if modifiers.intersects(modifier) {
                return Err(KeyParsingError::DuplicateModifier);
            }
            modifiers |= modifier;
        }

        Ok(KeyEvent::new(key_code, modifiers).into())
    }
}

impl<Ctx> Parsable<Ctx> for Key {
    type Parser = cmdparse::parsers::FromStrParser<Self>;
}

#[derive(Debug)]
pub(crate) struct KeyMapping<T> {
    mapping: HashMap<Key, T>,
}

impl<T> KeyMapping<T> {
    pub(crate) fn map(&mut self, key: Key, value: T) {
        self.mapping.insert(key, value);
    }

    pub(crate) fn unmap(&mut self, key: Key) -> bool {
        self.mapping.remove(&key).is_some()
    }

    pub(crate) fn contains(&self, key: Key) -> bool {
        self.mapping.contains_key(&key)
    }

    pub(crate) fn get(&self, key: Key) -> Option<&T> {
        self.mapping.get(&key)
    }
}

impl<T> Default for KeyMapping<T> {
    fn default() -> Self {
        KeyMapping {
            mapping: HashMap::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, KeyParsingError};
    use cmdparse::parse;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn parsing_keys() {
        assert_eq!(
            "a".parse::<Key>(),
            Ok(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE).into()),
        );
        assert_eq!(
            "S-Space".parse::<Key>(),
            Ok(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::SHIFT).into()),
        );
        assert_eq!(
            "Alt-Return".parse::<Key>(),
            Ok(KeyEvent::new(KeyCode::Enter, KeyModifiers::ALT).into()),
        );
        assert_eq!(
            "C-M-S-F5".parse::<Key>(),
            Ok(KeyEvent::new(
                KeyCode::F(5),
                KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT
            )
            .into()),
        );

        assert_eq!(
            parse::<_, Key>("S-Space", ()).unwrap(),
            KeyEvent::new(KeyCode::Char(' '), KeyModifiers::SHIFT).into(),
        );

        assert_eq!("S-unknown".parse::<Key>(), Err(KeyParsingError::UnknownKey));
        assert_eq!("F256".parse::<Key>(), Err(KeyParsingError::UnknownKey));
        assert_eq!("".parse::<Key>(), Err(KeyParsingError::Empty));
        assert_eq!(
            "L-a".parse::<Key>(),
            Err(KeyParsingError::UnknownModifier("L".to_string())),
        );
        assert_eq!(
            "S-A-S-a".parse::<Key>(),
            Err(KeyParsingError::DuplicateModifier),
        );
    }
}
