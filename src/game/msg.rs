use super::game_info::{GameId, GAME_ID_LEN};
use super::lobby_mgr::LobbyKind;
use actix::prelude::*;

#[derive(Debug, Clone, Copy)]
pub enum ServerMessage {
    PlaceChip(usize),
    OpponentLeaving,
    OpponentJoining,
    LobbyResponse(GameId),
    GameStart(bool), // true if recipient goes first
    GameOver(bool),  // true if recipient won
    LobbyClosing,
    Okay,
    Error(Option<SrvMsgError>),
}
impl ServerMessage {
    pub fn serialize(self) -> String {
        use ServerMessage::*;
        match self {
            PlaceChip(row) => format!("PC:{}", row),
            OpponentLeaving => "OPP_LEAVING".to_owned(),
            OpponentJoining => "OPP_JOINED".to_owned(),
            LobbyResponse(game_id) => format!("LOBBY_ID:{}", game_id),
            GameStart(your_turn) => format!("GAME_START:{}", if your_turn { "YOU" } else { "OPP" }),
            GameOver(you_win) => format!("GAME_OVER:{}", if you_win { "YOU" } else { "OPP" }),
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

#[derive(Debug, Clone, Copy)]
pub enum SrvMsgError {
    Internal,
    InvalidMessage,
    LobbyNotFound,
    LobbyFull,
    InvalidColumn,
    NotInLobby,
    NotYourTurn,
    AlreadyInLobby,
    GameNotStarted,
    GameNotOver,
    IncorrectCredentials,
    AlreadyPlaying,
    // GameAlreadyOver,
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
            AlreadyInLobby => "AlreadyInLobby".to_owned(),
            GameNotOver => "GameNotOver".to_owned(),
            IncorrectCredentials => "IncorrectCredentials".to_owned(),
            AlreadyPlaying => "AlreadyPlaying".to_owned(),
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

#[derive(Debug, Clone)]
pub enum PlayerMessage {
    PlaceChip(usize),
    PlayAgainRequest,
    Leaving,
    LobbyRequest(LobbyKind),
    LobbyJoin(GameId),
    Login(String, String),
}

impl PlayerMessage {
    pub fn parse(s: &str) -> Option<PlayerMessage> {
        if s.len() > 200 {
            return None;
        }
        use PlayerMessage::*;
        if s.starts_with("PC:") && s.len() == 4 {
            if let Ok(row) = s[3..4].parse() {
                return Some(PlaceChip(row));
            }
        } else if s == "REQ_LOBBY" {
            return Some(LobbyRequest(LobbyKind::Private));
        } else if s == "REQ_WW" {
            return Some(LobbyRequest(LobbyKind::Public));
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
        } else if s.starts_with("LOGIN:") {
            let split: Vec<&str> = s["LOGIN:".len()..].split('#').collect();
            if split.len() == 2 {
                return Some(Login(split[0].to_owned(), split[1].to_owned()));
            }
        }
        None
    }
}

impl<'a> Message for PlayerMessage {
    type Result = Result<(), ()>;
}
// impl Message for ServerMessageNamed {
//     type Result = Result<(), ()>; // whether action was successful or not
// }
impl Message for ServerMessage {
    type Result = Result<(), ()>; // whether action was successful or not
}
