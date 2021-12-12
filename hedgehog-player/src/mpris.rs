use crate::state::PlaybackState;
use crate::volume::Volume;
use crate::{
    ActorCommand, PlaybackCommand, PlaybackMetadata, Player, PlayerNotification, SeekDirection,
    State, VolumeCommand, VolumeQueryRequest,
};
use actix::fut::wrap_future;
use actix::prelude::*;
use dbus::arg::{RefArg, Variant};
use dbus::channel::{MatchingReceiver, Sender};
use dbus::message::MatchRule;
use dbus::nonblock::SyncConnection;
use dbus::MethodErr;
use dbus_crossroads::{Crossroads, IfaceBuilder};
use dbus_tokio::connection;
use std::collections::HashMap;
use std::fmt::Display;
use std::process;
use std::sync::{Arc, RwLock};
use std::time::Duration;

type PropChangeCallback =
    Box<dyn Fn(&dbus::Path, &dyn RefArg) -> Option<dbus::Message> + Send + Sync>;

#[derive(Default)]
struct PlayerState {
    state: PlaybackState,
    metadata: Option<PlaybackMetadata>,
}

impl PlayerState {
    fn construct_mpris_metadata(&self) -> HashMap<String, Variant<Box<dyn RefArg>>> {
        let mut metadata = HashMap::<String, Variant<Box<dyn RefArg>>>::new();
        let duration = self.state.timing().and_then(|timing| timing.duration);
        if let Some(duration) = duration {
            metadata.insert(
                "mpris:length".to_string(),
                Variant(Box::new(duration.as_micros() as i64)),
            );
        }
        if let Some(player_medatata) = &self.metadata {
            let track_id = format!(
                "/org/mpris/MediaPlayer2/Episodes/{}",
                player_medatata.episode_id
            );
            metadata.insert(
                "mpris:trackid".to_string(),
                Variant(Box::new(dbus::Path::from(track_id))),
            );
            metadata.insert(
                "xesam:title".to_string(),
                Variant(Box::new(
                    player_medatata
                        .episode_title
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(String::new),
                )),
            );
            metadata.insert(
                "xesam:album".to_string(),
                Variant(Box::new(
                    player_medatata
                        .feed_title
                        .as_ref()
                        .cloned()
                        .unwrap_or_else(String::new),
                )),
            );
        }
        metadata
    }
}

struct DBusCallbacks {
    volume_changed: PropChangeCallback,
    status_changed: PropChangeCallback,
    metadata_changed: PropChangeCallback,
    seeked_signal: Box<dyn Fn(&dbus::Path, &(i64,)) -> dbus::Message + Send + Sync>,
}

pub struct MprisPlayer {
    player: Addr<Player>,
    playback_state: Arc<RwLock<PlayerState>>,
    connection: Option<Arc<SyncConnection>>,
    dbus_callbacks: Option<DBusCallbacks>,
}

impl MprisPlayer {
    pub fn new(player: Addr<Player>) -> Self {
        MprisPlayer {
            player,
            playback_state: Arc::new(RwLock::new(PlayerState::default())),
            connection: None,
            dbus_callbacks: None,
        }
    }
}

impl Actor for MprisPlayer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let context = MprisContext {
            player: self.player.clone(),
            state: self.playback_state.clone(),
        };

        ctx.spawn(
            wrap_future(async {
                let (resource, connection) = connection::new_session_sync().unwrap();
                let arbiter = Arbiter::current();

                arbiter.spawn(async {
                    resource.await;
                });

                let pid = process::id();
                let name = format!("org.mpris.MediaPlayer2.hedgehog.instance{}", pid);
                let name_request = connection.request_name(name, false, true, false);
                if name_request.await.is_err() {
                    return (None, None);
                }

                let mut cr = Crossroads::new();
                cr.set_async_support(Some((
                    connection.clone(),
                    Box::new(move |x| {
                        arbiter.spawn(x);
                    }),
                )));

                let iface = cr.register("org.mpris.MediaPlayer2", |b| {
                    b.method("Raise", (), (), |_, _, ()| Ok(()));
                    b.method("Quit", (), (), |_, _, ()| {
                        System::current().stop();
                        Ok(())
                    });

                    b.property("CanQuit").get(|_, _| Ok(true));
                    b.property("CanRaise").get(|_, _| Ok(false));
                    b.property("HasTrackList").get(|_, _| Ok(false));
                    b.property("DesktopEntry")
                        .get(|_, _| Ok("hedgehog".to_string()));
                    b.property("Identity")
                        .get(|_, _| Ok("Hedgehog podcast player".to_string()));
                    b.property("SupportedUriSchemes")
                        .get(|_, _| Ok(Vec::<String>::new()));
                    b.property("SupportedMimeTypes")
                        .get(|_, _| Ok(Vec::<String>::new()));
                });

                let mut callbacks = None;
                let player_iface = cr.register("org.mpris.MediaPlayer2.Player", |builder| {
                    build_player_interface(builder, &mut callbacks);
                });
                cr.insert("/org/mpris/MediaPlayer2", &[iface, player_iface], context);
                connection.start_receive(
                    MatchRule::new_method_call(),
                    Box::new(move |msg, conn| {
                        let _ = cr.handle_message(msg, conn);
                        true
                    }),
                );
                (callbacks, Some(connection))
            })
            .map(|(callbacks, connection), actor: &mut MprisPlayer, _ctx| {
                actor.connection = connection;
                actor.dbus_callbacks = callbacks;
            }),
        );

        self.player
            .do_send(ActorCommand::Subscribe(ctx.address().recipient()));
    }
}

