use chrono::{TimeZone, Utc};
use hedgehog_player::state::PlaybackTiming;
use std::{fmt, time::Duration};
use unicode_width::UnicodeWidthStr;

pub(super) fn number_width(number: i64) -> u16 {
    fn width_positive(number: i64) -> u16 {
        macro_rules! impl_width{
            ($($i:literal)*; $remaining:literal) => {
                $( if number < 10_i64.pow($i) { $i } else)*
                { $remaining }
            }
        }
        impl_width!(1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18; 19)
    }

    if number >= 0 {
        width_positive(number)
    } else {
        width_positive(-number) + 1
    }
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

pub(super) struct DurationFormatter(pub(super) Duration);

impl DurationFormatter {
    pub(super) fn width(&self) -> u16 {
        let precision = get_duration_precision(self.0);
        match precision {
            0 => 4,
            1 => 5,
            2 => 6 + number_width((self.0.as_secs() / 3600) as i64),
            _ => unreachable!(),
        }
    }
}

impl fmt::Display for DurationFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let precision = get_duration_precision(self.0);
        format_duration(f, self.0, precision)
    }
}

pub(super) struct PlaybackTimingFormatter(pub(super) PlaybackTiming);

impl fmt::Display for PlaybackTimingFormatter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(duration) = self.0.duration {
            let precision = get_duration_precision(duration);
            format_duration(f, self.0.position, precision)?;
            f.write_str(" / ")?;
            format_duration(f, duration, precision)
        } else {
            let precision = get_duration_precision(self.0.position);
            format_duration(f, self.0.position, precision)
        }
    }
}

pub(super) fn date_width(format: &str) -> u16 {
    // wednesday, september (the longest day of week and month in English)
    let width1 = Utc
        .ymd(2021, 9, 22)
        .and_hms_milli(11, 30, 40, 1234)
        .format(format)
        .to_string()
        .width();
    let width2 = Utc
        .ymd(2021, 12, 22)
        .and_hms_milli(11, 30, 40, 1234)
        .format(format)
        .to_string()
        .width();
    width1.max(width2) as u16
}

#[cfg(test)]
mod tests {
    use super::{number_width, DurationFormatter, PlaybackTimingFormatter};
    use hedgehog_player::state::PlaybackTiming;
    use std::time::Duration;

    #[test]
    fn number_width_test() {
        fn test_number(number: i64) {
            let str_repr = number.to_string();
            let width = number_width(number);
            assert_eq!(width, str_repr.len() as u16, "{}", str_repr);
        }

        let powers_of_10 = [
            0,
            10,
            100,
            1000,
            10000,
            100000,
            1000000,
            10000000,
            100000000,
            1000000000,
            10000000000,
            100000000000,
            1000000000000,
            10000000000000,
            100000000000000,
            1000000000000000,
            10000000000000000,
            100000000000000000,
            1000000000000000000,
        ];

        for num in &powers_of_10 {
            test_number(*num);
            test_number(*num - 1);
            test_number(-*num);
            test_number(-(*num - 1));
        }
    }

    #[test]
    fn formatting_timing_without_duration() {
        fn make_timing(seconds: u64) -> PlaybackTimingFormatter {
            PlaybackTimingFormatter(PlaybackTiming {
                position: Duration::from_secs(seconds),
                duration: None,
            })
        }

        assert_eq!(format!("{}", make_timing(92)), "1:32");
        assert_eq!(format!("{}", make_timing(3599)), "59:59");
        assert_eq!(format!("{}", make_timing(3600)), "1:00:00");
        assert_eq!(format!("{}", make_timing(9492)), "2:38:12");
    }

    #[test]
    fn formatting_timing() {
        fn make_timing(position_seconds: u64, duration_seconds: u64) -> PlaybackTimingFormatter {
            PlaybackTimingFormatter(PlaybackTiming {
                position: Duration::from_secs(position_seconds),
                duration: Some(Duration::from_secs(duration_seconds)),
            })
        }

        assert_eq!(format!("{}", make_timing(40, 92)), "0:40 / 1:32");
        assert_eq!(format!("{}", make_timing(40, 3599)), "00:40 / 59:59");
        assert_eq!(format!("{}", make_timing(40, 3600)), "0:00:40 / 1:00:00");
        assert_eq!(format!("{}", make_timing(40, 9492)), "0:00:40 / 2:38:12");
    }

    #[test]
    fn formatting_duration() {
        fn make_duration(seconds: u64) -> DurationFormatter {
            DurationFormatter(Duration::from_secs(seconds))
        }

        assert_eq!(format!("{}", make_duration(40)), "0:40");
        assert_eq!(format!("{}", make_duration(92)), "1:32");
        assert_eq!(format!("{}", make_duration(3599)), "59:59");
        assert_eq!(format!("{}", make_duration(3600)), "1:00:00");
        assert_eq!(format!("{}", make_duration(9492)), "2:38:12");
    }

    #[test]
    fn duration_width() {
        fn make_duration(hours: u64, minutes: u64, seconds: u64) -> DurationFormatter {
            DurationFormatter(Duration::from_secs(seconds + minutes * 60 + hours * 3600))
        }

        fn assert_width(duration: DurationFormatter) {
            let formatted = format!("{}", duration);
            let width = duration.width();
            assert_eq!(formatted.len() as u16, width, "{}", formatted);
        }

        assert_width(make_duration(0, 0, 0));
        assert_width(make_duration(0, 0, 5));
        assert_width(make_duration(0, 0, 59));
        assert_width(make_duration(0, 1, 00));
        assert_width(make_duration(0, 9, 59));
        assert_width(make_duration(0, 10, 00));
        assert_width(make_duration(0, 10, 00));
        assert_width(make_duration(1, 00, 00));
        assert_width(make_duration(9, 59, 59));
        assert_width(make_duration(10, 00, 00));
        assert_width(make_duration(120, 00, 00));
    }
}
