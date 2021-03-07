mod game_logging;
mod lobby_logging;

pub use self::game_logging::*;
pub use self::lobby_logging::*;

use actix::{Actor, Context, Handler};

pub struct Logger;

impl Logger {
    pub fn new() -> Self {
        Logger
    }
}

impl Actor for Logger {
    type Context = Context<Self>;
}

impl Handler<GameLogEvent> for Logger {
    type Result = ();

    fn handle(&mut self, msg: GameLogEvent, _: &mut Self::Context) -> Self::Result {
        todo!()
    }
}
impl Handler<LobbyLogEvent> for Logger {
    type Result = ();

    fn handle(&mut self, msg: LobbyLogEvent, _: &mut Self::Context) -> Self::Result {
        todo!()
    }
}
