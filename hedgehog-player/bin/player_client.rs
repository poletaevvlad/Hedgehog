use actix::Actor;
use hedgehog_player::{Ping, Player};
use std::io::{self, BufRead, Write};

#[actix::main]
async fn main() {
    let player_addr = Player.start();

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
            ("ping", _) => print!("{:?}", player_addr.send(Ping).await),
            (_, _) => print!("command unknown"),
        }

        print!("\n> ");
        io::stdout().flush().unwrap();
    }
}
