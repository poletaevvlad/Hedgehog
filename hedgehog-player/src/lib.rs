mod gst_utils;
pub mod mpris;
pub mod state;
pub mod volume;

use actix::prelude::*;
use cmdparse::{
    error::{ParseError, UnrecognizedToken},
    tokens::{Token, TokenStream},
    CompletionResult, Parsable, ParseResult, Parser,
};
pub use gst_utils::GstError;
use gst_utils::{build_flags, get_property, set_property};
use gstreamer_base::{gst, gst::prelude::*, BaseParse};
use std::{str::FromStr, time::Duration};
use volume::{Volume, VolumeCommand};

#[derive(Debug, Default, Copy, Clone)]
pub struct State {
    pub(crate) is_started: bool,
    pub(crate) is_paused: bool,
    pub(crate) is_buffering: bool,
}

impl State {
    fn is_playing(&self) -> bool {
        self.is_started && !self.is_paused && !self.is_buffering
    }
}

pub struct Player {
    element: Option<gst::Element>,
    subscribers: Vec<Recipient<PlayerNotification>>,
    error_listener: Option<Recipient<PlayerErrorNotification>>,
    reported_volume: Option<Option<Volume>>,
    state: Option<State>,
    required_seek: Option<Duration>,
    seek_position: Option<Duration>,
}

impl Player {
    pub fn initialize() -> Result<(), GstError> {
        gst::init().map_err(GstError::from_err)
    }

    pub fn init() -> Result<Self, GstError> {
        let mut element = gst::ElementFactory::make("playbin", None).map_err(GstError::from_err)?;

        let flags = build_flags("GstPlayFlags", ["audio", "download"])?;
        set_property(&mut element, "flags", flags)?;

        Ok(Player {
            element: Some(element),
            reported_volume: None,
            subscribers: Vec::new(),
            error_listener: None,
            state: None,
            required_seek: None,
            seek_position: None,
        })
    }

    pub fn init_uninitialized() -> Self {
        Player {
            element: None,
            subscribers: Vec::new(),
            error_listener: None,
            reported_volume: None,
            state: None,
            required_seek: None,
            seek_position: None,
        }
    }

    fn emit_error(&mut self, error: GstError) {
        if let Some(ref listener) = self.error_listener {
            if let Err(SendError::Closed(_)) = listener.do_send(PlayerErrorNotification(error)) {
                self.error_listener = None;
            }
        }
    }

    fn set_state(&mut self, state: Option<State>) {
        self.state = state;
        self.notify_subscribers(PlayerNotification::StateChanged(self.state));
    }

    fn notify_subscribers(&mut self, notification: PlayerNotification) {
        for subscriber in &self.subscribers {
            if let Err(error) = subscriber.do_send(notification.clone()) {
                self.emit_error(GstError::from_err(error));
                return;
            }
        }
    }
}

impl Actor for Player {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let element = match &self.element {
            Some(element) => element,
            None => return,
        };
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
        element.connect_notify(Some("volume"), move |element, _| {
            handle_volume_changed(element, &addr);
        });

        let addr = ctx.address();
        element.connect_notify(Some("mute"), move |element, _| {
            handle_volume_changed(element, &addr);
        });

        if let Some(bus) = element.bus() {
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

        ctx.address().do_send(TimerTick);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        if let Some(element) = &self.element {
            let _ = element.set_state(gst::State::Null);
        }
    }
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub enum ActorCommand {
    Subscribe(Recipient<PlayerNotification>),
    SubscribeErrors(Recipient<PlayerErrorNotification>),
}

impl Handler<ActorCommand> for Player {
    type Result = ();

    fn handle(&mut self, msg: ActorCommand, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            ActorCommand::Subscribe(recipient) => self.subscribers.push(recipient),
            ActorCommand::SubscribeErrors(recipient) => self.error_listener = Some(recipient),
        }
    }
}

#[derive(Default)]
struct DurationParser;

