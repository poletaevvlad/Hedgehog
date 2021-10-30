use std::time::Duration;

use crate::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaybackStatus {
    None,
    Buffering,
    Playing,
    Paused,
}

impl PlaybackStatus {
    pub fn enumerate() -> impl IntoIterator<Item = Self> {
        [
            PlaybackStatus::None,
            PlaybackStatus::Buffering,
            PlaybackStatus::Playing,
            PlaybackStatus::Paused,
        ]
    }
}

#[derive(Debug, Default, Copy, Clone)]
pub struct PlaybackTiming {
    pub duration: Option<Duration>,
    pub position: Duration,
}

#[derive(Debug, Default)]
pub struct PlaybackState(Option<(State, PlaybackTiming)>);

impl PlaybackState {
    pub fn status(&self) -> PlaybackStatus {
        match self.0 {
            Some((state, _)) if !state.is_started || state.is_buffering => {
                PlaybackStatus::Buffering
            }
            Some((state, _)) if state.is_paused => PlaybackStatus::Paused,
            Some(_) => PlaybackStatus::Playing,
            None => PlaybackStatus::None,
        }
    }

    pub fn timing(&self) -> Option<PlaybackTiming> {
        self.0.map(|(_, timing)| timing)
    }

    pub fn set_state(&mut self, state: Option<State>) {
        match (self.0, state) {
            (None, None) => (),
            (None, Some(state)) => self.0 = Some((state, PlaybackTiming::default())),
            (Some(_), None) => self.0 = None,
            (Some((ref mut current_state, _)), Some(state)) => *current_state = state,
        }
    }

    pub fn set_duration(&mut self, duration: Duration) {
        if let Some((_, mut timing)) = self.0 {
            timing.duration = Some(duration)
        }
    }

    pub fn set_position(&mut self, position: Duration) {
        if let Some((_, mut timing)) = self.0 {
            timing.position = position
        }
    }
}
