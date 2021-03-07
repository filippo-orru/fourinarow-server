use actix::Message;
use mongodb::bson::oid::ObjectId;

#[derive(Clone)]
pub struct LobbyId(ObjectId);

impl LobbyId {
    pub fn new() -> Self {
        LobbyId(ObjectId::new())
    }
}

pub enum LobbyCloseReason {
    Cancel,                        // Player waited but noone joined -> cancelled
    Success { games_played: u32 }, // Lobby had at least 1 game played
}

pub enum LobbyLogEvent {
    LobbyCreated {
        id: LobbyId,
    },
    LobbyClosed {
        id: LobbyId,
        // reason: LobbyCloseReason,
    },
}

impl Message for LobbyLogEvent {
    type Result = ();
}
