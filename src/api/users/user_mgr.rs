use crate::api::{chat::ChatThreadId, users::user::*, ApiError};
use crate::game::client_adapter::ClientAdapterMsg;
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

#[derive(Deserialize, Debug)]
pub struct UserAuth {
    pub username: String,
    pub password: String,
}

pub mod msg {

    use super::*;
    use crate::{api::users::session_token::SessionToken, game::msg::SrvMsgError};

    pub struct Register(pub UserAuth);
    impl Message for Register {
        type Result = Result<SessionToken, ApiError>;
    }
    impl Handler<Register> for UserManager {
        type Result = Result<SessionToken, ApiError>;

        fn handle(&mut self, msg: Register, _ctx: &mut Self::Context) -> Self::Result {
            let auth = msg.0;
            if !BackendUserMe::check_password(&auth.password) {
                Err(ApiError::PasswordInsufficient)
            } else if self
                .db
                .users
                .get_username(&auth.username, &self.db.friendships)
                .is_some()
            {
                Err(ApiError::UsernameInUse)
            } else {
                let mut user = BackendUserMe::new(auth.username.clone(), auth.password.clone());
                while self
                    .db
                    .users
                    .get_id(&user.id, &self.db.friendships)
                    .is_some()
                {
                    user.gen_new_id();
                }
                self.db.users.insert(user.clone());
                self.db
                    .users
                    .create_session_token(auth, &self.db.friendships)
                    .ok_or(ApiError::IncorrectCredentials)
            }
        }
    }
    pub struct Login(pub UserAuth);
    impl Message for Login {
        type Result = Result<SessionToken, ApiError>;
    }
    impl Handler<Login> for UserManager {
        type Result = Result<SessionToken, ApiError>;

        fn handle(&mut self, msg: Login, _ctx: &mut Self::Context) -> Self::Result {
            self.db
                .users
                .create_session_token(msg.0, &self.db.friendships)
                .ok_or(ApiError::IncorrectCredentials)
        }
    }

    pub struct Logout(pub SessionToken);
    impl Message for Logout {
        type Result = Result<(), ApiError>;
    }
    impl Handler<Logout> for UserManager {
        type Result = Result<(), ApiError>;

        fn handle(&mut self, msg: Logout, _ctx: &mut Self::Context) -> Self::Result {
            self.db
                .users
                .remove_session_token(msg.0)
                .map_err(|_| ApiError::InternalServerError)
        }
    }

    pub struct StartPlaying {
        pub session_token: SessionToken,
        pub addr: Addr<ClientAdapter>,
    }
    impl Message for StartPlaying {
        type Result = Result<PublicUserMe, SrvMsgError>;
    }
    impl Handler<StartPlaying> for UserManager {
        type Result = Result<PublicUserMe, SrvMsgError>;
        fn handle(&mut self, msg: StartPlaying, _ctx: &mut Self::Context) -> Self::Result {
            if let Some(user) = self
                .db
                .users
                .get_session_token(msg.session_token, &self.db.friendships)
            {
                let mut user = user;
                if let Some(client_adapter) = user.playing {
                    // Cancel current connection
                    client_adapter.do_send(ClientAdapterMsg::Close);
                }
                user.playing = Some(msg.addr);
                self.db.users.update(user.clone());
                Ok(user.to_public_user_me(&self.db))
            } else {
                Err(SrvMsgError::IncorrectCredentials)
            }
        }
    }

