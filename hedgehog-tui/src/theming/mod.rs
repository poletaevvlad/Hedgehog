mod selectors;
mod style_parser;

use crate::cmdreader::CommandReader;
use crate::environment::AppEnvironment;
use selectors::StyleSelector;
pub(crate) use selectors::{
    Empty, EmptyItem, List, ListColumn, ListItem, ListState, Player, PlayerItem, Selector,
    StatusBar,
};
use std::collections::HashMap;
use std::path::PathBuf;
use tui::style::Style;

#[derive(Default)]
pub(crate) struct Theme {
    styles: HashMap<Selector, Style>,
}

impl Theme {
    pub(crate) fn handle_command(&mut self, command: ThemeCommand, env: &AppEnvironment) -> bool {
        match command {
            ThemeCommand::Reset => {
                *self = Theme::default();
            }
            ThemeCommand::Set(selector, style) => {
                self.set(selector, style);
            }
            ThemeCommand::Load(mut path, reset) => {
                path.set_extension("theme");
                let theme_path = env.resolve_config(&path);

                let snapshot = self.styles.clone();
                if reset {
                    *self = Theme::default();
                }

                let mut reader = match CommandReader::open(&theme_path) {
                    Ok(reader) => reader,
                    Err(err) => {
                        log::error!(target: "io", "Cannot open theme {:?}. {}", theme_path, err);
                        self.styles = snapshot;
                        return false;
                    }
                };

                loop {
                    match reader.read(()) {
                        Ok(Some(command)) => {
                            if !self.handle_command(command, env) {
                                self.styles = snapshot;
                                return false;
                            }
                        }
                        Ok(None) => break,
                        Err(error) => {
                            self.styles = snapshot;
                            log::error!(target: "command", "Cannot parse {:?}. {}", theme_path, error);
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    pub(crate) fn get(&self, selector: impl Into<Selector>) -> Style {
        self.styles
            .get(&selector.into())
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn set(&mut self, selector: impl Into<Selector>, style: Style) {
        let styles = &mut self.styles;
        let mut override_style = move |selector: Selector| {
            styles
                .entry(selector)
                .and_modify(|current| *current = current.patch(style))
                .or_insert(style);
        };

        let selector = selector.into();
        override_style(selector);
        selector.for_each_overrides(override_style);
    }
}

#[derive(Debug, Clone, PartialEq, cmdparse::Parsable)]
pub(crate) enum ThemeCommand {
    Reset,
    Set(Selector, #[cmd(parser = "style_parser::StyleParser")] Style),
    Load(
        PathBuf,
        #[cmd(default = "true", attr(extend = "false"))] bool,
    ),
}

#[cfg(test)]
mod tests {
    use super::{List, ListColumn, ListItem, ListState, StatusBar, Theme};
    use crate::logger::Severity;
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
        theme.set(
            StatusBar::Status(None, false),
            Style::default().bg(Color::Red),
        );
        theme.set(
            StatusBar::Status(Some(Severity::Error), false),
            Style::default().fg(Color::White),
        );
        assert_eq!(
            theme.get(StatusBar::Status(Some(Severity::Error), false)),
            Style::default().bg(Color::Red).fg(Color::White)
        );
        assert_eq!(
            theme.get(StatusBar::Status(Some(Severity::Information), false)),
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
            List::Item(ListItem::default()),
            Style::default().fg(Color::White),
        );
        theme.set(
            List::Item(ListItem {
                focused: true,
                ..Default::default()
            }),
            Style::default().bg(Color::Red),
        );
        theme.set(
            List::Item(ListItem {
                state: Some(ListState::EpisodeError),
                ..Default::default()
            }),
            Style::default().add_modifier(Modifier::UNDERLINED),
        );
        theme.set(
            List::Item(ListItem {
                column: Some(ListColumn::Title),
                ..Default::default()
            }),
            Style::default().add_modifier(Modifier::BOLD),
        );

        assert_eq!(
            theme.get(List::Item(ListItem {
                focused: true,
                column: Some(ListColumn::Title),
                ..Default::default()
            })),
            Style::default()
                .fg(Color::White)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD)
        );
        assert_eq!(
            theme.get(List::Item(ListItem {
                state: Some(ListState::EpisodeError),
                column: Some(ListColumn::Title),
                ..Default::default()
            })),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED)
        );
    }
}
