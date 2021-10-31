use crate::State;
use std::fmt;
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

fn format_duration(f: &mut fmt::Formatter<'_>, duration: Duration, precision: u32) -> fmt::Result {
    let total_seconds = duration.as_secs();
    let seconds = total_seconds % 60;
    let minutes = total_seconds / 60 % 60;
    let hours = total_seconds / 3600;

    match precision {
        0 => f.write_fmt(format_args!("{}:{:0>2}", minutes, seconds)),
        1 => f.write_fmt(format_args!("{:0>2}:{:0>2}", minutes, seconds)),
        _ => f.write_fmt(format_args!("{}:{:0>2}:{:0>2}", hours, minutes, seconds)),
    }
}

fn get_duration_precision(duration: Duration) -> u32 {
    let seconds = duration.as_secs();
    if seconds < 600 {
        0
    } else if seconds < 3600 {
        1
    } else {
        2
    }
}

impl fmt::Display for PlaybackTiming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(duration) = self.duration {
            let precision = get_duration_precision(duration);
            format_duration(f, self.position, precision)?;
            f.write_str(" / ")?;
            format_duration(f, duration, precision)
        } else {
            let precision = get_duration_precision(self.position);
            format_duration(f, self.position, precision)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::PlaybackTiming;
    use std::time::Duration;

    #[test]
    fn formatting_timing_without_duration() {
        fn make_timing(seconds: u64) -> PlaybackTiming {
            PlaybackTiming {
                position: Duration::from_secs(seconds),
                duration: None,
            }
        }

        assert_eq!(format!("{}", make_timing(92)), "1:32");
        assert_eq!(format!("{}", make_timing(3599)), "59:59");
        assert_eq!(format!("{}", make_timing(3600)), "1:00:00");
        assert_eq!(format!("{}", make_timing(9492)), "2:38:12");
    }

    #[test]
    fn formatting_timing() {
        fn make_timing(position_seconds: u64, duration_seconds: u64) -> PlaybackTiming {
            PlaybackTiming {
                position: Duration::from_secs(position_seconds),
                duration: Some(Duration::from_secs(duration_seconds)),
            }
        }

        assert_eq!(format!("{}", make_timing(40, 92)), "0:40 / 1:32");
        assert_eq!(format!("{}", make_timing(40, 3599)), "00:40 / 59:59");
        assert_eq!(format!("{}", make_timing(40, 3600)), "0:00:40 / 1:00:00");
        assert_eq!(format!("{}", make_timing(40, 9492)), "0:00:40 / 2:38:12");
    }
}
