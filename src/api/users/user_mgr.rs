use crate::api::{chat::ChatThreadId, users::user::*, ApiError};
use crate::database::DatabaseManager;
use crate::game::client_adapter::ClientAdapterMsg;
use crate::game::lobby_mgr::{self, LobbyManager};
use crate::game::msg::*;

use actix::prelude::*;
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

#[derive(Clone)]
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

    use futures::future::OptionFuture;

    use super::*;
    use crate::{
        api::users::session_token::SessionToken,
        game::{client_state::ClientState, msg::SrvMsgError},
    };

    pub struct Register(pub UserAuth);
    impl Message for Register {
        type Result = Result<SessionToken, ApiError>;
    }
    impl Handler<Register> for UserManager {
        type Result = ResponseActFuture<Self, Result<SessionToken, ApiError>>;

        fn handle(&mut self, msg: Register, _ctx: &mut Self::Context) -> Self::Result {
            let auth = msg.0;
            let db = self.db.clone();

            Box::pin(
                async move {
                    let username_is_in_use = db
                        .users
                        .get_username(&auth.username, &db.friendships)
                        .await
                        .is_some();

                    if !BackendUserMe::check_password(&auth.password) {
                        Err(ApiError::PasswordInsufficient)
                    } else if username_is_in_use {
                        Err(ApiError::UsernameInUse)
                    } else {
                        let mut user =
                            BackendUserMe::new(auth.username.clone(), auth.password.clone());
                        while db.users.get_id(&user.id, &db.friendships).await.is_some() {
                            user.gen_new_id();
                        }
                        db.users.insert(user.clone()).await;
                        db.users
                            .create_session_token(auth, &db.friendships)
                            .await
                            .ok_or(ApiError::IncorrectCredentials)
                    }
                }
                .into_actor(self),
            )
            //.map(|res, _, _| res)
            //.boxed_local(ctx)
        }
    }
    pub struct Login(pub UserAuth);
    impl Message for Login {
        type Result = Result<SessionToken, ApiError>;
    }
    impl Handler<Login> for UserManager {
        type Result = ResponseActFuture<Self, Result<SessionToken, ApiError>>;

        fn handle(&mut self, msg: Login, _ctx: &mut Self::Context) -> Self::Result {
            let db = self.db.clone();
            Box::pin(
                async move {
                    db.users
                        .create_session_token(msg.0, &db.friendships)
                        .await
                        .ok_or(ApiError::IncorrectCredentials)
                }
                .into_actor(self),
            )
        }
    }

    pub struct Logout(pub SessionToken);
    impl Message for Logout {
        type Result = Result<(), ApiError>;
    }
    impl Handler<Logout> for UserManager {
        type Result = ResponseActFuture<Self, Result<(), ApiError>>;

        fn handle(&mut self, msg: Logout, _ctx: &mut Self::Context) -> Self::Result {
            let db = self.db.clone();
            Box::pin(
                async move {
                    db.users
                        .remove_session_token(msg.0)
                        .await
                        .map_err(|_| ApiError::InternalServerError)
                }
                .into_actor(self),
            )
        }
    }

    pub struct StartPlaying {
        pub session_token: SessionToken,
        pub addr: Addr<ClientState>,
    }
    impl Message for StartPlaying {
        type Result = Result<PublicUserMe, SrvMsgError>;
    }
    impl Handler<StartPlaying> for UserManager {
        type Result = ResponseActFuture<Self, Result<PublicUserMe, SrvMsgError>>;
        fn handle(&mut self, msg: StartPlaying, _ctx: &mut Self::Context) -> Self::Result {
            let db = self.db.clone();
            Box::pin(
                async move {
                    if let Some(user) = db
                        .users
                        .get_session_token(msg.session_token, &db.friendships)
                        .await
                    {
                        let mut user = user;
                        if let Some(client_adapter) = user.playing {
                            // Cancel current connection
                            client_adapter.do_send(ClientAdapterMsg::Close);
                        }
                        user.playing = Some(msg.addr);
                        db.users.update(user.clone()).await;
                        Ok(user.to_public_user_me(&db).await)
                    } else {
                        Err(SrvMsgError::IncorrectCredentials)
                    }
                }
                .into_actor(self),
            )
        }
    }

    pub enum IntUserMgrMsg {
        Backlink(Addr<LobbyManager>),
        Game(GameMsg),
        // StartPlaying(String, String),
        StopPlaying(UserId, Addr<ClientState>),
    }
    pub enum GameMsg {
        PlayedGame(PlayedGameInfo),
    }

    impl Message for IntUserMgrMsg {
        type Result = ();
    }
    impl Handler<IntUserMgrMsg> for UserManager {
        type Result = ResponseActFuture<Self, ()>;
        fn handle(&mut self, msg: IntUserMgrMsg, _ctx: &mut Self::Context) -> Self::Result {
            let db = self.db.clone();
            Box::pin(
                async move {
                    use GameMsg::*;
                    use IntUserMgrMsg::*;
                    let mut lobby_mgr_state: Option<BacklinkState> = None;
                    match msg {
                        Backlink(lobby_mgr) => {
                            lobby_mgr_state = Some(BacklinkState::Linked(lobby_mgr))
                        }
                        Game(game_msg) => match game_msg {
                            PlayedGame(game_info) => {
                                let mut found = false;
                                if let Some(mut winner) =
                                    db.users.get_id(&game_info.winner, &db.friendships).await
                                {
                                    winner.game_info.skill_rating += SR_PER_WIN;
                                    db.users.update(winner).await;
                                    found = true;
                                }
                                if let Some(mut loser) =
                                    db.users.get_id(&game_info.loser, &db.friendships).await
                                {
                                    if found {
                                        loser.game_info.skill_rating -= SR_PER_WIN;
                                        db.users.update(loser).await;
                                        db.games.insert(game_info).await;
                                    }
                                } else if found {
                                    if let Some(mut winner) =
                                        db.users.get_id(&game_info.winner, &db.friendships).await
                                    {
                                        winner.game_info.skill_rating -= SR_PER_WIN;
                                        db.users.update(winner).await;
                                    }
                                }
                            }
                        },
                        // StartPlaying(id) => {
                        //     if let Some(user) = db.users.get_mut(&id) {
                        //         user.playing = false;
                        //     }
                        // }
                        StopPlaying(id, addr) => {
                            if let Some(mut user) = db.users.get_id(&id, &db.friendships).await {
                                if let Some(playing_addr) = user.playing {
                                    if playing_addr == addr {
                                        // Only reset the address if the requesting ClientAdapter is still linked (might have been replaced already)
                                        user.playing = None;
                                        db.users.update(user).await;
                                    }
                                }
                            }
                        }
                    }
                    lobby_mgr_state
                }
                .into_actor(self)
                .map(|maybe_lobby_mgr_state, act, _| {
                    if let Some(state) = maybe_lobby_mgr_state {
                        act.lobby_mgr_state = state;
                    }
                }),
            )
        }
    }

    pub struct SearchUsers {
        pub query: String,
    }
    impl Message for SearchUsers {
        type Result = Option<Vec<PublicUserOther>>;
    }

    impl Handler<SearchUsers> for UserManager {
        type Result = ResponseActFuture<Self, Option<Vec<PublicUserOther>>>;

        fn handle(&mut self, msg: SearchUsers, _ctx: &mut Self::Context) -> Self::Result {
            let db = self.db.clone();
            Box::pin(async move { Some(db.users.query(&msg.query).await) }.into_actor(self))
        }
    }

    pub struct GetUserMe(pub SessionToken);
    impl Message for GetUserMe {
        type Result = Option<PublicUserMe>;
    }

    impl Handler<GetUserMe> for UserManager {
        type Result = ResponseActFuture<Self, Option<PublicUserMe>>;
        fn handle(&mut self, msg: GetUserMe, _ctx: &mut Self::Context) -> Self::Result {
            let db = self.db.clone();
            Box::pin(
                async move {
                    Into::<OptionFuture<_>>::into(
                        db.users
                            .get_session_token(msg.0, &db.friendships)
                            .await
                            .map(|user| user.to_public_user_me(&db)),
                    )
                    .await
                }
                .into_actor(self),
            )
        }
    }

    pub struct GetUserOther(pub UserId);
    impl Message for GetUserOther {
        type Result = Option<PublicUserOther>;
    }

    impl Handler<GetUserOther> for UserManager {
        type Result = ResponseActFuture<Self, Option<PublicUserOther>>;
        fn handle(&mut self, msg: GetUserOther, _ctx: &mut Self::Context) -> Self::Result {
            let db = self.db.clone();
            Box::pin(async move { db.users.get_id_public(msg.0).await }.into_actor(self))
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
        type Result = ResponseActFuture<Self, bool>;
        fn handle(&mut self, msg: UserAction, _ctx: &mut Self::Context) -> Self::Result {
            let db = self.db.clone();
            Box::pin(
                async move {
                    if let Some(user_me) = db
                        .users
                        .get_session_token(msg.session_token, &db.friendships)
                        .await
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
                                                db.friendships
                                                    .upgrade_to_friends(
                                                        user_me.id,
                                                        other_id,
                                                        chat_thread_id,
                                                    )
                                                    .await
                                            } else if user_me.friendships.iter().any(|req| {
                                                req.state == BackendFriendshipState::ReqOutgoing
                                                    && req.other_id == other_id
                                            }) {
                                                // User has already sent a request to this user.
                                                false
                                            } else {
                                                db.friendships.insert(user_me.id, other_id).await
                                            }
                                        } else {
                                            false
                                        }
                                    }
                                    Delete(other_id) => {
                                        if user_me
                                            .friendships
                                            .iter()
                                            .any(|fr| fr.other_id == other_id)
                                        {
                                            db.friendships
                                                .remove(user_me.id, other_id.clone())
                                                .await
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
                .into_actor(self),
            )
        }
    }

    pub struct BattleReq {
        pub sender_addr: Addr<ClientState>,
        pub sender_uid: UserId,
        pub receiver_uid: UserId,
    }
    impl Message for BattleReq {
        type Result = ();
    }
    impl Handler<BattleReq> for UserManager {
        type Result = ResponseActFuture<Self, ()>;
        fn handle(&mut self, msg: BattleReq, _ctx: &mut Self::Context) -> Self::Result {
            let db = self.db.clone();
            let lobby_mgr = self.lobby_mgr_state.clone();

            Box::pin(
                async move {
                    if let BacklinkState::Linked(lobby_mgr) = &lobby_mgr {
                        if let Some(receiver) =
                            db.users.get_id(&msg.receiver_uid, &db.friendships).await
                        {
                            if let Some(receiver_addr) = &receiver.playing {
                                lobby_mgr.do_send(lobby_mgr::BattleReq {
                                    sender_addr: msg.sender_addr,
                                    sender_uid: msg.sender_uid,
                                    receiver_addr: receiver_addr.clone(),
                                    receiver_uid: msg.receiver_uid,
                                });
                            } else {
                                msg.sender_addr.do_send(ServerMessage::Error(Some(
                                    SrvMsgError::UserNotPlaying,
                                )));
                            }
                        } else {
                            // println!("no such user: {}", msg.receiver);
                            msg.sender_addr
                                .do_send(ServerMessage::Error(Some(SrvMsgError::NoSuchUser)));
                        }
                    } else {
                        msg.sender_addr
                            .do_send(ServerMessage::Error(Some(SrvMsgError::Internal)));
                    }
                }
                .into_actor(self),
            )
        }
    }
}
