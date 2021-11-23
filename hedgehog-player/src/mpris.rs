use crate::volume::Volume;
use crate::{
    ActorCommand, PlaybackCommand, Player, PlayerNotification, SeekDirection, VolumeCommand,
};
use actix::fut::wrap_future;
use actix::prelude::*;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus_crossroads::{Crossroads, IfaceBuilder};
use dbus_tokio::connection;
use std::collections::HashMap;
use std::process;
use std::time::Duration;

pub struct MprisPlayer {
    player: Addr<Player>,
}

impl MprisPlayer {
    pub fn new(player: Addr<Player>) -> Self {
        MprisPlayer { player }
    }
}

impl Actor for MprisPlayer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        let player_address = self.player.clone();
        ctx.spawn(wrap_future(async {
            let (resource, connection) = connection::new_session_sync().unwrap();
            let arbiter = Arbiter::current();

            arbiter.spawn(async {
                resource.await;
            });

            let pid = process::id();
            let name = format!("org.mpris.MediaPlayer2.hedgehog.instance{}", pid);
            let name_request = connection.request_name(name, false, true, false);
            if name_request.await.is_err() {
                return;
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
                b.property("Identity")
                    .get(|_, _| Ok("Hedgehog podcast player".to_string()));
                b.property("SupportedUriSchemes")
                    .get(|_, _| Ok(Vec::<String>::new()));
                b.property("SupportedMimeTypes")
                    .get(|_, _| Ok(Vec::<String>::new()));
            });

            let context = MpirsContext {
                player: player_address,
            };

            let player_iface = cr.register("org.mpris.MediaPlayer2.Player", build_player_interface);
            cr.insert("/org/mpris/MediaPlayer2", &[iface, player_iface], context);
            connection.start_receive(
                MatchRule::new_method_call(),
                Box::new(move |msg, conn| {
                    let _ = cr.handle_message(msg, conn);
                    true
                }),
            );
        }));

        self.player
            .do_send(ActorCommand::Subscribe(ctx.address().recipient()));
    }
}

impl Handler<PlayerNotification> for MprisPlayer {
    type Result = ();

    fn handle(&mut self, _msg: PlayerNotification, _ctx: &mut Self::Context) -> Self::Result {}
}

struct MpirsContext {
    player: Addr<Player>,
}

fn build_player_interface(b: &mut IfaceBuilder<MpirsContext>) {
    b.method("Next", (), (), |_, _, ()| Ok(()));
    b.method("Previous", (), (), |_, _, ()| Ok(()));
    b.method("Pause", (), (), |_, mpirs_ctx, ()| {
        mpirs_ctx.player.do_send(PlaybackCommand::Pause);
        Ok(())
    });
    b.method("PlayPause", (), (), |_, mpirs_ctx, ()| {
        mpirs_ctx.player.do_send(PlaybackCommand::TogglePause);
        Ok(())
    });
    b.method("Stop", (), (), |_, mpirs_ctx, ()| {
        mpirs_ctx.player.do_send(PlaybackCommand::Stop);
        Ok(())
    });
    b.method("Play", (), (), |_, mpirs_ctx, ()| {
        mpirs_ctx.player.do_send(PlaybackCommand::Resume);
        Ok(())
    });
    b.method("Seek", ("x",), (), |_, mpirs_ctx, (offset,): (i64,)| {
        let duration = Duration::from_micros(offset.abs() as u64);
        let seek_direction = if offset > 0 {
            SeekDirection::Forward
        } else {
            SeekDirection::Backward
        };
        mpirs_ctx
            .player
            .do_send(PlaybackCommand::SeekRelative(duration, seek_direction));
        Ok(())
    });
    b.method(
        "SetPosition",
        ("o", "x"),
        (),
        |_, mpirs_ctx, (_track_id, position): (dbus::Path, i64)| {
            let duration = Duration::from_micros(position.abs() as u64);
            mpirs_ctx.player.do_send(PlaybackCommand::Seek(duration));
            Ok(())
        },
    );
    b.method("OpenUri", ("s",), (), |_, _, (_offset,): (dbus::Path,)| {
        Ok(())
    });

    b.signal::<(i64,), _>("Seeked", ("x",));

    b.property("PlaybackStatus")
        .get(|_, _| Ok("Stopped".to_string()));
    b.property("Rate").get(|_, _| Ok(1.0));
    b.property("Metadata")
        .get(|_, _| Ok(HashMap::<String, dbus::arg::Variant<String>>::new()));
    b.property("Volume")
        .get(|_, _| Ok(1.0))
        .set(|_, mpirs_ctx, value| {
            let volume = Volume::from_cubic_clip(value);
            mpirs_ctx.player.do_send(VolumeCommand::SetVolume(volume));
            Ok(Some(volume.cubic()))
        });
    b.property("Position").get(|_, _| Ok(1));
    b.property("MinimumRate").get(|_, _| Ok(1.0));
    b.property("MaximumRate").get(|_, _| Ok(1.0));
    b.property("CanGoNext").get(|_, _| Ok(false));
    b.property("CanGoPrevious").get(|_, _| Ok(false));
    b.property("CanPlay").get(|_, _| Ok(false));
    b.property("CanPause").get(|_, _| Ok(false));
    b.property("CanControl").get(|_, _| Ok(true));
}
