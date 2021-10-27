pub mod volume;

use actix::prelude::*;
use gstreamer::glib::FlagsClass;
use gstreamer::prelude::*;
use volume::{Volume, VolumeCommand};

#[derive(Debug)]
struct GstError(&'static str);

impl std::error::Error for GstError {}

impl std::fmt::Display for GstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

pub struct Player {
    element: gstreamer::Element,
}

impl Player {
    pub fn initialize() -> Result<(), Box<dyn std::error::Error + 'static>> {
        gstreamer::init().map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
    }

    pub fn init() -> Result<Self, Box<dyn std::error::Error>> {
        let element = gstreamer::ElementFactory::make("playbin", None)?;

        let flags = element.property("flags")?;
        let flags_class =
            FlagsClass::new(flags.type_()).ok_or(GstError("GstPlayFlags not found"))?;
        let flags = flags_class
            .builder()
            .set_by_nick("audio")
            .build()
            .ok_or(GstError("Cannot construct GstPlayFlags"))?;
        element.set_property_from_value("flags", &flags)?;

        Ok(Player { element })
    }

    pub fn emit_error<E: std::error::Error + ?Sized>(&mut self, error: &E) {
        // TODO
        println!("{:?}", error);
    }

    fn set_volume(
        &mut self,
        volume: Volume,
        is_muted: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.element.set_property("mute", is_muted)?;
        self.element.set_property("volume", volume.linear())?;
        Ok(())
    }

    fn volume(&self) -> Result<(Volume, bool), Box<dyn std::error::Error>> {
        let is_muted = self.element.property("mute")?.get()?;
        let volume = self.element.property("volume")?.get()?;
        Ok((Volume::from_linear(volume), is_muted))
    }
}

impl Actor for Player {
    type Context = Context<Self>;
}

#[derive(Debug, Message)]
#[rtype(result = "bool")]
pub enum PlaybackControll {
    Play(String),
    Stop,
}

impl Handler<PlaybackControll> for Player {
    type Result = bool;

    fn handle(&mut self, msg: PlaybackControll, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            PlaybackControll::Play(url) => {
                self.element.set_state(gstreamer::State::Null).unwrap();
                self.element.set_property("uri", url).unwrap();
                self.element.set_state(gstreamer::State::Playing).unwrap();
            }
            PlaybackControll::Stop => {
                self.element.set_state(gstreamer::State::Null).unwrap();
            }
        }
        true
    }
}

impl Handler<VolumeCommand> for Player {
    type Result = ();

    fn handle(&mut self, msg: VolumeCommand, _ctx: &mut Self::Context) -> Self::Result {
        let (mut volume, mut is_muted) = match self.volume() {
            Ok(result) => result,
            Err(error) => {
                self.emit_error(&*error);
                return;
            }
        };

        match msg {
            VolumeCommand::Mute => is_muted = true,
            VolumeCommand::Unmute => is_muted = false,
            VolumeCommand::ToggleMute => is_muted = !is_muted,
            VolumeCommand::SetVolume(new_volume) => volume = new_volume,
            VolumeCommand::AdjustVolume(delta) => volume = volume.add_cubic(delta),
        }

        if let Err(error) = self.set_volume(volume, is_muted) {
            self.emit_error(&*error)
        }
    }
}
