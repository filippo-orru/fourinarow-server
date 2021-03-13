use actix::Addr;
use dashmap::DashMap;
use mongodb::{
    bson::{self, doc},
    sync::Collection,
};
use serde::{Deserialize, Serialize};

use crate::{
    api::users::{
        session_token::SessionToken,
        user::{BackendUser, HashedPassword, PublicUserOther, UserGameInfo, UserId},
        user_mgr::UserAuth,
    },
    game::client_adapter::ClientAdapter,
};

use super::{deserialize_vec, friend_requests::FriendRequestCollection};

pub struct UserCollection {
    pub collection: Collection,
    playing_users_cache: DashMap<UserId, Addr<ClientAdapter>>,
}

impl UserCollection {
    pub fn new(collection: Collection) -> Self {
        UserCollection {
            collection,
            playing_users_cache: DashMap::new(),
        }
    }

    fn get_auth(
        &self,
        auth: UserAuth,
        friend_requests: &FriendRequestCollection,
    ) -> Option<BackendUser> {
        if let Some(user) = self.get_username(&auth.username, friend_requests) {
            if user.password.matches(&auth.password) {
                return Some(user);
            }
        }
        None
    }

    pub fn get_session_token(
        &self,
        session_token: SessionToken,
        friend_requests: &FriendRequestCollection,
    ) -> Option<BackendUser> {
        self.collection
            .find_one(doc! {"session_tokens": session_token.to_string() }, None)
            .ok()
            .flatten()
            .and_then(|doc| {
                super::deserialize::<DbUser>(doc.into())
                    .map(|user| user.to_backend_user(&self, friend_requests))
            })
    }

    pub fn create_session_token(
        &self,
        auth: UserAuth,
        friend_requests: &FriendRequestCollection,
    ) -> Option<SessionToken> {
        if let Some(user) = self.get_auth(auth, friend_requests) {
            let session_token = SessionToken::new();
            return self
                .collection
                .update_one(
                    doc! {"username": user.username},
                    doc! { "$push": { "session_tokens": session_token.to_string() } },
                    None,
                )
                .map(|_| session_token)
                .ok();
        }
        None
    }

    pub fn remove_session_token(&self, session_token: SessionToken) -> Result<(), ()> {
        let session_token_str = session_token.to_string();
        self.collection
            .update_one(
                doc! { "session_tokens": &session_token_str },
                doc! { "$pull": { "session_tokens": session_token_str } },
                None,
            )
            .map(|_| ())
            .map_err(|_| ())
    }

    pub fn get_username(
        &self,
        username: &str,
        friend_requests: &FriendRequestCollection,
    ) -> Option<BackendUser> {
        self.collection
            .find_one(doc! { "username": username }, None)
            .ok()
            .flatten()
            .and_then(|doc| {
                super::deserialize::<DbUser>(doc.into())
                    .map(|user| user.to_backend_user(&self, friend_requests))
            })
    }

    pub fn get_id(
        &self,
        id: &UserId,
        friend_requests: &FriendRequestCollection,
    ) -> Option<BackendUser> {
        self.collection
            .find_one(doc! {"_id": id.to_string()}, None)
            .ok()
            .flatten()
            .and_then(|doc| {
                super::deserialize::<DbUser>(doc)
                    .map(|user| user.to_backend_user(&self, friend_requests))
            })
    }

    pub fn get_id_public(&self, id: &UserId) -> Option<PublicUserOther> {
        self.collection
            .find_one(doc! {"_id": id.to_string()}, None)
            .ok()
            .flatten()
            .and_then(|doc| {
                super::deserialize::<DbUser>(doc).map(|user| user.to_public_user_other(&self))
            })
    }

    pub fn query(
        &self,
        query: &str,
        friend_requests: &FriendRequestCollection,
    ) -> Vec<PublicUserOther> {
        let query = query.to_lowercase();

        let x = self
            .collection
            .find(doc! {"username": {"$regex": &query}}, None)
            .ok()
            .map(|cursor| deserialize_vec::<DbUser>(cursor))
            .unwrap_or(Vec::new());

        // println!("query: {} -> {:?}", query, x);
        x.into_iter()
            .map(|user| user.to_public_user_other(&self))
            .collect()
    }

    pub fn insert(&self, user: BackendUser) -> bool {
        self.collection
            .insert_one(
                bson::to_document(&DbUser::from_backend_user(user)).unwrap(),
                None,
            )
            .is_ok()
    }

    pub fn update(&self, user: BackendUser) -> bool {
        if let Some(playing_addr) = &user.playing {
            self.playing_users_cache
                .insert(user.id.clone(), playing_addr.clone());
        } else {
            self.playing_users_cache.remove(&user.id);
        }

        self.collection
            .update_one(
                doc! { "_id": user.id.to_string()},
                doc! { "$set": bson::to_document(&DbUser::from_backend_user(user.clone())).unwrap() },
                None,
            )
            .is_ok()
    }

    // Goddamn shitty friends info is being redundantly saved in both users.
    // @future filippo have fun fixing this because i sure wont
    pub fn add_friend(&self, from_id: UserId, to_id: UserId) -> bool {
        self.collection
            .update_one(
                doc! {"_id": from_id.to_string() },
                doc! {"$push": {"friends": to_id.to_string()}},
                None,
            )
            .is_ok()
            && self
                .collection
                .update_one(
                    doc! {"_id": to_id.to_string() },
                    doc! {"$push": {"friends": from_id.to_string()}},
                    None,
                )
                .is_ok()
    }

    pub fn remove_friend(&self, from_id: UserId, to_id: UserId) -> bool {
        self.collection
            .update_one(
                doc! { "_id": from_id.to_string()},
                doc! { "$pull" : {"friends": to_id.to_string()} },
                None,
            )
            .is_ok()
            && self
                .collection
                .update_one(
                    doc! { "_id": to_id.to_string()},
                    doc! { "$pull" : {"friends": from_id.to_string()} },
                    None,
                )
                .is_ok()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DbUser {
    #[serde(rename = "_id")]
    pub id: UserId,
    pub username: String,
    pub password: HashedPassword,

    #[serde(default)]
    pub email: Option<String>,

    pub game_info: UserGameInfo,

    #[serde(default)]
    pub friends: Vec<UserId>,

    #[serde(skip)]
    pub session_tokens: Vec<SessionToken>,
}

impl DbUser {
    fn from_backend_user(user: BackendUser) -> Self {
        DbUser {
            id: user.id,
            username: user.username,
            password: user.password,
            email: user.email,
            game_info: user.game_info,
            friends: user.friends,
            session_tokens: vec![],
        }
    }
    fn to_backend_user(
        self,
        users: &UserCollection,
        friend_requests: &FriendRequestCollection,
    ) -> BackendUser {
        BackendUser {
            id: self.id,
            username: self.username,
            password: self.password,
            email: self.email,
            game_info: self.game_info,
            friends: self.friends,
            playing: users.playing_users_cache.get(&self.id).map(|p| p.clone()),
            friend_requests: friend_requests.get_requests_for(self.id),
        }
    }

    fn to_public_user_other(self, users: &UserCollection) -> PublicUserOther {
        PublicUserOther {
            id: self.id,
            username: self.username,
            game_info: self.game_info,
            playing: users.playing_users_cache.contains_key(&self.id),
        }
    }
}
