use super::{super::ApiError, user::*};
use crate::game::lobby_mgr::LobbyManager;
// use crate::game::msg::SrvMsgError;
use actix::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::io;

const USERS_PATH: &str = "data/users.json";
const GAMES_PATH: &str = "data/games.json";
const SR_PER_WIN: i32 = 25;

/// in seconds
const DB_SAVE_INTERVAL: u64 = 5 * 60; // = 5 minutes

pub struct UserManager {
    users: HashMap<UserId, User>,
    games: Vec<PlayedGameInfo>,
    lobby_mgr_state: BacklinkState,
}
impl UserManager {
    pub fn new() -> UserManager {
        UserManager {
            users: HashMap::new(),
            games: Vec::new(),
            lobby_mgr_state: BacklinkState::Waiting,
        }
    }

    fn load_db_file<T>(&mut self, path: &str) -> io::Result<T>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        if let Ok(file) = std::fs::File::open(path) {
            serde_json::from_reader(file).map_err(From::from)
        } else {
            std::fs::File::create(path).map(|_| T::default())
        }
    }

    fn load_db_files(&mut self) -> io::Result<()> {
        self.users = self.load_db_file(USERS_PATH)?;
        self.games = self.load_db_file(GAMES_PATH)?;
        Ok(())
    }

    fn save_db(&mut self) {
        if let Err(e) = self.save_db_internal() {
            println!("Failed to save users: {:?}.", e);
        }
    }
    fn save_db_internal(&mut self) -> io::Result<()> {
        serde_json::to_writer(std::fs::File::create(USERS_PATH)?, &self.users)?;
        serde_json::to_writer(std::fs::File::create(GAMES_PATH)?, &self.games)?;
        Ok(())
    }
}

enum BacklinkState {
    Waiting,
    Linked(Addr<LobbyManager>),
}

impl Actor for UserManager {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        if let Err(e) = self.load_db_files() {
            println!("Failed to load database: {:?}.", e);
        }
        ctx.run_interval(
            std::time::Duration::from_secs(DB_SAVE_INTERVAL),
            |act, _| act.save_db(),
        );
    }

    fn stopping(&mut self, _ctx: &mut Self::Context) -> Running {
        self.save_db();
        Running::Stop
    }
}

#[derive(Deserialize)]
pub struct UserInfoPayload {
    username: String,
    password: String,
}
// impl UserInfoPayload {
//     pub fn new(username: String, password: String) -> UserInfoPayload {
//         UserInfoPayload { username, password }
//     }
// }

pub mod msg {
    use super::*;
    use crate::game::msg::SrvMsgError;

    pub struct Register(pub UserInfoPayload);
    impl Message for Register {
        type Result = Result<UserId, ApiError>;
    }
    impl Handler<Register> for UserManager {
        type Result = Result<UserId, ApiError>;

        fn handle(&mut self, msg: Register, _ctx: &mut Self::Context) -> Self::Result {
            if !User::check_password(&msg.0.password) {
                Err(ApiError::PasswordInsufficient)
            } else if self.users.values().any(|u| u.username == msg.0.username) {
                Err(ApiError::UsernameInUse)
            } else {
                let mut user = User::new(msg.0.username, msg.0.password);
                while self.users.contains_key(&user.id) {
                    user.gen_new_id();
                }
                // println!("new userid: {}", user.id);
                self.users.insert(user.id, user.clone());
                Ok(user.id)
            }
        }
    }
    pub struct Login(pub UserInfoPayload);
    impl Message for Login {
        type Result = Result<UserId, ApiError>;
    }
    impl Handler<Login> for UserManager {
        type Result = Result<UserId, ApiError>;

        fn handle(&mut self, msg: Login, _ctx: &mut Self::Context) -> Self::Result {
            self.users
                .values()
                .find_map(|u| {
                    if u.username == msg.0.username && u.password.matches(&msg.0.password) {
                        Some(u.id)
                    } else {
                        None
                    }
                })
                .ok_or(ApiError::IncorrectCredentials)
        }
    }

    pub struct StartPlaying(pub String, pub String);
    impl Message for StartPlaying {
        type Result = Result<UserId, SrvMsgError>;
    }
    impl Handler<StartPlaying> for UserManager {
        type Result = Result<UserId, SrvMsgError>;
        fn handle(&mut self, msg: StartPlaying, _ctx: &mut Self::Context) -> Self::Result {
            if let Some(user) = self
                .users
                .values_mut()
                .find(|user| user.username == msg.0 && user.password.matches(&msg.1))
            {
                if user.playing {
                    return Err(SrvMsgError::AlreadyLoggedIn);
                } else {
                    user.playing = true;
                }
                Ok(user.id)
            } else {
                Err(SrvMsgError::IncorrectCredentials)
            }
        }
    }

    pub enum IntUserMgrMsg {
        Backlink(Addr<LobbyManager>),
        Game(GameMsg),
        // StartPlaying(String, String),
        StopPlaying(UserId),
    }
    pub enum GameMsg {
        PlayedGame(PlayedGameInfo),
    }

    impl Message for IntUserMgrMsg {
        type Result = ();
    }
    impl Handler<IntUserMgrMsg> for UserManager {
        type Result = ();
        fn handle(&mut self, msg: IntUserMgrMsg, _ctx: &mut Self::Context) -> Self::Result {
            use GameMsg::*;
            use IntUserMgrMsg::*;
            match msg {
                Backlink(lobby_mgr) => self.lobby_mgr_state = BacklinkState::Linked(lobby_mgr),
                Game(game_msg) => match game_msg {
                    PlayedGame(game_info) => {
                        if let Some(user) = self.users.get_mut(&game_info.one) {
                            let sr = &mut user.game_info.skill_rating;
                            if *sr >= SR_PER_WIN {
                                *sr += if game_info.one_won {
                                    SR_PER_WIN
                                } else {
                                    -SR_PER_WIN
                                };
                            }
                        }
                        if let Some(user) = self.users.get_mut(&game_info.two) {
                            {
                                let sr = &mut user.game_info.skill_rating;
                                if *sr >= SR_PER_WIN {
                                    *sr += if !game_info.one_won {
                                        SR_PER_WIN
                                    } else {
                                        -SR_PER_WIN
                                    };
                                }
                            }
                            self.games.push(game_info);
                        }
                    }
                },
                // StartPlaying(id) => {
                //     if let Some(user) = self.users.get_mut(&id) {
                //         user.playing = false;
                //     }
                // }
                StopPlaying(id) => {
                    if let Some(user) = self.users.get_mut(&id) {
                        user.playing = false;
                    }
                }
            }
        }
    }

    pub struct GetUsers;
    impl Message for GetUsers {
        type Result = Option<Vec<User>>;
    }

    impl Handler<GetUsers> for UserManager {
        type Result = Option<Vec<User>>;
        fn handle(&mut self, _msg: GetUsers, _ctx: &mut Self::Context) -> Self::Result {
            Some(self.users.values().cloned().collect())
        }
    }
}
