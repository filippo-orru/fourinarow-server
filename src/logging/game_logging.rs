use actix::Message;
use mongodb::bson::oid::ObjectId;

#[derive(Clone)]
pub struct GameOId(ObjectId);

impl GameOId {
    pub fn new() -> Self {
        GameOId(ObjectId::new())
    }
}
pub enum GameEndReason {
    Regular,
    PlayerLeft,
    PlayerDisconnected,
}

pub enum GameLogEvent {
    StartGame { id: GameOId, ranked: bool },
    EndGame { id: GameOId, reason: GameEndReason },
}

impl Message for GameLogEvent {
    type Result = ();
}
