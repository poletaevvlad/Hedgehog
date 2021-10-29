use std::time::Duration;

pub enum PlaybackStatus {
    None,
    Buffering,
    Playing,
    Paused,
}

#[derive(Debug, Default)]
pub struct PlaybackTiming {
    pub duration: Option<Duration>,
    pub position: Duration,
}

pub enum PlaybackState {
    None,
    Active {
        timing: PlaybackTiming,
        is_buffering: bool,
        is_paused: bool,
    },
}

impl PlaybackState {
    pub fn status(&self) -> PlaybackStatus {
        match self {
            PlaybackState::None => PlaybackStatus::None,
            PlaybackState::Active {
                is_paused: true, ..
            } => PlaybackStatus::Paused,
            PlaybackState::Active {
                is_buffering: true, ..
            } => PlaybackStatus::Buffering,
            PlaybackState::Active { .. } => PlaybackStatus::Playing,
        }
    }

    pub fn timing(&self) -> Option<&PlaybackTiming> {
        match self {
            PlaybackState::None => None,
            PlaybackState::Active { timing, .. } => Some(timing),
        }
    }
}

#[derive(Debug, Clone)]
pub enum StateUpdate {
    Initiated,
    BufferingChanged(bool),
    PausedChanged(bool),
    Stopped,
    DurationSet(Duration),
    PositionChanged(Duration),
}

impl PlaybackState {
    fn update(&mut self, update: StateUpdate) -> bool {
        match self {
            this @ PlaybackState::None => {
                if let StateUpdate::Initiated = update {
                    *this = PlaybackState::Active {
                        timing: PlaybackTiming::default(),
                        is_buffering: true,
                        is_paused: false,
                    };
                    true
                } else {
                    false
                }
            }
            this if matches!(update, StateUpdate::Stopped) => {
                *this = PlaybackState::None;
                true
            }
            PlaybackState::Active {
                timing,
                is_buffering,
                is_paused,
            } => match update {
                StateUpdate::BufferingChanged(buffering) => {
                    *is_buffering = buffering;
                    true
                }
                StateUpdate::PausedChanged(paused) => {
                    *is_paused = paused;
                    true
                }
                StateUpdate::DurationSet(duration) => {
                    timing.duration = Some(duration);
                    true
                }
                StateUpdate::PositionChanged(new_position) => {
                    timing.position = new_position;
                    true
                }
                _ => false,
            },
        }
    }
}
