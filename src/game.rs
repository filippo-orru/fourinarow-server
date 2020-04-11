use crate::lobby_mgr::LobbyMap;
use crate::msg::SrvMsgError;

use rand::{thread_rng, Rng};
use std::fmt;

pub const FIELD_SIZE: usize = 7;
pub const GAME_ID_LEN: usize = 4;

const VALID_GAME_ID_CHARS: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ";

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct GameId {
    inner: [char; 4],
}
impl GameId {
    pub fn generate(map: &LobbyMap) -> GameId {
        let mut ret = Self::generate_inner();

        while map.contains_key(&ret) {
            ret = Self::generate_inner();
        }
        ret
    }
    fn generate_inner() -> GameId {
        let abc = VALID_GAME_ID_CHARS.chars().collect::<Vec<_>>();
        let mut rands: [usize; GAME_ID_LEN] = [0; GAME_ID_LEN];
        for rand in rands.iter_mut() {
            *rand = thread_rng().gen_range(0, VALID_GAME_ID_CHARS.len());
        }
        GameId {
            inner: [abc[rands[0]], abc[rands[1]], abc[rands[2]], abc[rands[3]]],
        }
    }
    pub fn parse(text: &str) -> Option<GameId> {
        let chars = text.chars().collect::<Vec<char>>();
        if chars.len() == 4 {
            Some(GameId {
                inner: [chars[0], chars[1], chars[2], chars[3]],
            })
        } else {
            None
        }
    }
}
impl fmt::Display for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}{}",
            self.inner[0], self.inner[1], self.inner[2], self.inner[3]
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Player {
    One,
    Two,
}
impl Player {
    pub fn other(self) -> Player {
        if self == Player::One {
            Player::Two
        } else {
            Player::One
        }
    }

    pub fn select<T>(self, one: T, two: T) -> T {
        match self {
            Player::One => one,
            Player::Two => two,
        }
    }
}

/*pub enum GameState {
    // Idle,
    // WaitingInLobby(PlayerInfo), // my info
    // Playing(PlayingInfo),
    OnePlayer(Addr<ClientConnection>),
    TwoPlayers(GameInfo),
}
impl GameState {
    pub fn new() -> GameState {
        GameState::OnePlayer(addr)
    }
}*/

pub struct GameInfo {
    field: [[Option<Player>; FIELD_SIZE]; FIELD_SIZE],
    pub turn: Player,
}
impl GameInfo {
    pub fn new() -> Self {
        let turn = [Player::One, Player::Two][thread_rng().gen_range(0, 2)];
        println!("Created game. {:?} starts playing", turn);
        GameInfo {
            field: [[None; FIELD_SIZE]; FIELD_SIZE],
            turn,
        }
    }
    pub fn place_chip(&mut self, column: usize, player: Player) -> Result<(), Option<SrvMsgError>> {
        if column >= self.field.len() {
            return Err(Some(SrvMsgError::InvalidColumn));
        }
        if player == self.turn {
            if !self.is_column_full(column) {
                for i in (0..FIELD_SIZE).rev() {
                    if self.field[column][i] == None {
                        self.field[column][i] = Some(self.turn);
                        self.turn = self.turn.other();
                        return Ok(());
                    }
                }
            } else {
                return Err(Some(SrvMsgError::InvalidColumn));
            }
        } else {
            return Err(Some(SrvMsgError::NotYourTurn));
        }
        Err(None)
    }
    fn is_column_full(&self, column: usize) -> bool {
        !self.field[column].contains(&None)
    }
}
