use super::game_info::{GameId, GAME_ID_LEN};
use actix::prelude::*;

#[derive(Debug, Clone, Copy)]
pub enum ServerMessage {
    PlaceChip(usize),
    ResetField,
    OpponentLeaving,
    OpponentJoining,
    LobbyResponse(GameId),
    GameStart(bool), // whether it's your or opponent's turn
    LobbyClosing,
    Okay,
    Error(Option<SrvMsgError>),
}

#[derive(Debug, Clone, Copy)]
pub enum SrvMsgError {
    Internal,
    InvalidMessage,
    LobbyNotFound,
    LobbyFull,
    InvalidColumn,
    NotInLobby,
    NotYourTurn,
    GameAlreadyStarted,
    GameNotStarted,
    GameNotOver,
    // GameAlreadyOver,
}

impl ServerMessage {
    pub fn serialize(self) -> String {
        use ServerMessage::*;
        match self {
            PlaceChip(row) => format!("PC:{}", row),
            ResetField => "RESET_FIELD".to_owned(),
            OpponentLeaving => "OPP_LEAVING".to_owned(),
            OpponentJoining => "OPP_JOINED".to_owned(),
            LobbyResponse(game_id) => format!("LOBBY_ID:{}", game_id),
            GameStart(your_turn) => format!("GAME_START:{}", if your_turn { "YOU" } else { "OPP" }),
            LobbyClosing => "LOBBY_CLOSING".to_owned(),
            Okay => "OKAY".to_owned(),
            Error(maybe_msg) => {
                if let Some(msg) = maybe_msg {
                    format!("ERROR:{}", msg.serialize())
                } else {
                    "ERROR".to_owned()
                }
            }
        }
    }
}

impl SrvMsgError {
    fn serialize(self) -> String {
        use SrvMsgError::*;
        match self {
            Internal => "Internal".to_owned(),
            InvalidMessage => "InvalidMessage".to_owned(),
            NotYourTurn => "NotYourTurn".to_owned(),
            NotInLobby => "NotInLobby".to_owned(),
            LobbyNotFound => "LobbyNotFound".to_owned(),
            LobbyFull => "LobbyFull".to_owned(),
            InvalidColumn => "InvalidColumn".to_owned(),
            GameNotStarted => "GameNotStarted".to_owned(),
            GameAlreadyStarted => "GameAlreadyStarted".to_owned(),
            GameNotOver => "GameNotOver".to_owned(),
            // GameAlreadyOver => "GameAlreadyOver".to_owned(),
        }
    }
}

impl From<bool> for ServerMessage {
    /// Maps a boolean to Okay / Error
    fn from(b: bool) -> Self {
        if b {
            ServerMessage::Okay
        } else {
            ServerMessage::Error(None)
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PlayerMessage {
    PlaceChip(usize),
    PlayAgainRequest,
    Leaving,
    LobbyRequest,
    LobbyJoin(GameId),
}

impl PlayerMessage {
    pub fn parse(s: &str) -> Option<PlayerMessage> {
        use PlayerMessage::*;
        if s.starts_with("PC:") && s.len() == 4 {
            if let Ok(row) = s[3..4].parse() {
                return Some(PlaceChip(row));
            }
        } else if s == "REQ_LOBBY" {
            return Some(LobbyRequest);
        // } else if s == "PlayerLeaving" {
        //     return Some(PlayerLeaving);
        } else if s.starts_with("JOIN_LOBBY:") && s.len() == 11 + GAME_ID_LEN {
            if let Some(id) = GameId::parse(&s[11..11 + GAME_ID_LEN]) {
                return Some(LobbyJoin(id));
            }
        } else if s == "LEAVE" {
            return Some(Leaving);
        } else if s == "PLAY_AGAIN" {
            return Some(PlayAgainRequest);
        }
        None
    }
}

impl Message for PlayerMessage {
    type Result = Result<(), ()>;
}
// impl Message for ServerMessageNamed {
//     type Result = Result<(), ()>; // whether action was successful or not
// }
impl Message for ServerMessage {
    type Result = Result<(), ()>; // whether action was successful or not
}
