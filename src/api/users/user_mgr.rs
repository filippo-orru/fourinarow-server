use super::{super::ApiError, user::*};
use crate::game::lobby_mgr::{self, LobbyManager};
use crate::game::msg::*;
use crate::{database::DatabaseManager, game::client_adapter::ClientAdapter};

use actix::*;
use serde::Deserialize;
use std::sync::Arc;

const SR_PER_WIN: i32 = 25;

pub struct UserManager {
    db: Arc<DatabaseManager>,
    lobby_mgr_state: BacklinkState,
}
impl UserManager {
    pub fn new(db: Arc<DatabaseManager>) -> UserManager {
        UserManager {
            db,
            lobby_mgr_state: BacklinkState::Waiting,
        }
    }
}

enum BacklinkState {
    Waiting,
    Linked(Addr<LobbyManager>),
}

impl Actor for UserManager {
    type Context = Context<Self>;
}

#[derive(Deserialize)]
pub struct UserAuth {
    pub username: String,
    pub password: String,
}
impl UserAuth {
    pub fn new(username: String, password: String) -> UserAuth {
        UserAuth { username, password }
    }
}

pub mod msg {

    use super::*;
    use crate::game::msg::SrvMsgError;

    pub struct Register(pub UserAuth);
    impl Message for Register {
        type Result = Result<UserId, ApiError>;
    }
    impl Handler<Register> for UserManager {
        type Result = Result<UserId, ApiError>;

        fn handle(&mut self, msg: Register, _ctx: &mut Self::Context) -> Self::Result {
            if !BackendUser::check_password(&msg.0.password) {
                Err(ApiError::PasswordInsufficient)
            } else if self
                .db
                .users
                .get_username(&msg.0.username, &self.db.friend_requests)
                .is_some()
            {
                Err(ApiError::UsernameInUse)
            } else {
                let mut user = BackendUser::new(msg.0.username, msg.0.password);
                while self
                    .db
                    .users
                    .get_id(&user.id, &self.db.friend_requests)
                    .is_some()
                {
                    user.gen_new_id();
                }
                // println!("new userid: {}", user.id);
                self.db.users.insert(user.clone());
                Ok(user.id)
            }
        }
    }
    pub struct Login(pub UserAuth);
    impl Message for Login {
        type Result = Result<UserId, ApiError>;
    }
    impl Handler<Login> for UserManager {
        type Result = Result<UserId, ApiError>;

        fn handle(&mut self, msg: Login, _ctx: &mut Self::Context) -> Self::Result {
            self.db
                .users
                .get_auth(msg.0, &self.db.friend_requests)
                .map(|user| user.id)
                .ok_or(ApiError::IncorrectCredentials)
        }
    }

