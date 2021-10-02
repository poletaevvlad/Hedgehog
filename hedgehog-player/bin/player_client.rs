use actix::Actor;
use hedgehog_player::{PlaybackControll, Player};
use std::io::{self, BufRead, Write};

#[actix::main]
async fn main() {
    Player::initialize().unwrap();
    let player_addr = Player::new().start();

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
                    .send(PlaybackControll::Play(url.to_string()))
                    .await
            ),
            ("stop", _) => print!("{:?}", player_addr.send(PlaybackControll::Stop).await),
            (_, _) => print!("command unknown"),
        }

        print!("\n> ");
        io::stdout().flush().unwrap();
    }
}
