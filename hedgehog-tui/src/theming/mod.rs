mod parser;
mod selectors;
mod style_parser;

use crate::cmdreader::{self, CommandReader, FileResolver};
pub(crate) use selectors::{List, ListItem, Selector, StatusBar};
use std::collections::HashMap;
use std::path::PathBuf;
use tui::style::Style;

#[derive(Debug, Clone, Copy)]
struct OverridableStyle {
    style: Style,
    inherited: bool,
}

impl Default for OverridableStyle {
    fn default() -> Self {
        OverridableStyle {
            style: Style::default(),
            inherited: false,
        }
    }
}

pub(crate) struct Theme {
    status_bar: HashMap<StatusBar, Style>,
    divider: Option<Style>,
    list_items: [OverridableStyle; 8],
}

impl Theme {
    pub(crate) fn handle_command(&mut self, command: ThemeCommand) -> Result<(), cmdreader::Error> {
        match command {
            ThemeCommand::Reset => *self = Theme::default(),
            ThemeCommand::Set(selector, style) => self.set(selector, style),
            ThemeCommand::Load(path, loading_option) => {
                if let ThemeLoadingMode::Reset = loading_option.unwrap_or_default() {
                    *self = Theme::default();
                }
                let resolver = FileResolver::new()
                    .with_suffix(".theme")
                    .with_reversed_order();
                let path = resolver.resolve(path).ok_or(cmdreader::Error::Resolution)?;

                let mut reader = CommandReader::open(path)?;
                while let Some(command) = reader.read()? {
                    self.handle_command(command)?;
                }
            }
        }
        Ok(())
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            status_bar: HashMap::new(),
            divider: None,
            list_items: [OverridableStyle::default(); 8],
        }
    }
}

pub(crate) trait StyleProvider<S> {
    fn set(&mut self, selector: S, style: Style);
    fn get(&self, selector: S) -> Style;
}

impl StyleProvider<StatusBar> for Theme {
    fn set(&mut self, selector: StatusBar, style: Style) {
        self.status_bar.insert(selector, style);
    }

    fn get(&self, selector: StatusBar) -> Style {
        match selector {
            selector @ StatusBar::Status(Some(_)) => self
                .status_bar
                .get(&StatusBar::Status(None))
                .cloned()
                .unwrap_or_default()
                .patch(self.status_bar.get(&selector).cloned().unwrap_or_default()),
            selector => self.status_bar.get(&selector).cloned().unwrap_or_default(),
        }
    }
}

impl StyleProvider<List> for Theme {
    fn set(&mut self, selector: List, style: Style) {
        match selector {
            List::Divider => self.divider = Some(style),
            List::Item(item) => {
                for (current_item, overridable) in self.list_items.iter_mut().enumerate() {
                    let current_item = ListItem::from_bits_truncate(current_item);
                    if current_item & item != item {
                        continue;
                    }
                    overridable.style = overridable.style.patch(style);
                    overridable.inherited = item != current_item;
                }
            }
        }
    }

    fn get(&self, selector: List) -> Style {
        match selector {
            List::Divider => self.divider.unwrap_or_default(),
            List::Item(item) => self.list_items[item.bits()].style,
        }
    }
}

impl StyleProvider<Selector> for Theme {
    fn set(&mut self, selector: Selector, style: Style) {
        match selector {
            Selector::StatusBar(statusbar) => self.set(statusbar, style),
            Selector::List(list) => self.set(list, style),
        }
    }

    fn get(&self, selector: Selector) -> Style {
        match selector {
            Selector::StatusBar(statusbar) => self.get(statusbar),
            Selector::List(list) => self.get(list),
        }
    }
}

#[derive(Debug, serde::Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ThemeLoadingMode {
    Reset,
    NoReset,
}

impl Default for ThemeLoadingMode {
    fn default() -> Self {
        ThemeLoadingMode::Reset
    }
}

#[derive(Debug, serde::Deserialize, Clone, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum ThemeCommand {
    Reset,
    Set(
        Selector,
        #[serde(deserialize_with = "style_parser::deserialize")] Style,
    ),
    Load(PathBuf, Option<ThemeLoadingMode>),
}

#[cfg(test)]
mod tests {
    use crate::status::Severity;

    use super::{List, ListItem, StatusBar, StyleProvider, Theme};
    use tui::style::{Color, Modifier, Style};

    #[test]
    fn status_bar_styles() {
        let mut theme = Theme::default();
        assert_eq!(theme.get(StatusBar::Empty), Style::default());
        theme.set(StatusBar::Empty, Style::default().fg(Color::Red));
        assert_eq!(theme.get(StatusBar::Empty), Style::default().fg(Color::Red));
    }

    #[test]
    fn status_bar_status() {
        let mut theme = Theme::default();
        theme.set(StatusBar::Status(None), Style::default().bg(Color::Red));
        theme.set(
            StatusBar::Status(Some(Severity::Error)),
            Style::default().fg(Color::White),
        );
        assert_eq!(
            theme.get(StatusBar::Status(Some(Severity::Error))),
            Style::default().bg(Color::Red).fg(Color::White)
        );
        assert_eq!(
            theme.get(StatusBar::Status(Some(Severity::Information))),
            Style::default().bg(Color::Red)
        );
    }

    #[test]
    fn divider_styles() {
        let mut theme = Theme::default();
        assert_eq!(theme.get(List::Divider), Style::default());
        theme.set(List::Divider, Style::default().fg(Color::Red));
        assert_eq!(theme.get(List::Divider), Style::default().fg(Color::Red));
    }

    #[test]
    fn list_item_style() {
        let mut theme = Theme::default();
        theme.set(
            List::Item(ListItem::empty()),
            Style::default().fg(Color::White),
        );
        theme.set(
            List::Item(ListItem::SELECTED),
            Style::default().bg(Color::Red),
        );
        theme.set(
            List::Item(ListItem::FOCUSED),
            Style::default().add_modifier(Modifier::BOLD),
        );
        theme.set(
            List::Item(ListItem::ACTIVE),
            Style::default().add_modifier(Modifier::UNDERLINED),
        );

        let selected_focused = theme.get(List::Item(ListItem::SELECTED | ListItem::FOCUSED));
        assert_eq!(
            selected_focused,
            Style {
                fg: Some(Color::White),
                bg: Some(Color::Red),
                add_modifier: Modifier::BOLD,
                sub_modifier: Modifier::empty(),
            }
        );

        let focused_active = theme.get(List::Item(ListItem::FOCUSED | ListItem::ACTIVE));
        assert_eq!(
            focused_active,
            Style {
                fg: Some(Color::White),
                bg: None,
                add_modifier: Modifier::BOLD | Modifier::UNDERLINED,
                sub_modifier: Modifier::empty(),
            }
        );
    }
}
