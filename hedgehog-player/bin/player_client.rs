use actix::prelude::*;
use hedgehog_player::volume::{Volume, VolumeCommand};
use hedgehog_player::{
    ActorCommand, PlaybackCommand, Player, PlayerErrorNotification, PlayerNotification,
    SeekDirection,
};
use std::io::{self, BufRead, Write};
use std::time::Duration;

struct NotificationListener;

impl Actor for NotificationListener {
    type Context = Context<Self>;
}

impl Handler<PlayerNotification> for NotificationListener {
    type Result = ();

    fn handle(&mut self, msg: PlayerNotification, _ctx: &mut Self::Context) -> Self::Result {
        println!("@ {:?}", msg);
    }
}

impl Handler<PlayerErrorNotification> for NotificationListener {
    type Result = ();

    fn handle(&mut self, msg: PlayerErrorNotification, _ctx: &mut Self::Context) -> Self::Result {
        println!("ERR {:?}", msg.0);
    }
}

#[actix::main]
async fn main() {
    Player::initialize().unwrap();
    let arbiter = Arbiter::new();
    let handle = arbiter.handle();

    let player = Player::init().unwrap();
    let player_addr = Player::start_in_arbiter(&handle, move |_| player);

    let notification_listener =
        NotificationListener::start_in_arbiter(&handle, |_| NotificationListener);
    player_addr
        .send(ActorCommand::Subscribe(
            notification_listener.clone().recipient(),
        ))
        .await
        .unwrap();
    player_addr
        .send(ActorCommand::SubscribeErrors(
            notification_listener.recipient(),
        ))
        .await
        .unwrap();

    let stdin = io::stdin();
    print!("> ");
    io::stdout().flush().unwrap();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let (command, attr) = line.split_once(' ').unwrap_or((&line, ""));
        if command.is_empty() {
            print!("> ");
            io::stdout().flush().unwrap();
            continue;
        }
        print!("< ");

        match (command, attr) {
            ("play", url) => print!(
                "{:?}",
                player_addr
                    .send(PlaybackCommand::Play(url.to_string(), Duration::ZERO, None))
                    .await
            ),
            ("stop", _) => print!("{:?}", player_addr.send(PlaybackCommand::Stop).await),
            ("pause", _) => print!("{:?}", player_addr.send(PlaybackCommand::Pause).await),
            ("resume", _) => print!("{:?}", player_addr.send(PlaybackCommand::Resume).await),
            ("mute", _) => print!("{:?}", player_addr.send(VolumeCommand::Mute).await),
            ("unmute", _) => print!("{:?}", player_addr.send(VolumeCommand::Unmute).await),
            ("toggle_mute", _) => print!("{:?}", player_addr.send(VolumeCommand::ToggleMute).await),
            ("seek", duration) => match duration.parse().map(Duration::from_secs) {
                Ok(duration) => print!(
                    "{:?}",
                    player_addr.send(PlaybackCommand::Seek(duration)).await
                ),
                Err(error) => print!("{:?}", error),
            },
            ("seek_fwd", duration) | ("seek_bck", duration) => {
                match duration.parse().map(Duration::from_secs) {
                    Ok(duration) => print!(
                        "{:?}",
                        player_addr
                            .send(PlaybackCommand::SeekRelative(
                                if command == "seek_fwd" {
                                    SeekDirection::Forward
                                } else {
                                    SeekDirection::Backward
                                },
                                duration
                            ))
                            .await
                    ),
                    Err(error) => print!("{:?}", error),
                }
            }
            ("set_volume", volume) => match volume.parse().map(Volume::from_cubic) {
                Ok(volume) => print!(
                    "{:?}",
                    player_addr.send(VolumeCommand::SetVolume(volume)).await
                ),
                Err(error) => print!("{:?}", error),
            },
            ("adj_volume", delta) => match delta.parse() {
                Ok(delta) => print!(
                    "{:?}",
                    player_addr.send(VolumeCommand::AdjustVolume(delta)).await
                ),
                Err(error) => print!("{:?}", error),
            },
            (_, _) => print!("command unknown"),
        }

        print!("\n> ");
        io::stdout().flush().unwrap();
    }
}