impl DurationParser {
    fn parse_duration(input: &str) -> Result<Duration, ()> {
        let mut parts = input.rsplitn(3, ':');
        let mut seconds: f64 = match parts.next().unwrap().parse() {
            Ok(seconds) => seconds,
            Err(_) => return Err(()),
        };

        let mut multiplier = 60.0;
        for part in parts {
            match part.parse::<u64>() {
                Ok(part) => {
                    seconds += multiplier * part as f64;
                    multiplier *= 60.0;
                }
                Err(_) => return Err(()),
            }
        }
        Ok(Duration::from_secs_f64(seconds))
    }
}

impl<Ctx> Parser<Ctx> for DurationParser {
    type Value = Duration;

    fn parse<'a>(&self, input: TokenStream<'a>, _ctx: Ctx) -> ParseResult<'a, Self::Value> {
        match input.take().transpose()? {
            Some((token @ Token::Attribute(_), remaining)) => {
                Err(UnrecognizedToken::new(token, remaining).into())
            }
            Some((token @ Token::Text(text), remaining)) => {
                let text = text.parse_string();
                match DurationParser::parse_duration(&text) {
                    Ok(duration) => Ok((duration, remaining)),
                    Err(_) => Err(ParseError::invalid(token, None).expected("duration").into()),
                }
            }
            None => Err(ParseError::token_required().expected("duration").into()),
        }
    }

    fn complete<'a>(&self, input: TokenStream<'a>, _ctx: Ctx) -> CompletionResult<'a> {
        match input.take() {
            Some(Ok((Token::Text(_), remaining))) if remaining.is_all_consumed() => {
                CompletionResult::new_final(true)
            }
            Some(Ok((Token::Text(_), remaining))) => CompletionResult::new(remaining, true),
            Some(Ok((Token::Attribute(_), _))) => CompletionResult::new(input, false),
            Some(Err(_)) | None => CompletionResult::new_final(false),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SeekDirection {
    Forward,
    Backward,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct SeekOffset(pub SeekDirection, pub Duration);

impl FromStr for SeekOffset {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (direction, tail) = if let Some(tail) = s.strip_prefix('-') {
            (SeekDirection::Backward, tail)
        } else if let Some(tail) = s.strip_prefix('+') {
            (SeekDirection::Forward, tail)
        } else {
            (SeekDirection::Forward, s)
        };

        let duration = DurationParser::parse_duration(tail)?;
        Ok(SeekOffset(direction, duration))
    }
}

impl<Ctx> Parsable<Ctx> for SeekOffset {
    type Parser = cmdparse::parsers::FromStrParser<Self>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackMetadata {
    pub episode_id: i64,
    pub episode_title: Option<String>,
    pub feed_title: Option<String>,
}

#[derive(Debug, Message, PartialEq, Clone, cmdparse::Parsable)]
#[rtype(result = "()")]
pub enum PlaybackCommand {
    #[cmd(ignore)]
    Play(
        String,
        #[cmd(parser = "DurationParser")] Duration,
        Option<PlaybackMetadata>,
    ),
    Stop,
    Pause,
    Resume,
    TogglePause,
    #[cmd(transparent_no_error)]
    Seek(#[cmd(parser = "DurationParser")] Duration),
    #[cmd(rename = "seek")]
    SeekRelative(SeekOffset),
    #[cmd(rename = "rate")]
    SetRate(f64),
}

impl Handler<PlaybackCommand> for Player {
    type Result = ();

    fn handle(&mut self, msg: PlaybackCommand, _ctx: &mut Self::Context) -> Self::Result {
        if self.element.is_none() {
            return;
        }
        let result: Result<(), GstError> = (|| {
            match msg {
                PlaybackCommand::Play(url, position, metadata) => {
                    self.set_state(Some(State::default()));
                    let element = self.element.as_ref().unwrap();
                    element
                        .set_state(gst::State::Null)
                        .map_err(GstError::from_err)?;
                    element
                        .set_property("uri", url)
                        .map_err(GstError::from_err)?;
                    element
                        .set_state(gst::State::Playing)
                        .map_err(GstError::from_err)?;
                    self.required_seek = if position.is_zero() {
                        None
                    } else {
                        Some(position)
                    };
                    self.seek_position = None;
                    if let Some(metadata) = metadata {
                        self.notify_subscribers(PlayerNotification::MetadataChanged(metadata));
                    }
                }
                PlaybackCommand::Stop => {
                    if self.state.is_some() {
                        self.set_state(None);
                        let element = self.element.as_ref().unwrap();
                        element
                            .set_state(gst::State::Null)
                            .map_err(GstError::from_err)?;
                    }
                }
                PlaybackCommand::Pause => {
                    if let Some(state) = self.state {
                        if !state.is_paused {
                            self.set_state(Some(State {
                                is_paused: true,
                                ..state
                            }));
                            let element = self.element.as_ref().unwrap();
                            element
                                .set_state(gst::State::Paused)
                                .map_err(GstError::from_err)?;
                        }
                    }
                }
                PlaybackCommand::Resume => {
                    if let Some(state) = self.state {
                        if state.is_paused {
                            self.set_state(Some(State {
                                is_paused: false,
                                ..state
                            }));
                            let element = self.element.as_ref().unwrap();
                            element
                                .set_state(gst::State::Playing)
                                .map_err(GstError::from_err)?;
                        }
                    }
                }
                PlaybackCommand::TogglePause => {
                    if let Some(state) = self.state {
                        let is_paused = !state.is_paused;
                        self.set_state(Some(State { is_paused, ..state }));
                        let element = self.element.as_ref().unwrap();
                        element
                            .set_state(if is_paused {
                                gst::State::Paused
                            } else {
                                gst::State::Playing
                            })
                            .map_err(GstError::from_err)?;
                    }
                }
                PlaybackCommand::Seek(position) => {
                    if self.state.map(|state| state.is_started) == Some(true) {
                        let element = self.element.as_ref().unwrap();
                        element
                            .seek_simple(
                                gst::SeekFlags::TRICKMODE.union(gst::SeekFlags::FLUSH),
                                gst::ClockTime::from_nseconds(position.as_nanos() as u64),
                            )
                            .map_err(GstError::from_err)?;

                        self.seek_position = Some(position);
                        self.notify_subscribers(PlayerNotification::PositionSet {
                            position,
                            seeked: true,
                        });
                    }
                }
                PlaybackCommand::SeekRelative(SeekOffset(direction, duration)) => {
                    if self.state.map(|state| state.is_started) == Some(true) {
                        let element = self.element.as_ref().unwrap();
                        let current_position =
                            element.query_position::<gst::ClockTime>().or_else(|| {
                                self.seek_position
                                    .map(|pos| gst::ClockTime::from_nseconds(pos.as_nanos() as u64))
                            });

                        if let Some(current_position) = current_position {
                            let delta = gst::ClockTime::from_nseconds(duration.as_nanos() as u64);
                            let new_position = match direction {
                                SeekDirection::Forward => current_position.saturating_add(delta),
                                SeekDirection::Backward => current_position.saturating_sub(delta),
                            };
                            element
                                .seek_simple(
                                    gst::SeekFlags::TRICKMODE.union(gst::SeekFlags::FLUSH),
                                    new_position,
                                )
                                .map_err(GstError::from_err)?;

                            let pos_duration = Duration::from_nanos(new_position.nseconds());
                            self.notify_subscribers(PlayerNotification::PositionSet {
                                position: pos_duration,
                                seeked: true,
                            });
                            self.seek_position = Some(pos_duration);
                        }
                    }
                }
                PlaybackCommand::SetRate(speed) => {
                    if self.state.map(|state| state.is_started) == Some(true) {
                        let element = self.element.as_ref().unwrap();
                        let current_position =
                            element.query_position::<gst::ClockTime>().or_else(|| {
                                self.seek_position
                                    .map(|pos| gst::ClockTime::from_nseconds(pos.as_nanos() as u64))
                            });

                        if let Some(current_position) = current_position {
                            element
                                .seek(
                                    speed,
                                    gst::SeekFlags::FLUSH,
                                    gst::SeekType::Set,
                                    Some(current_position),
                                    gst::SeekType::Set,
                                    gst::ClockTime::NONE,
                                )
                                .map_err(GstError::from_err)?;
                        }
                    }
                }
            }
            Ok(())
        })();

        if let Err(error) = result {
            self.emit_error(error);
        }
    }
}

impl Handler<VolumeCommand> for Player {
    type Result = ();

    fn handle(&mut self, msg: VolumeCommand, _ctx: &mut Self::Context) -> Self::Result {
        if self.state.is_none() {
            return;
        }
        let element = match self.element {
            Some(ref mut element) => element,
            None => return,
        };
        let result = match msg {
            VolumeCommand::SetMuted(muted) => set_property(element, "mute", muted),
            VolumeCommand::ToggleMute => get_property(element, "mute")
                .and_then(|muted: bool| set_property(element, "mute", !muted)),
            VolumeCommand::SetVolume(new_volume) => set_property(element, "volume", new_volume),
            VolumeCommand::AdjustVolume(delta) => {
                get_property(element, "volume").and_then(|volume| {
                    let volume = Volume::from_linear(volume).add_cubic(delta);
                    set_property(element, "volume", volume)
                })
            }
        };

        if let Err(error) = result {
            self.emit_error(error);
        }
    }
}

#[derive(Debug, Message)]
#[rtype(result = "Result<Option<Volume>, GstError>")]
pub struct VolumeQueryRequest;

impl Handler<VolumeQueryRequest> for Player {
    type Result = Result<Option<Volume>, GstError>;

    fn handle(&mut self, _msg: VolumeQueryRequest, _ctx: &mut Self::Context) -> Self::Result {
        let element = match &self.element {
            Some(element) => element,
            None => return Ok(None),
        };
        let volume = get_property(element, "volume");
        let muted = get_property(element, "mute");
        match (volume, muted) {
            (Ok(volume), Ok(false)) => Ok(Some(Volume::from_linear(volume))),
            (Ok(_), Ok(true)) => Ok(None),
            (Ok(_), Err(err)) => Err(err),
            (Err(err), _) => Err(err),
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
            InternalEvent::Error(error) => self.emit_error(error),
        }
    }
}

#[derive(Debug, Message, Clone)]
#[rtype(result = "()")]
pub enum PlayerNotification {
    MetadataChanged(PlaybackMetadata),
    VolumeChanged(Option<Volume>),
    StateChanged(Option<State>),
    DurationSet(Duration),
    PositionSet { position: Duration, seeked: bool },
    Eos,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct PlayerErrorNotification(pub GstError);

impl StreamHandler<gst::Message> for Player {
    fn handle(&mut self, item: gst::Message, ctx: &mut Self::Context) {
        if let Some(state) = self.state {
            match item.view() {
                gst::MessageView::Eos(_) => {
                    self.notify_subscribers(PlayerNotification::Eos);
                    self.set_state(None);
                }
                gst::MessageView::Error(error) => {
                    self.emit_error(GstError::from_err(error.error()));
                    self.set_state(None);
                }
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
                    let element = match &self.element {
                        Some(element) => element,
                        None => return,
                    };
                    if state_changed.pending() == gst::State::VoidPending
                        && state_changed.src().as_ref() == Some(element.upcast_ref())
                    {
                        #[allow(clippy::collapsible_if)]
                        if !state.is_started && state_changed.current() == gst::State::Playing {
                            self.set_state(Some(State {
                                is_started: true,
                                ..state
                            }));
                            if let Some(seek) = self.required_seek.take() {
                                ctx.address().do_send(PlaybackCommand::Seek(seek));
                            }
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

#[derive(Message)]
#[rtype(result = "()")]
struct TimerTick;

impl Handler<TimerTick> for Player {
    type Result = ();

    fn handle(&mut self, _msg: TimerTick, ctx: &mut Self::Context) -> Self::Result {
        if let Some(true) = self.state.as_ref().map(State::is_playing) {
            if let Some(position) = self
                .element
                .as_ref()
                .and_then(|element| element.query_position::<gst::ClockTime>())
            {
                let position = Duration::from_nanos(position.nseconds());
                self.notify_subscribers(PlayerNotification::PositionSet {
                    position,
                    seeked: false,
                });
            }
        }
        ctx.spawn(
            actix::clock::sleep(Duration::from_secs(1))
                .into_actor(self)
                .map(|_, _actor, ctx| ctx.address().do_send(TimerTick)),
        );
    }
}