struct MprisContext {
    player: Addr<Player>,
    state: Arc<RwLock<PlayerState>>,
}

fn build_player_interface(
    b: &mut IfaceBuilder<MprisContext>,
    callbacks: &mut Option<DBusCallbacks>,
) {
    b.method("Next", (), (), |_, _, ()| Ok(()));
    b.method("Previous", (), (), |_, _, ()| Ok(()));
    b.method("Pause", (), (), |_, mpris_ctx, ()| {
        mpris_ctx.player.do_send(PlaybackCommand::Pause);
        Ok(())
    });
    b.method("PlayPause", (), (), |_, mpris_ctx, ()| {
        mpris_ctx.player.do_send(PlaybackCommand::TogglePause);
        Ok(())
    });
    b.method("Stop", (), (), |_, mpris_ctx, ()| {
        mpris_ctx.player.do_send(PlaybackCommand::Stop);
        Ok(())
    });
    b.method("Play", (), (), |_, mpris_ctx, ()| {
        mpris_ctx.player.do_send(PlaybackCommand::Resume);
        Ok(())
    });
    b.method(
        "Seek",
        ("Offset",),
        (),
        |_, mpris_ctx, (offset,): (i64,)| {
            let duration = Duration::from_micros(offset.abs() as u64);
            let seek_direction = if offset > 0 {
                SeekDirection::Forward
            } else {
                SeekDirection::Backward
            };
            mpris_ctx
                .player
                .do_send(PlaybackCommand::SeekRelative(seek_direction, duration));
            Ok(())
        },
    );
    b.method(
        "SetPosition",
        ("TrackId", "Position"),
        (),
        |_, mpris_ctx, (_track_id, position): (dbus::Path, i64)| {
            let duration = Duration::from_micros(position.abs() as u64);
            mpris_ctx.player.do_send(PlaybackCommand::Seek(duration));
            Ok(())
        },
    );
    b.method(
        "OpenUri",
        ("Uri",),
        (),
        |_, _, (_offset,): (String,)| Ok(()),
    );

    let seeked_signal = b.signal::<(i64,), _>("Seeked", ("Position",)).msg_fn();

    let status_changed = b
        .property("PlaybackStatus")
        .get(|_, mpris_ctx| match mpris_ctx.state.read() {
            Ok(state) => {
                let status = PlaybackStatus::from_state(&state.state);
                Ok(status.to_string())
            }
            Err(err) => Err(MethodErr::failed(&err)),
        })
        .emits_changed_true()
        .changed_msg_fn();

    b.property("Rate").get(|_, _| Ok(1.0));

    let metadata_changed = b
        .property("Metadata")
        .get(|_, mpris_ctx| match mpris_ctx.state.read() {
            Ok(state) => Ok(state.construct_mpris_metadata()),
            Err(err) => Err(MethodErr::failed(&err)),
        })
        .emits_changed_true()
        .changed_msg_fn();

    let volume_changed = b
        .property("Volume")
        .get_async(|mut ctx, mpris_ctx| {
            let player = mpris_ctx.player.clone();
            async move {
                let result = match player.send(VolumeQueryRequest).await {
                    Ok(Ok(Some(volume))) => Ok(volume.cubic()),
                    Ok(Ok(None)) => Ok(0.0),
                    Ok(Err(err)) => Err(MethodErr::failed(&err)),
                    Err(err) => Err(MethodErr::failed(&err)),
                };
                ctx.reply(result)
            }
        })
        .set(|_, mpris_ctx, value| {
            let volume = Volume::from_cubic_clip(value);
            mpris_ctx.player.do_send(VolumeCommand::SetVolume(volume));
            Ok(Some(volume.cubic()))
        })
        .emits_changed_true()
        .changed_msg_fn();

    b.property("Position")
        .get(|_, mpris_ctx| match mpris_ctx.state.read() {
            Ok(state) => {
                let position = state
                    .state
                    .timing()
                    .map(|timing| timing.position)
                    .unwrap_or(Duration::ZERO);
                Ok(position.as_micros() as i64)
            }
            Err(err) => Err(MethodErr::failed(&err)),
        })
        .emits_changed_false();

    b.property("MinimumRate").get(|_, _| Ok(1.0));
    b.property("MaximumRate").get(|_, _| Ok(1.0));
    b.property("CanGoNext").get(|_, _| Ok(false));
    b.property("CanGoPrevious").get(|_, _| Ok(false));
    b.property("CanPlay").get(|_, _| Ok(true));
    b.property("CanPause").get(|_, _| Ok(true));
    b.property("CanSeek").get(|_, _| Ok(true));
    b.property("CanControl").get(|_, _| Ok(true));

    *callbacks = Some(DBusCallbacks {
        volume_changed,
        status_changed,
        seeked_signal,
        metadata_changed,
    });
}

