use super::game_info::{GameId, GAME_ID_LEN};
use super::lobby_mgr::LobbyKind;
use crate::api::users::user::UserId;
use actix::prelude::*;

#[derive(Debug, Clone)]
pub enum ServerMessage {
    PlaceChip(usize),
    OpponentLeaving,
    OpponentJoining,
    LobbyResponse(GameId),
    /// ( bool: true if recipient goes first,
    ///   Option<String>: Set if opponent is logged in
    GameStart(bool, Option<String>),
    GameOver(bool), // true if recipient won
    LobbyClosing,
    Okay,
    Pong,
    Error(Option<SrvMsgError>),
    BattleReq(UserId, GameId),
}
impl ServerMessage {
    pub fn serialize(self) -> String {
        use ServerMessage::*;
        match self {
            PlaceChip(row) => format!("PC:{}", row),
            OpponentLeaving => "OPP_LEAVING".to_owned(),
            OpponentJoining => "OPP_JOINED".to_owned(),
            LobbyResponse(game_id) => format!("LOBBY_ID:{}", game_id),
            GameStart(your_turn, maybe_username) => format!(
                "GAME_START:{}{}",
                if your_turn { "YOU" } else { "OPP" },
                if let Some(username) = maybe_username {
                    format!(":{}", username)
                } else {
                    "".to_owned()
                }
            ),
            GameOver(you_win) => format!("GAME_OVER:{}", if you_win { "YOU" } else { "OPP" }),
            LobbyClosing => "LOBBY_CLOSING".to_owned(),
            Okay => "OKAY".to_owned(),
            Pong => "PONG".to_owned(),
            Error(maybe_msg) => {
                if let Some(msg) = maybe_msg {
                    format!("ERROR:{}", msg.serialize())
                } else {
                    "ERROR".to_owned()
                }
            }
            BattleReq(requesting_id, lobby_id) => {
                format!("BATTLE_REQ:{}:{}", requesting_id, lobby_id)
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
    NotLoggedIn,
    UserNotPlaying,
    NoSuchUser,
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
            NotLoggedIn => "NotLoggedIn".to_owned(),
            UserNotPlaying => "UserNotPlaying".to_owned(),
            NoSuchUser => "NoSuchUser".to_owned(),
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
    Ping,
    LobbyRequest(LobbyKind),
    LobbyJoin(GameId),
    Login(String, String),
    BattleReq(UserId),
}

impl PlayerMessage {
    pub fn parse(orig: &str) -> Option<PlayerMessage> {
        let s = orig.to_uppercase();
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
        } else if s == "PING" {
            return Some(Ping);
        } else if s == "PLAY_AGAIN" {
            return Some(PlayAgainRequest);
        } else if s.starts_with("LOGIN:") {
            let split: Vec<&str> = orig.split(':').collect();
            if split.len() == 3 {
                return Some(Login(split[1].to_owned(), split[2].to_owned()));
            }
        } else if s.starts_with("BATTLE_REQ") {
            let split: Vec<&str> = orig.split(':').collect();
            if split.len() == 2 {
                if let Ok(user_id) = UserId::from_str(&split[1]) {
                    return Some(BattleReq(user_id));
                }
                // Err(e) => println!("Error: invalid battlereq userid ({})", e),
                // }
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
