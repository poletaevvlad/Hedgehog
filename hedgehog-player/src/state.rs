use crate::State;
use std::time::Duration;

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
        match state {
            None => self.0 = None,
            Some(state) => {
                let current_state = self.0.take();
                let timing = current_state.map(|(_, timing)| timing).unwrap_or_default();
                self.0 = Some((state, timing));
            }
        }
    }

    pub fn set_duration(&mut self, duration: Duration) {
        if let Some((_, ref mut timing)) = self.0 {
            timing.duration = Some(duration)
        }
    }

    pub fn set_position(&mut self, position: Duration) {
        if let Some((_, ref mut timing)) = self.0 {
            timing.position = position
        }
    }
}