    pub struct StartPlaying {
        pub username: String,
        pub password: String,
        pub addr: Addr<ClientAdapter>,
    }
    impl Message for StartPlaying {
        type Result = Result<PublicUserMe, SrvMsgError>;
    }
    impl Handler<StartPlaying> for UserManager {
        type Result = Result<PublicUserMe, SrvMsgError>;
        fn handle(&mut self, msg: StartPlaying, _ctx: &mut Self::Context) -> Self::Result {
            if let Some(mut user) = self.db.users.get_auth(
                UserAuth::new(msg.username, msg.password),
                &self.db.friend_requests,
            ) {
                if user.playing.is_some() {
                    return Err(SrvMsgError::AlreadyPlaying);
                } else {
                    user.playing = Some(msg.addr);
                    self.db.users.update(user.clone());
                }
                Ok(PublicUserMe::from(user.clone(), &self.db))
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
                        let mut found = false;
                        if let Some(mut winner) = self
                            .db
                            .users
                            .get_id(&game_info.winner, &self.db.friend_requests)
                        {
                            winner.game_info.skill_rating += SR_PER_WIN;
                            self.db.users.update(winner);
                            found = true;
                        }
                        if let Some(mut loser) = self
                            .db
                            .users
                            .get_id(&game_info.loser, &self.db.friend_requests)
                        {
                            if found {
                                loser.game_info.skill_rating -= SR_PER_WIN;
                                self.db.users.update(loser);
                                self.db.games.insert(game_info);
                            }
                        } else if found {
                            if let Some(mut winner) = self
                                .db
                                .users
                                .get_id(&game_info.winner, &self.db.friend_requests)
                            {
                                winner.game_info.skill_rating -= SR_PER_WIN;
                                self.db.users.update(winner);
                            }
                        }
                    }
                },
                // StartPlaying(id) => {
                //     if let Some(user) = self.users.get_mut(&id) {
                //         user.playing = false;
                //     }
                // }
                StopPlaying(id) => {
                    if let Some(mut user) = self.db.users.get_id(&id, &self.db.friend_requests) {
                        user.playing = None;
                        self.db.users.update(user);
                    }
                }
            }
        }
    }

    pub struct SearchUsers(pub String);
    impl Message for SearchUsers {
        type Result = Option<Vec<PublicUserOther>>;
    }

    impl Handler<SearchUsers> for UserManager {
        type Result = Option<Vec<PublicUserOther>>;
        fn handle(&mut self, msg: SearchUsers, _ctx: &mut Self::Context) -> Self::Result {
            let query = (&msg.0).to_lowercase();
            Some(
                self.db
                    .users
                    .query(&query, &self.db.friend_requests)
                    .iter()
                    .map(|user| PublicUserOther::from(user.clone()))
                    .collect(),
            )
        }
    }

    pub struct GetUserMe(pub UserAuth);
    impl Message for GetUserMe {
        type Result = Option<PublicUserMe>;
    }

    impl Handler<GetUserMe> for UserManager {
        type Result = Option<PublicUserMe>;
        fn handle(&mut self, msg: GetUserMe, _ctx: &mut Self::Context) -> Self::Result {
            self.db
                .users
                .get_auth(msg.0, &self.db.friend_requests)
                .map(|user| PublicUserMe::from(user, &self.db))
        }
    }

    pub struct GetUserOther(pub UserId);
    impl Message for GetUserOther {
        type Result = Option<PublicUserOther>;
    }

    impl Handler<GetUserOther> for UserManager {
        type Result = Option<PublicUserOther>;
        fn handle(&mut self, msg: GetUserOther, _ctx: &mut Self::Context) -> Self::Result {
            self.db
                .users
                .get_id(&msg.0, &self.db.friend_requests)
                .map(|user| PublicUserOther::from(user))
        }
    }

    pub struct UserAction {
        pub action: Action,
        pub auth: UserAuth,
    }
    pub enum Action {
        FriendsAction(FriendsAction),
    }
    pub enum FriendsAction {
        Request(UserId),
        Delete(UserId), // will delete either a friend or an outgoing or incoming friend request
    }
    impl Message for UserAction {
        type Result = bool;
    }
    impl Handler<UserAction> for UserManager {
        type Result = bool;
        fn handle(&mut self, msg: UserAction, _ctx: &mut Self::Context) -> Self::Result {
            if let Some(user_me) = self.db.users.get_auth(msg.auth, &self.db.friend_requests) {
                match msg.action {
                    Action::FriendsAction(friends_action) => {
                        use FriendsAction::*;
                        match friends_action {
                            Request(other_id) => {
                                if user_me.id != other_id && !user_me.friends.contains(&other_id) {
                                    let friend_requests = self
                                        .db
                                        .friend_requests
                                        .get_requests_for(user_me.id, &self.db.users);
                                    if friend_requests.iter().any(|req| {
                                        req.direction == PublicFriendRequestDirection::Incoming
                                            && req.other.id == other_id
                                    }) {
                                        // User has incoming friend request from other user -> accept request
                                        if self.db.users.add_friend(user_me.id, other_id) {
                                            self.db.friend_requests.remove(user_me.id, other_id)
                                        } else {
                                            // db fail
                                            false
                                        }
                                    } else if friend_requests.iter().any(|req| {
                                        req.direction == PublicFriendRequestDirection::Outgoing
                                            && req.other.id == other_id
                                    }) {
                                        // User has already sent a request to this user.
                                        false
                                    } else {
                                        self.db.friend_requests.insert(user_me.id, other_id)
                                    }
                                } else {
                                    false
                                }
                            }
                            Delete(other_id) => {
                                if user_me.friends.iter().any(|f| *f == other_id) {
                                    self.db.users.remove_friend(user_me.id, other_id)
                                } else if user_me
                                    .friend_requests
                                    .iter()
                                    .any(|fr| fr.other.id == other_id)
                                {
                                    self.db.friend_requests.remove(user_me.id, other_id.clone())
                                } else {
                                    false
                                }
                            }
                        }
                    }
                }
            } else {
                false
            }
        }
    }

    pub struct BattleReq {
        pub sender: (Addr<ClientAdapter>, UserId),
        pub receiver: UserId,
    }
    impl Message for BattleReq {
        type Result = ();
    }
    impl Handler<BattleReq> for UserManager {
        type Result = ();
        fn handle(&mut self, msg: BattleReq, _ctx: &mut Self::Context) {
            if let BacklinkState::Linked(lobby_mgr) = &self.lobby_mgr_state {
                // println!("user_mgr: got battlereq");
                if let Some(receiver) = self
                    .db
                    .users
                    .get_id(&msg.receiver, &self.db.friend_requests)
                {
                    if let Some(receiver_addr) = &receiver.playing {
                        lobby_mgr.do_send(lobby_mgr::BattleReq {
                            sender: msg.sender,
                            receiver: (receiver_addr.clone(), msg.receiver),
                        });
                    } else {
                        msg.sender
                            .0
                            .do_send(ServerMessage::Error(Some(SrvMsgError::UserNotPlaying)));
                    }
                } else {
                    // println!("no such user: {}", msg.receiver);
                    msg.sender
                        .0
                        .do_send(ServerMessage::Error(Some(SrvMsgError::NoSuchUser)));
                }
            } else {
                msg.sender
                    .0
                    .do_send(ServerMessage::Error(Some(SrvMsgError::Internal)));
            }
        }
    }
}
