use cmd_parser::CmdParsable;
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

    #[error("'{0}' is not a recognized key")]
    UnknownKey(String),

    #[error("'{0}' is not a recognized modifier")]
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
                        .map_err(|_| KeyParsingError::UnknownKey(key.to_string()))?
                } else {
                    return Err(KeyParsingError::UnknownKey(key.to_string()));
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

impl CmdParsable for Key {
    fn parse_cmd_raw(input: &str) -> Result<(Self, &str), cmd_parser::ParseError<'_>> {
        cmd_parser::parse_cmd_token(input, "key")
    }
}

struct KeyDeserializerVisitor;

impl<'de> serde::de::Visitor<'de> for KeyDeserializerVisitor {
    type Value = Key;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("key")
    }

    fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
        v.parse().map_err(E::custom)
    }
}

impl<'de> serde::Deserialize<'de> for Key {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(KeyDeserializerVisitor)
    }
}

#[derive(Debug)]
pub(crate) struct KeyMapping<T, S> {
    mapping: HashMap<(Key, Option<S>), T>,
}

impl<T, S: Eq + std::hash::Hash> KeyMapping<T, S> {
    pub(crate) fn new() -> Self {
        KeyMapping::default()
    }

    pub(crate) fn map(&mut self, key: Key, state: Option<S>, value: T) {
        self.mapping.insert((key, state), value);
    }

    pub(crate) fn unmap(&mut self, key: Key, state: Option<S>) -> bool {
        self.mapping.remove(&(key, state)).is_some()
    }

    pub(crate) fn contains(&self, key: Key, state: Option<S>) -> bool {
        self.mapping.contains_key(&(key, state))
    }

    pub(crate) fn get(&self, key: Key, state: Option<S>) -> Option<&T> {
        if let Some(state) = state {
            self.mapping
                .get(&(key, Some(state)))
                .or_else(|| self.mapping.get(&(key, None)))
        } else {
            self.mapping.get(&(key, None))
        }
    }
}

impl<T, S> Default for KeyMapping<T, S> {
    fn default() -> Self {
        KeyMapping {
            mapping: HashMap::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Key, KeyParsingError};
    use crate::cmdparser;
    use cmd_parser::CmdParsable;
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
            Key::parse_cmd("S-Space 10").unwrap(),
            (
                KeyEvent::new(KeyCode::Char(' '), KeyModifiers::SHIFT).into(),
                "10"
            )
        );

        assert_eq!(
            "S-unknown".parse::<Key>(),
            Err(KeyParsingError::UnknownKey("unknown".to_string())),
        );
        assert_eq!(
            "F256".parse::<Key>(),
            Err(KeyParsingError::UnknownKey("F256".to_string())),
        );
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

    #[test]
    fn deserialize_key() {
        assert_eq!(
            cmdparser::from_str::<Key>("C-a").unwrap(),
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL).into()
        );
        assert_eq!(
            &cmdparser::from_str::<Key>("unknown")
                .unwrap_err()
                .to_string(),
            "'unknown' is not a recognized key"
        );
    }
}
