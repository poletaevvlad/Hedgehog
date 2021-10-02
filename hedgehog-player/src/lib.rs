use actix::prelude::*;

pub struct Player;

impl Actor for Player {
    type Context = Context<Self>;
}

#[derive(Debug, Message)]
#[rtype(result = "String")]
pub struct Ping;

impl Handler<Ping> for Player {
    type Result = String;

    fn handle(&mut self, _msg: Ping, _ctx: &mut Self::Context) -> Self::Result {
        "pong".to_string()
    }
}
