mod gst_utils;
pub mod state;
pub mod volume;

use actix::prelude::*;
use gst_utils::{build_flags, get_property, set_property, GstError};
use gstreamer_base::{gst, gst::prelude::*, BaseParse};
use std::error::Error;
use std::time::Duration;
use volume::{Volume, VolumeCommand};

#[derive(Debug, Default, Copy, Clone)]
pub struct State {
    pub(crate) is_started: bool,
    pub(crate) is_paused: bool,
    pub(crate) is_buffering: bool,
}

pub struct Player {
    element: gst::Element,
    subscribers: Vec<Recipient<PlayerNotification>>,
    reported_volume: Option<Option<Volume>>,
    state: Option<State>,
}

impl Player {
    pub fn initialize() -> Result<(), GstError> {
        gst::init().map_err(GstError::from_err)
    }

    pub fn init() -> Result<Self, GstError> {
        let mut element = gst::ElementFactory::make("playbin", None).map_err(GstError::from_err)?;

        let flags = build_flags("GstPlayFlags", ["audio"])?;
        set_property(&mut element, "flags", flags)?;

        Ok(Player {
            element,
            reported_volume: None,
            subscribers: Vec::new(),
            state: None,
        })
    }

    fn emit_error<E: Error + ?Sized>(&mut self, error: &E) {
        // TODO
        println!("{:?}", error);
    }

    fn set_state(&mut self, state: Option<State>) {
        self.state = state;
        self.notify_subscribers(PlayerNotification::StateChanged(self.state))
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
        fn handle_volume_changed(element: &gst::Element, addr: &Addr<Player>) {
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

        if let Some(bus) = self.element.bus() {
            ctx.add_stream(bus.stream_filtered(&[
                gst::MessageType::Eos,
                gst::MessageType::Error,
                gst::MessageType::Buffering,
                gst::MessageType::StateChanged,
                gst::MessageType::DurationChanged,
            ]));
        } else {
            ctx.address()
                .do_send(InternalEvent::Error(GstError::from_str(
                    "element does not have a bus",
                )));
        }
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub enum AgentCommand {
    Subscribe(Recipient<PlayerNotification>),
}

impl Handler<AgentCommand> for Player {
    type Result = ();

    fn handle(&mut self, msg: AgentCommand, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            AgentCommand::Subscribe(recipient) => self.subscribers.push(recipient),
        }
    }
}

#[derive(Debug)]
pub enum SeekDirection {
    Forward,
    Backward,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub enum PlaybackControl {
    Play(String),
    Stop,
    Pause,
    Resume,
    Seek(Duration),
    SeekRelative(Duration, SeekDirection),
}

impl Handler<PlaybackControl> for Player {
    type Result = ();

    fn handle(&mut self, msg: PlaybackControl, _ctx: &mut Self::Context) -> Self::Result {
        let result: Result<(), Box<dyn Error>> = (|| {
            match msg {
                PlaybackControl::Play(url) => {
                    self.set_state(Some(State::default()));
                    self.element.set_state(gst::State::Null)?;
                    self.element.set_property("uri", url)?;
                    self.element.set_state(gst::State::Playing)?;
                }
                PlaybackControl::Stop => {
                    if self.state.is_some() {
                        self.set_state(Some(State::default()));
                        self.element.set_state(gst::State::Null)?;
                    }
                }
                PlaybackControl::Pause => {
                    if let Some(state) = self.state {
                        if !state.is_paused {
                            self.set_state(Some(State {
                                is_paused: true,
                                ..state
                            }));
                            self.element.set_state(gst::State::Paused)?;
                        }
                    }
                }
                PlaybackControl::Resume => {
                    if let Some(state) = self.state {
                        if state.is_paused {
                            self.set_state(Some(State {
                                is_paused: false,
                                ..state
                            }));
                            self.element.set_state(gst::State::Playing)?;
                        }
                    }
                }
                PlaybackControl::Seek(position) => {
                    if self.state.is_some() {
                        self.element
                            .seek_simple(
                                gst::SeekFlags::TRICKMODE.union(gst::SeekFlags::FLUSH),
                                gst::ClockTime::from_nseconds(position.as_nanos() as u64),
                            )
                            .map_err(GstError::from_err)?;
                    }
                }
                PlaybackControl::SeekRelative(duration, direction) => {
                    if self.state.is_some() {
                        if let Some(current_position) =
                            self.element.query_position::<gst::ClockTime>()
                        {
                            let delta = gst::ClockTime::from_nseconds(duration.as_nanos() as u64);
                            let new_position = match direction {
                                SeekDirection::Forward => current_position.saturating_add(delta),
                                SeekDirection::Backward => current_position.saturating_sub(delta),
                            };
                            self.element
                                .seek_simple(
                                    gst::SeekFlags::TRICKMODE.union(gst::SeekFlags::FLUSH),
                                    new_position,
                                )
                                .unwrap();
                        }
                    }
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
        let result = match msg {
            VolumeCommand::Mute => set_property(&mut self.element, "mute", true),
            VolumeCommand::Unmute => set_property(&mut self.element, "mute", false),
            VolumeCommand::ToggleMute => get_property(&self.element, "mute")
                .and_then(|muted: bool| set_property(&mut self.element, "mute", !muted)),
            VolumeCommand::SetVolume(new_volume) => {
                set_property(&mut self.element, "volume", new_volume)
            }
            VolumeCommand::AdjustVolume(delta) => {
                get_property(&self.element, "volume").and_then(|volume| {
                    let volume = Volume::from_linear(volume).add_cubic(delta);
                    set_property(&mut self.element, "volume", volume)
                })
            }
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
    StateChanged(Option<State>),
    DurationSet(Duration),
}

impl StreamHandler<gst::Message> for Player {
    fn handle(&mut self, item: gst::Message, _ctx: &mut Self::Context) {
        if let Some(state) = self.state {
            match item.view() {
                gst::MessageView::Eos(_) => self.set_state(None),
                gst::MessageView::Error(_) => self.set_state(None),
                gst::MessageView::Buffering(buffering) => {
                    let is_buffering = buffering.percent() != 100;
                    if state.is_buffering != is_buffering {
                        self.set_state(Some(State {
                            is_buffering,
                            ..state
                        }));
                    }
                }
                gst::MessageView::StateChanged(state_changed) => {
                    if state_changed.pending() == gst::State::VoidPending
                        && state_changed.src().as_ref() == Some(self.element.upcast_ref())
                    {
                        #[allow(clippy::collapsible_if)]
                        if !state.is_started && state_changed.current() == gst::State::Playing {
                            self.set_state(Some(State {
                                is_started: true,
                                ..state
                            }));
                        }
                    }
                }
                gst::MessageView::DurationChanged(duration_changed) => {
                    let clock_time = duration_changed.src().and_then(|src| {
                        src.downcast_ref::<BaseParse>()?
                            .query_duration::<gst::ClockTime>()
                    });
                    if let Some(src) = clock_time {
                        let duration = Duration::from_nanos(src.nseconds());
                        self.notify_subscribers(PlayerNotification::DurationSet(duration));
                    }
                }
                _ => (),
            }
        }
    }
}
