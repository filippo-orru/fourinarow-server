use super::lobby_mgr::LobbyKind;
use super::{
    connection_mgr::SessionToken,
    game_info::{GameId, GAME_ID_LEN},
};
use crate::api::users::user::UserId;
use actix::prelude::*;

#[derive(Debug, Clone)]
pub enum ReliablePacketIn {
    Ack(usize),                // Acknowledge the server's message with that id
    Msg(usize, PlayerMessage), // Actual message with id and content
}

impl ReliablePacketIn {
    pub fn parse(orig: &str) -> Result<ReliablePacketIn, ReliabilityError> {
        let uppercase: String = if let Some(u) = orig.split("::").next() {
            u.into()
        } else {
            return Err(ReliabilityError::InvalidFormat);
        };
        let parts: Vec<_> = orig.split("::").collect();
        return if parts.len() == 2 && uppercase == "ACK" {
            if let Ok(id) = parts[1].parse::<usize>() {
                Ok(ReliablePacketIn::Ack(id))
            } else {
                Err(ReliabilityError::InvalidFormat)
            }
        } else if parts.len() == 3 && uppercase == "MSG" {
            if let Ok(id) = parts[1].parse::<usize>() {
                if let Some(player_msg) = PlayerMessage::parse(parts[2]) {
                    Ok(ReliablePacketIn::Msg(id, player_msg))
                } else {
                    Err(ReliabilityError::InvalidContent)
                }
            } else {
                Err(ReliabilityError::InvalidFormat)
            }
        } else {
            Err(ReliabilityError::UnknownMessage)
        };
    }
}

#[derive(Debug, Clone)]
pub enum ReliablePacketOut {
    Ack(usize), // Acknowledge the client's message with that id
    Msg {
        id: usize,
        msg: ServerMessage,
        retry_count: usize,
    },
    Err(ReliabilityError),
}

impl Message for ReliablePacketOut {
    type Result = ();
}

impl ReliablePacketOut {
    pub fn serialize(self) -> String {
        use ReliablePacketOut::*;
        match self {
            Ack(id) => format!("ACK::{}", id),
            Msg {
                id,
                msg,
                retry_count: _,
            } => format!("MSG::{}::{}", id, msg.serialize()),
            Err(err) => format!("ERR::{}", err.serialize()),
        }
    }
}
#[derive(Debug, Clone)]
pub enum ReliabilityError {
    InvalidContent, // Message content could not be parsed
    InvalidFormat,  // ReliableMessage could not be parsed
    UnknownMessage, // Correct format but unknown keyword (ack, syn, msg)
                    //Unknown,        // all other errors
}
impl ReliabilityError {
    pub fn serialize(self) -> String {
        use ReliabilityError::*;
        match self {
            InvalidContent => "INVALID_CONTENT",
            InvalidFormat => "INVALID_FORMAT",
            UnknownMessage => "UNKNOWN_MESSAGE",
            //Unknown => "UNKNOWN",
        }
        .into()
    }
}

impl Into<ReliablePacketOut> for ReliabilityError {
    fn into(self) -> ReliablePacketOut {
        ReliablePacketOut::Err(self)
    }
}

pub struct HelloIn {
    pub protocol_version: usize,
    pub maybe_session_token: Option<SessionToken>,
}

impl HelloIn {
    pub fn parse(orig: &str) -> Option<Self> {
        //let uppercase = orig.to_uppercase();
        let parts: Vec<_> = orig.split("::").collect();
        if parts.len() == 3 && parts[0] == "HELLO" {
            if let Ok(protocol_version) = parts[1].parse() {
                let request_parts: Vec<_> = parts[2].split(":").collect();
                let maybe_session_token = if request_parts.len() == 1 && request_parts[0] == "NEW" {
                    None
                } else if request_parts.len() == 2 && request_parts[0] == "REQ" {
                    let session_token = request_parts[1].to_string();
                    if session_token.len() != 32 {
                        None
                    } else {
                        Some(session_token)
                    }
                } else {
                    None
                };
                return Some(HelloIn {
                    protocol_version,
                    maybe_session_token,
                });
            }
        }
        None
    }
}

pub enum HelloOut {
    Ok(SessionToken, bool), // Hello sessionToken and session_token.is_new
    OutDated,               // Sent when the client's version is too old
}
impl HelloOut {
    pub fn serialize(self) -> String {
        use HelloOut::*;
        match self {
            Ok(session_token, is_new) => {
                let is_new_str = if is_new { "NEW" } else { "FOUND" };
                format!("HELLO::{}::{}", is_new_str, session_token)
            }
            OutDated => "HELLO::OUTDATED".to_string(),
        }
    }
}

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
    CurrentServerState(usize, bool), // connected players, someone wants to play
    ChatMessage(bool, String, Option<String>), // is_global, message, sender_name
    ChatRead(bool),                  // is_global
}

impl ServerMessage {
    pub fn serialize(&self) -> String {
        use ServerMessage::*;
        match self.clone() {
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
            CurrentServerState(connected_players, player_waiting) => format!(
                "CURRENT_SERVER_STATE:{}:{}",
                connected_players, player_waiting
            ),
            ChatMessage(is_global, message, maybe_sender) => {
                let sender_name = if let Some(sender) = maybe_sender {
                    sender
                } else {
                    "".to_string()
                };
                let encoded_message = base64::encode_config(message, base64::STANDARD);
                format!("CHAT_MSG:{}:{}:{}", is_global, encoded_message, sender_name)
            }
            ChatRead(is_global) => format!("CHAT_READ:{}", is_global),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SrvMsgError {
    Internal,
    // InvalidMessage,
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
            // InvalidMessage => "InvalidMessage".to_owned(),
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
    ChatMessage(String),
    ChatRead,
}

impl PlayerMessage {
    pub fn parse(orig: &str) -> Option<PlayerMessage> {
        let s = orig.to_uppercase();
        if s.len() > 1000 {
            // chat messages might be long
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
        } else if s.starts_with("CHAT_MSG") {
            let split: Vec<&str> = orig.split(':').collect();
            if split.len() == 2 {
                if let Ok(Ok(decoded_msg)) =
                    base64::decode_config(split[1], base64::STANDARD).map(String::from_utf8)
                {
                    return Some(ChatMessage(decoded_msg));
                }
            }
        } else if s == "CHAT_READ" {
            return Some(ChatRead);
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
