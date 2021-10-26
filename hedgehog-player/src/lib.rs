pub mod volume;

use actix::prelude::*;
use gstreamer::prelude::*;
use volume::{Volume, VolumeCommand};

pub struct Player {
    element: gstreamer::Element,

    volume: Volume,
    is_muted: bool,
}

impl Player {
    pub fn initialize() -> Result<(), Box<dyn std::error::Error + 'static>> {
        gstreamer::init().map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
    }

    pub fn new() -> Self {
        Player {
            element: gstreamer::ElementFactory::make("playbin", None).unwrap(),
            volume: Volume::FULL,
            is_muted: false,
        }
    }

    pub fn emit_error<E: std::error::Error>(&mut self, error: E) {
        // TODO
        println!("{:?}", error);
    }

    fn set_volume(&mut self, volume: Volume, is_muted: bool) {
        self.volume = volume;
        self.is_muted = is_muted;

        let result = self
            .element
            .set_property("mute", self.is_muted)
            .and_then(|_| self.element.set_property("volume", self.volume.linear()));
        if let Err(error) = result {
            self.emit_error(error)
        }
    }

    fn volume(&self) -> Option<Volume> {
        if self.is_muted {
            None
        } else {
            Some(self.volume)
        }
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
    type Result = Option<Volume>;

    fn handle(&mut self, msg: VolumeCommand, _ctx: &mut Self::Context) -> Self::Result {
        let mut volume = self.volume;
        let mut is_muted = self.is_muted;
        match msg {
            VolumeCommand::Mute => is_muted = true,
            VolumeCommand::Unmute => is_muted = false,
            VolumeCommand::ToggleMute => is_muted = !is_muted,
            VolumeCommand::SetVolume(new_volume) => volume = new_volume,
            VolumeCommand::AdjustVolume(delta) => volume = volume.add_cubic(delta),
        }

        self.set_volume(volume, is_muted);
        self.volume()
    }
}
