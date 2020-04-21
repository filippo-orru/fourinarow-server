use super::msg::SrvMsgError;

use rand::{thread_rng, Rng};
use std::fmt;

pub const FIELD_SIZE: usize = 7;
pub const GAME_ID_LEN: usize = 4;

const VALID_GAME_ID_CHARS: &str = "ABCDEFGHJKLMNOPQRSTUXYZ";

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct GameId {
    inner: [char; GAME_ID_LEN],
}
impl GameId {
    pub fn generate(map: &[&GameId]) -> GameId {
        let mut ret = Self::generate_inner();

        while map.contains(&&ret) {
            ret = Self::generate_inner();
        }
        ret
    }
    fn generate_inner() -> GameId {
        let abc = VALID_GAME_ID_CHARS.chars().collect::<Vec<_>>();
        let mut rand_chars: [char; GAME_ID_LEN] = ['a'; GAME_ID_LEN];
        for rand_char in rand_chars.iter_mut() {
            *rand_char = abc[thread_rng().gen_range(0, VALID_GAME_ID_CHARS.len())];
        }
        GameId { inner: rand_chars }
    }
    pub fn parse(text: &str) -> Option<GameId> {
        let chars = text.chars().collect::<Vec<char>>();
        if chars.len() == GAME_ID_LEN {
            let mut inner = ['a'; GAME_ID_LEN];
            for (i, c) in chars.iter().enumerate() {
                inner[i] = *c;
            }
            Some(GameId { inner })
        } else {
            None
        }
    }
}
impl fmt::Display for GameId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::fmt::Write;
        for c in self.inner.iter() {
            f.write_char(*c)?;
        }

        fmt::Result::Ok(())
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
    pub winner: Option<WinnerInfo>,
}
impl GameInfo {
    pub fn new() -> Self {
        GameInfo {
            field: [[None; FIELD_SIZE]; FIELD_SIZE],
            turn: [Player::One, Player::Two][thread_rng().gen_range(0, 2)],
            winner: None,
        }
    }
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    pub fn check_win(&mut self) -> Option<Player> {
        let maybe_winner = self.check_win_internal();
        if let Some(winner) = maybe_winner {
            self.winner = Some(WinnerInfo {
                winner,
                requesting_rematch: None,
            });
        }
        maybe_winner
    }

    fn check_win_internal(&self) -> Option<Player> {
        const RANGE: isize = FIELD_SIZE as isize - 4;
        for r in (-RANGE)..=RANGE {
            let mut combo: (Option<Player>, usize) = (None, 0);
            for i in 0..FIELD_SIZE {
                let i_isize = i as isize;
                if i_isize + r < 0 || i_isize + r >= FIELD_SIZE as isize {
                    combo = (None, 0);
                    continue;
                }
                let column = (i_isize + r) as usize;
                let cell = self.field[column][i];
                if cell.is_none() {
                    combo = (None, 0);
                    continue;
                } else if cell != combo.0 {
                    combo = (cell, 0);
                }

                combo.1 += 1;
                if combo.1 >= 4 {
                    return cell;
                }
            }

            let mut combo: (Option<Player>, usize) = (None, 0);
            for i in (0..FIELD_SIZE - 1).rev() {
                let i_isize = i as isize;
                if i_isize + r < 0 || i_isize + r >= FIELD_SIZE as isize {
                    combo = (None, 0);
                    continue;
                }
                let real_y = FIELD_SIZE - 1 - i;
                let column = (i_isize + r) as usize;
                let cell = self.field[column][real_y];
                if cell.is_none() {
                    combo = (None, 0);
                    continue;
                } else if cell != combo.0 {
                    combo = (cell, 0);
                }

                combo.1 += 1;
                if combo.1 >= 4 {
                    return cell;
                }
            }
        }

        let mut x_combo: Vec<(Option<Player>, usize)> = vec![(None, 0); FIELD_SIZE];
        let mut combo: (Option<Player>, usize) = (None, 0);
        for column in &self.field {
            for (y, cell) in column.iter().enumerate() {
                let cell = *cell;
                if cell.is_none() {
                    combo = (None, 0);
                    x_combo[y] = (None, 0);
                    continue;
                } else if cell != combo.0 {
                    combo = (cell, 0);
                }
                combo.1 += 1;

                if combo.1 >= 4 {
                    return cell;
                }

                if cell != x_combo[y].0 {
                    x_combo[y] = (cell, 0);
                }
                x_combo[y].1 += 1;

                if x_combo[y].1 >= 4 {
                    return cell;
                }
            }
        }
        None
    }

    pub fn place_chip(
        &mut self,
        column: usize,
        player: Player,
    ) -> Result<Option<Player>, Option<SrvMsgError>> {
        if column >= self.field.len() {
            return Err(Some(SrvMsgError::InvalidColumn));
        }
        if player == self.turn {
            for i in (0..FIELD_SIZE).rev() {
                if self.field[column][i] == None {
                    self.field[column][i] = Some(self.turn);
                    self.turn = self.turn.other();
                    // self.print_field();
                    return Ok(self.check_win());
                }
            }
            Err(Some(SrvMsgError::InvalidColumn))
        } else {
            Err(Some(SrvMsgError::NotYourTurn))
        }
        // Err(None)
    }

    #[allow(dead_code)]
    fn print_field(&self) {
        for column in self.field.iter() {
            for cell in column {
                match cell {
                    None => print!("□"),
                    Some(Player::One) => print!("X"),
                    Some(Player::Two) => print!("O"),
                }
                print!(" ");
            }
            println!();
        }
    }
    // fn is_column_full(&self, column: usize) -> bool {
    //     !self.field[column].contains(&None)
    // }
}

pub struct WinnerInfo {
    pub winner: Player,
    pub requesting_rematch: Option<Player>,
}
