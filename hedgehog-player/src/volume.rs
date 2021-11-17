use actix::Message;
use cmd_parser::{CmdParsable, ParseError};
use gstreamer_base::glib::{ToValue, Type, Value};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(transparent)]
pub struct Volume(f64);

impl Volume {
    pub const FULL: Volume = Volume(1.0);

    pub fn from_cubic(cubic: f64) -> Self {
        Volume(cubic)
    }

    pub fn from_cubic_clip(cubic: f64) -> Self {
        Volume(cubic.min(1.0).max(0.0))
    }

    pub fn from_linear(linear: f64) -> Self {
        Volume::from_cubic(linear.cbrt())
    }

    pub fn cubic(self) -> f64 {
        self.0
    }

    pub fn linear(self) -> f64 {
        self.0.powi(3)
    }

    pub fn add_cubic(self, delta: f64) -> Self {
        Volume::from_cubic_clip(self.0 + delta)
    }
}

impl fmt::Display for Volume {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "{:.precision$}%",
            self.0.abs() * 100.0,
            precision = f.precision().unwrap_or(0)
        ))
    }
}

impl ToValue for Volume {
    fn to_value(&self) -> Value {
        self.linear().to_value()
    }

    fn value_type(&self) -> Type {
        Type::F64
    }
}

impl CmdParsable for Volume {
    fn parse_cmd_raw(input: &str) -> Result<(Self, &str), ParseError<'_>> {
        let (percentage, input) = f64::parse_cmd_raw(input)?;
        Ok((Volume::from_cubic_clip(percentage / 100.0), input))
    }
}

#[derive(Debug, Copy, Clone, Message, PartialEq, CmdParsable)]
#[rtype(result = "()")]
pub enum VolumeCommand {
    Mute,
    Unmute,
    ToggleMute,
    #[cmd(rename = "set")]
    SetVolume(Volume),
    #[cmd(rename = "adjust")]
    AdjustVolume(f64),
}

#[cfg(test)]
mod tests {
    use super::Volume;

    #[test]
    fn display() {
        assert_eq!(Volume::from_cubic(-0.0).to_string(), "0%");
        assert_eq!(Volume::from_cubic(0.0).to_string(), "0%");
        assert_eq!(Volume::from_cubic(0.4).to_string(), "40%");
        assert_eq!(Volume::from_cubic(1.0).to_string(), "100%");
        assert_eq!(format!("{:.2}", Volume::from_cubic(0.4)), "40.00%");
    }

    #[test]
    fn converting_cubic_linear() {
        for i in 0..=10 {
            let linear = Volume::from_linear(i as f64 / 10.0);
            let cubic = Volume::from_cubic(linear.cubic());
            let new_linear = Volume::from_linear(cubic.linear());
            assert!((linear.linear() - new_linear.linear()).abs() < 1e-7);
            assert!((linear.cubic() - new_linear.cubic()).abs() < 1e-7);
        }

        let volume = Volume::from_linear(0.125);
        assert!((volume.cubic() - 0.5) < 1e-7);
    }

    #[test]
    fn add_cubic_clipping() {
        assert_eq!(Volume::from_cubic(0.7).add_cubic(0.2).to_string(), "90%");
        assert_eq!(Volume::from_cubic(0.7).add_cubic(0.4).to_string(), "100%");
        assert_eq!(Volume::from_cubic(0.3).add_cubic(-0.2).to_string(), "10%");
        assert_eq!(Volume::from_cubic(0.3).add_cubic(-0.4).to_string(), "0%");
    }
}
