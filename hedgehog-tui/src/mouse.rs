use std::time::{Duration, Instant};
use tui::layout::Rect;

fn rect_contains(rect: &Rect, row: u16, column: u16) -> bool {
    (rect.left()..rect.right()).contains(&column) && (rect.top()..rect.bottom()).contains(&row)
}

#[derive(Default)]
pub(crate) struct WidgetPositions {
    episodes_list: Option<Rect>,
    feeds_list: Option<Rect>,
    search_list: Option<Rect>,
    player_status: Option<Rect>,
    command_entry: Option<Rect>,
}

pub(crate) enum MouseHitResult {
    FeedsRow(usize),
    EpisodesRow(usize),
    SearchRow(usize),
    Player,
    CommandEntry(usize),
}

impl WidgetPositions {
    pub(crate) fn hit_test_at(&self, row: u16, column: u16) -> Option<MouseHitResult> {
        if let Some(feeds_list) = self.feeds_list {
            if rect_contains(&feeds_list, row, column) {
                return Some(MouseHitResult::FeedsRow((row - feeds_list.y) as usize));
            }
        }
        if let Some(episodes_list) = self.episodes_list {
            if rect_contains(&episodes_list, row, column) {
                return Some(MouseHitResult::EpisodesRow(
                    (row - episodes_list.y) as usize,
                ));
            }
        }
        if let Some(search_list) = self.search_list {
            if rect_contains(&search_list, row, column) {
                return Some(MouseHitResult::SearchRow(
                    (row - search_list.y) as usize / 2,
                ));
            }
        }
        if let Some(player_status) = self.player_status {
            if rect_contains(&player_status, row, column) {
                return Some(MouseHitResult::Player);
            }
        }
        if let Some(command_entry) = self.command_entry {
            if rect_contains(&command_entry, row, column) {
                return Some(MouseHitResult::CommandEntry(
                    (column - command_entry.x) as usize,
                ));
            }
        }
        None
    }

    pub(crate) fn set_episodes_list(&mut self, rect: Rect) {
        self.episodes_list = Some(rect);
    }

    pub(crate) fn set_feeds_list(&mut self, rect: Rect) {
        self.feeds_list = Some(rect);
    }

    pub(crate) fn set_search_list(&mut self, rect: Rect) {
        self.search_list = Some(rect);
    }

    pub(crate) fn set_player_status(&mut self, rect: Rect) {
        self.player_status = Some(rect);
    }

    pub(crate) fn set_command_entry(&mut self, rect: Rect) {
        self.command_entry = Some(rect);
    }
}

#[derive(Debug, Default)]
pub(crate) struct MouseState {
    dragging: bool,
    started: Option<(u16, u16)>,
    previous: Option<(u16, u16, Instant)>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MouseEventKind {
    ScrollUp,
    ScrollDown,
    Click(bool),
}

pub(crate) struct MouseEvent {
    pub(crate) kind: MouseEventKind,
    pub(crate) row: u16,
    pub(crate) column: u16,
}

impl MouseEvent {
    fn new(kind: MouseEventKind, row: u16, column: u16) -> Self {
        MouseEvent { kind, row, column }
    }
}

impl MouseState {
    pub(crate) fn handle_event(
        &mut self,
        event: crossterm::event::MouseEvent,
    ) -> Option<MouseEvent> {
        match event.kind {
            crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                self.dragging = false;
                self.started = Some((event.row, event.column));
                None
            }
            crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left) => {
                let previous = std::mem::replace(
                    &mut self.previous,
                    Some((event.row, event.column, Instant::now())),
                );
                if let Some((start_row, start_column)) = self.started {
                    if !self.dragging && start_row == event.row && start_column == event.column {
                        let is_double_click = match previous {
                            Some((prev_row, prev_column, prev_instant)) => {
                                prev_row == event.row
                                    && prev_column == event.column
                                    && (Instant::now() - prev_instant) < Duration::from_millis(500)
                            }
                            None => false,
                        };
                        return Some(MouseEvent::new(
                            MouseEventKind::Click(is_double_click),
                            event.row,
                            event.column,
                        ));
                    }
                }
                None
            }
            crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left) => {
                self.dragging = true;
                None
            }
            crossterm::event::MouseEventKind::ScrollDown => {
                self.dragging = false;
                Some(MouseEvent::new(
                    MouseEventKind::ScrollDown,
                    event.row,
                    event.column,
                ))
            }
            crossterm::event::MouseEventKind::ScrollUp => {
                self.dragging = false;
                Some(MouseEvent::new(
                    MouseEventKind::ScrollUp,
                    event.row,
                    event.column,
                ))
            }
            _ => None,
        }
    }
}
