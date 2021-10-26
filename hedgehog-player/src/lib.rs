pub mod state;

use actix::prelude::*;
use gstreamer::prelude::*;

pub struct Player {
    element: gstreamer::Element,
}

impl Player {
    pub fn initialize() -> Result<(), Box<dyn std::error::Error + 'static>> {
        gstreamer::init().map_err(|error| Box::new(error) as Box<dyn std::error::Error>)
    }

    pub fn new() -> Self {
        Player {
            element: gstreamer::ElementFactory::make("playbin", None).unwrap(),
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
