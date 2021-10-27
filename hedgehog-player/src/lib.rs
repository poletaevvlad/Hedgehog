pub mod volume;

use actix::prelude::*;
use gstreamer::glib::FlagsClass;
use gstreamer::prelude::*;
use std::error::Error;
use volume::{Volume, VolumeCommand};

#[derive(Debug)]
struct GstError(&'static str);

impl Error for GstError {}

impl std::fmt::Display for GstError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

fn box_err(err: impl Error + Send + 'static) -> Box<dyn Error + Send + 'static> {
    Box::new(err)
}

pub struct Player {
    element: gstreamer::Element,
    volume: Option<Volume>,
    muted: Option<bool>,
}

impl Player {
    pub fn initialize() -> Result<(), Box<dyn Error + 'static>> {
        gstreamer::init().map_err(|error| Box::new(error) as Box<dyn Error>)
    }

    pub fn init() -> Result<Self, Box<dyn Error>> {
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

        Ok(Player {
            element,
            volume: None,
            muted: None,
        })
    }

    pub fn emit_error<E: Error + ?Sized>(&mut self, error: &E) {
        // TODO
        println!("{:?}", error);
    }
}

impl Actor for Player {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let addr = ctx.address();
        self.element
            .connect_notify(Some("volume"), move |element, _| {
                let volume = element
                    .property("volume")
                    .map_err(box_err)
                    .and_then(|volume| volume.get().map_err(box_err));
                let message = match volume {
                    Ok(volume) => InternalEvent::VolumeChanged(Volume::from_linear(volume)),
                    Err(error) => InternalEvent::Error(error),
                };
                addr.do_send(message)
            });

        let addr = ctx.address();
        self.element
            .connect_notify(Some("mute"), move |element, _| {
                let muted = element
                    .property("mute")
                    .map_err(box_err)
                    .and_then(|muted| muted.get().map_err(box_err));
                let message = match muted {
                    Ok(muted) => InternalEvent::MutedChanged(muted),
                    Err(error) => InternalEvent::Error(error),
                };
                addr.do_send(message);
            });
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub enum PlaybackControll {
    Play(String),
    Stop,
}

impl Handler<PlaybackControll> for Player {
    type Result = ();

    fn handle(&mut self, msg: PlaybackControll, _ctx: &mut Self::Context) -> Self::Result {
        let result: Result<(), Box<dyn Error>> = (|| {
            match msg {
                PlaybackControll::Play(url) => {
                    self.element.set_state(gstreamer::State::Null)?;
                    self.element.set_property("uri", url)?;
                    self.element.set_state(gstreamer::State::Playing)?;
                }
                PlaybackControll::Stop => {
                    self.element.set_state(gstreamer::State::Null)?;
                }
            }
            Ok(())
        })();

        if let Err(error) = result {
            self.emit_error(&*error)
        }
    }
}

impl Handler<VolumeCommand> for Player {
    type Result = ();

    fn handle(&mut self, msg: VolumeCommand, _ctx: &mut Self::Context) -> Self::Result {
        macro_rules! set_property {
            ($name:ident, $property:literal, |$var:pat| $result:expr) => {
                if let Some($var) = self.$name.take() {
                    let new_value = $result;
                    if let Err(error) = self.element.set_property($property, new_value) {
                        self.emit_error(&error);
                    } else {
                        self.$name = Some(new_value);
                    }
                }
            };
        }

        match msg {
            VolumeCommand::Mute => set_property!(muted, "mute", |_| true),
            VolumeCommand::Unmute => set_property!(muted, "mute", |_| false),
            VolumeCommand::ToggleMute => set_property!(muted, "mute", |muted| !muted),
            VolumeCommand::SetVolume(new_volume) => {
                set_property!(volume, "volume", |_| new_volume)
            }
            VolumeCommand::AdjustVolume(delta) => {
                set_property!(volume, "volume", |volume| volume.add_cubic(delta))
            }
        }
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
enum InternalEvent {
    VolumeChanged(Volume),
    MutedChanged(bool),
    Error(Box<dyn Error + Send + 'static>),
}

impl Handler<InternalEvent> for Player {
    type Result = ();

    fn handle(&mut self, msg: InternalEvent, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            InternalEvent::VolumeChanged(volume) => self.volume = Some(volume),
            InternalEvent::MutedChanged(muted) => self.muted = Some(muted),
            InternalEvent::Error(error) => self.emit_error(&*error),
        }
    }
}