#[derive(Debug, PartialEq, Eq)]
enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

impl Display for PlaybackStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaybackStatus::Playing => f.write_str("Playing"),
            PlaybackStatus::Paused => f.write_str("Paused"),
            PlaybackStatus::Stopped => f.write_str("Stopped"),
        }
    }
}

impl PlaybackStatus {
    fn from_state(playback_state: &PlaybackState) -> Self {
        let state = playback_state.state();
        match state {
            Some(State {
                is_paused: true, ..
            }) => PlaybackStatus::Paused,
            Some(_) => PlaybackStatus::Playing,
            None => PlaybackStatus::Stopped,
        }
    }
}

impl Handler<PlayerNotification> for MprisPlayer {
    type Result = ();

    fn handle(&mut self, msg: PlayerNotification, _ctx: &mut Self::Context) -> Self::Result {
        if let (Some(callbacks), Some(connection)) = (&self.dbus_callbacks, &self.connection) {
            match msg {
                PlayerNotification::VolumeChanged(volume) => {
                    let message = (callbacks.volume_changed)(
                        &dbus::Path::from("/org/mpris/MediaPlayer2").into_static(),
                        &volume.map(Volume::cubic).unwrap_or(0.0),
                    );
                    if let Some(message) = message {
                        let _ = connection.send(message);
                    }
                }
                PlayerNotification::StateChanged(update) => {
                    if let Ok(mut guard) = self.playback_state.write() {
                        let status_before = PlaybackStatus::from_state(&guard.state);
                        guard.state.set_state(update);
                        let status_after = PlaybackStatus::from_state(&guard.state);
                        if status_before != status_after {
                            let message = (callbacks.status_changed)(
                                &dbus::Path::from("/org/mpris/MediaPlayer2").into_static(),
                                &status_after.to_string(),
                            );
                            if let Some(message) = message {
                                let _ = connection.send(message);
                            }
                        }
                    }
                }
                PlayerNotification::DurationSet(duration) => {
                    if let Ok(mut guard) = self.playback_state.write() {
                        guard.state.set_duration(duration);

                        let message = (callbacks.metadata_changed)(
                            &dbus::Path::from("/org/mpris/MediaPlayer2").into_static(),
                            &(guard.construct_mpris_metadata(),),
                        );
                        if let Some(message) = message {
                            let _ = connection.send(message);
                        }
                    }
                }
                PlayerNotification::PositionSet { position, seeked } => {
                    if let Ok(mut guard) = self.playback_state.write() {
                        guard.state.set_position(position);
                    }
                    if seeked {
                        let message = (callbacks.seeked_signal)(
                            &dbus::Path::from("/org/mpris/MediaPlayer2").into_static(),
                            &(position.as_micros() as i64,),
                        );
                        let _ = connection.send(message);
                    }
                }
                PlayerNotification::MetadataChanged(metadata) => {
                    if let Ok(mut guard) = self.playback_state.write() {
                        guard.metadata = Some(metadata);

                        let message = (callbacks.metadata_changed)(
                            &dbus::Path::from("/org/mpris/MediaPlayer2").into_static(),
                            &(guard.construct_mpris_metadata(),),
                        );
                        if let Some(message) = message {
                            let _ = connection.send(message);
                        }
                    }
                }
                PlayerNotification::Eos => {}
            }
        }
    }
}
