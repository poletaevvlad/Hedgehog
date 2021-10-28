mod gst_utils;
pub mod volume;

use actix::prelude::*;
use gst_utils::{get_property, set_property, GstError};
use gstreamer as gst;
use gstreamer::glib::FlagsClass;
use gstreamer::prelude::*;
use std::error::Error;
use volume::{Volume, VolumeCommand};

pub struct Player {
    element: gst::Element,
    subscribers: Vec<Recipient<PlayerNotification>>,
    reported_volume: Option<Option<Volume>>,
}

impl Player {
    pub fn initialize() -> Result<(), Box<dyn Error + 'static>> {
        gstreamer::init().map_err(|error| Box::new(error) as Box<dyn Error>)
    }

    pub fn init() -> Result<Self, Box<dyn Error>> {
        let element = gstreamer::ElementFactory::make("playbin", None)?;

        let flags = element.property("flags")?;
        let flags_class =
            FlagsClass::new(flags.type_()).ok_or(GstError::from_str("GstPlayFlags not found"))?;
        let flags = flags_class
            .builder()
            .set_by_nick("audio")
            .build()
            .ok_or(GstError::from_str("Cannot construct GstPlayFlags"))?;
        element.set_property_from_value("flags", &flags)?;

        Ok(Player {
            element,
            reported_volume: None,
            subscribers: Vec::new(),
        })
    }

    pub fn emit_error<E: Error + ?Sized>(&mut self, error: &E) {
        // TODO
        println!("{:?}", error);
    }

    fn notify_subscribers(&mut self, notification: PlayerNotification) {
        for subscriber in &self.subscribers {
            if let Err(error) = subscriber.do_send(notification.clone()) {
                self.emit_error(&error);
                return;
            }
        }
    }
}

impl Actor for Player {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        fn handle_volume_changed(element: &gstreamer::Element, addr: &Addr<Player>) {
            let volume = get_property(element, "volume").map(Volume::from_linear);
            let muted = get_property(element, "mute");
            let message = match (volume, muted) {
                (Ok(_), Ok(true)) => InternalEvent::VolumeChanged(None),
                (Ok(volume), Ok(false)) => InternalEvent::VolumeChanged(Some(volume)),
                (Err(error), _) | (_, Err(error)) => InternalEvent::Error(error),
            };
            addr.do_send(message);
        }

        let addr = ctx.address();
        self.element
            .connect_notify(Some("volume"), move |element, _| {
                handle_volume_changed(element, &addr)
            });

        let addr = ctx.address();
        self.element
            .connect_notify(Some("mute"), move |element, _| {
                handle_volume_changed(element, &addr)
            });
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub enum PlaybackControll {
    Play(String),
    Stop,
    Subscribe(Recipient<PlayerNotification>),
}

impl Handler<PlaybackControll> for Player {
    type Result = ();

    fn handle(&mut self, msg: PlaybackControll, _ctx: &mut Self::Context) -> Self::Result {
        let result: Result<(), Box<dyn Error>> = (|| {
            match msg {
                PlaybackControll::Play(url) => {
                    self.element.set_state(gst::State::Null)?;
                    self.element.set_property("uri", url)?;
                    self.element.set_state(gst::State::Playing)?;
                }
                PlaybackControll::Stop => {
                    self.element.set_state(gst::State::Null)?;
                }
                PlaybackControll::Subscribe(recipient) => self.subscribers.push(recipient),
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
        let result = match msg {
            VolumeCommand::Mute => set_property(&mut self.element, "mute", true),
            VolumeCommand::Unmute => set_property(&mut self.element, "mute", false),
            VolumeCommand::ToggleMute => get_property(&mut self.element, "mute")
                .and_then(|muted: bool| set_property(&mut self.element, "mute", !muted)),
            VolumeCommand::SetVolume(new_volume) => {
                set_property(&mut self.element, "volume", new_volume)
            }
            VolumeCommand::AdjustVolume(delta) => get_property(&mut self.element, "volume")
                .and_then(|volume| {
                    let volume = Volume::from_linear(volume).add_cubic(delta);
                    set_property(&mut self.element, "volume", volume)
                }),
        };

        if let Err(error) = result {
            self.emit_error(&error)
        }
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
enum InternalEvent {
    VolumeChanged(Option<Volume>),
    Error(GstError),
}

impl Handler<InternalEvent> for Player {
    type Result = ();

    fn handle(&mut self, msg: InternalEvent, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            InternalEvent::VolumeChanged(volume) => {
                if let Some(reported_volume) = self.reported_volume {
                    if reported_volume != volume {
                        self.notify_subscribers(PlayerNotification::VolumeChanged(volume));
                    }
                }
                self.reported_volume = Some(volume);
            }
            InternalEvent::Error(error) => self.emit_error(&error),
        }
    }
}

#[derive(Debug, Message, Clone)]
#[rtype(result = "()")]
pub enum PlayerNotification {
    VolumeChanged(Option<Volume>),
}
