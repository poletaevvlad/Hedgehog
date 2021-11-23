use crate::{ActorCommand, Player, PlayerNotification};
use actix::fut::wrap_future;
use actix::prelude::*;
use dbus::channel::MatchingReceiver;
use dbus::message::MatchRule;
use dbus_crossroads::Crossroads;
use dbus_tokio::connection;
use std::collections::HashMap;
use std::process;

pub struct MpirsPlayer {
    player: Addr<Player>,
}

impl MpirsPlayer {
    pub fn new(player: Addr<Player>) -> Self {
        MpirsPlayer { player }
    }
}

impl Actor for MpirsPlayer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
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

            let player_iface = cr.register("org.mpris.MediaPlayer2.Player", |b| {
                b.method("Next", (), (), |_, _, ()| Ok(()));
                b.method("Previous", (), (), |_, _, ()| Ok(()));
                b.method("Pause", (), (), |_, _, ()| Ok(()));
                b.method("PlayPause", (), (), |_, _, ()| Ok(()));
                b.method("Stop", (), (), |_, _, ()| Ok(()));
                b.method("Play", (), (), |_, _, ()| Ok(()));
                b.method("Seek", ("x",), (), |_, _, (_offset,): (i64,)| Ok(()));
                b.method(
                    "SetPosition",
                    ("o", "x"),
                    (),
                    |_, _, (_track_id, _position): (i64, i64)| Ok(()),
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
                b.property("Volume").get(|_, _| Ok(1.0));
                b.property("Position").get(|_, _| Ok(1));
                b.property("MinimumRate").get(|_, _| Ok(1.0));
                b.property("MaximumRate").get(|_, _| Ok(1.0));
                b.property("CanGoNext").get(|_, _| Ok(false));
                b.property("CanGoPrevious").get(|_, _| Ok(false));
                b.property("CanPlay").get(|_, _| Ok(false));
                b.property("CanPause").get(|_, _| Ok(false));
                b.property("CanControl").get(|_, _| Ok(true));
            });

            cr.insert("/org/mpris/MediaPlayer2", &[iface, player_iface], ());
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

impl Handler<PlayerNotification> for MpirsPlayer {
    type Result = ();

    fn handle(&mut self, _msg: PlayerNotification, _ctx: &mut Self::Context) -> Self::Result {}
}