    pub enum IntUserMgrMsg {
        Backlink(Addr<LobbyManager>),
        Game(GameMsg),
        // StartPlaying(String, String),
        StopPlaying(UserId, Addr<ClientAdapter>),
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
                            .get_id(&game_info.winner, &self.db.friendships)
                        {
                            winner.game_info.skill_rating += SR_PER_WIN;
                            self.db.users.update(winner);
                            found = true;
                        }
                        if let Some(mut loser) =
                            self.db.users.get_id(&game_info.loser, &self.db.friendships)
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
                                .get_id(&game_info.winner, &self.db.friendships)
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
                StopPlaying(id, addr) => {
                    if let Some(mut user) = self.db.users.get_id(&id, &self.db.friendships) {
                        if let Some(playing_addr) = user.playing {
                            if playing_addr == addr {
                                // Only reset the address if the requesting ClientAdapter is still linked (might have been replaced already)
                                user.playing = None;
                                self.db.users.update(user);
                            }
                        }
                    }
                }
            }
        }
    }

    pub struct SearchUsers {
        pub session_token: SessionToken,
        pub query: String,
    }
    impl Message for SearchUsers {
        type Result = SearchUsersResult;
    }

    #[derive(MessageResponse)]
    pub struct SearchUsersResult(pub Vec<PublicUserOther>);

    impl Handler<SearchUsers> for UserManager {
        type Result = SearchUsersResult;
        fn handle(&mut self, msg: SearchUsers, _ctx: &mut Self::Context) -> Self::Result {
            SearchUsersResult(
                self.db
                    .users
                    .get_session_token(msg.session_token.clone(), &self.db.friendships)
                    .map(|requesting_user| {
                        self.db
                            .users
                            .query(&msg.query)
                            .into_iter()
                            .filter_map(|user| requesting_user.get_public_user_other(user))
                            .collect()
                    })
                    .unwrap_or(Vec::new()),
            )
        }
    }

    pub struct GetUserMe(pub SessionToken);
    impl Message for GetUserMe {
        type Result = Option<PublicUserMe>;
    }

    impl Handler<GetUserMe> for UserManager {
        type Result = Option<PublicUserMe>;
        fn handle(&mut self, msg: GetUserMe, _ctx: &mut Self::Context) -> Self::Result {
            // Some(
            //)  || panic!("GetUserMe: could not get by sessionT"),
            self.db
                .users
                .get_session_token(msg.0, &self.db.friendships)
                .map(|user| user.to_public_user_me(&self.db))
        }
    }

    pub struct GetUserOther {
        pub session_token: SessionToken,
        pub user_id: UserId,
    }
    impl Message for GetUserOther {
        type Result = Option<PublicUserOther>;
    }

    impl Handler<GetUserOther> for UserManager {
        type Result = Option<PublicUserOther>;
        fn handle(&mut self, msg: GetUserOther, _ctx: &mut Self::Context) -> Self::Result {
            self.db
                .users
                .get_session_token(msg.session_token.clone(), &self.db.friendships)
                .and_then(|requesting_user| {
                    self.db
                        .users
                        .get_id_other(&msg.user_id)
                        .and_then(|user| requesting_user.get_public_user_other(user))
                })
        }
    }

    pub struct UserAction {
        pub action: Action,
        pub session_token: SessionToken,
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
            if let Some(user_me) = self
                .db
                .users
                .get_session_token(msg.session_token, &self.db.friendships)
            {
                match msg.action {
                    Action::FriendsAction(friends_action) => {
                        use FriendsAction::*;
                        match friends_action {
                            Request(other_id) => {
                                // If not trying to add myself && isn't already friend
                                if user_me.id != other_id
                                    && !user_me
                                        .friendships
                                        .friends()
                                        .any(|f| f.other_id == other_id)
                                {
                                    if user_me.friendships.iter().any(|req| {
                                        req.state == BackendFriendshipState::ReqIncoming
                                            && req.other_id == other_id
                                    }) {
                                        // User has incoming friend request from other user -> accept request
                                        let chat_thread_id = ChatThreadId::new();
                                        self.db.friendships.upgrade_to_friends(
                                            user_me.id,
                                            other_id,
                                            chat_thread_id,
                                        )
                                    } else if user_me.friendships.iter().any(|req| {
                                        req.state == BackendFriendshipState::ReqOutgoing
                                            && req.other_id == other_id
                                    }) {
                                        // User has already sent a request to this user.
                                        false
                                    } else {
                                        self.db.friendships.insert(user_me.id, other_id)
                                    }
                                } else {
                                    false
                                }
                            }
                            Delete(other_id) => {
                                if user_me.friendships.iter().any(|fr| fr.other_id == other_id) {
                                    self.db.friendships.remove(user_me.id, other_id.clone())
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
                if let Some(receiver) = self.db.users.get_id(&msg.receiver, &self.db.friendships) {
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
