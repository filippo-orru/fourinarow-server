use actix::Addr;
use dashmap::DashMap;
use mongodb::{
    bson::{self, doc},
    sync::Collection,
};
use serde::{Deserialize, Serialize};

use super::{deserialize_vec, friendships::FriendshipCollection};
use crate::{
    api::users::{
        session_token::SessionToken,
        user::{BackendUserMe, BackendUserOther, HashedPassword, UserGameInfo, UserId},
        user_mgr::UserAuth,
    },
    game::client_adapter::ClientAdapter,
};

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
        friendships: &FriendshipCollection,
    ) -> Option<BackendUserMe> {
        if let Some(user) = self.get_username(&auth.username, friendships) {
            if user.password.matches(&auth.password) {
                return Some(user);
            }
        }
        None
    }

    pub fn get_session_token(
        &self,
        session_token: SessionToken,
        friendships: &FriendshipCollection,
    ) -> Option<BackendUserMe> {
        self.collection
            .find_one(doc! {"session_tokens": session_token.to_string() }, None)
            .ok()
            .flatten()
            .and_then(|doc| {
                super::deserialize::<DbUser>(doc.into())
                    .map(|user| user.to_backend_user(&self, friendships))
            })
    }

    pub fn create_session_token(
        &self,
        auth: UserAuth,
        friendships: &FriendshipCollection,
    ) -> Option<SessionToken> {
        if let Some(user) = self.get_auth(auth, friendships) {
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
        friendships: &FriendshipCollection,
    ) -> Option<BackendUserMe> {
        self.collection
            .find_one(doc! { "username": username }, None)
            .ok()
            .flatten()
            .and_then(|doc| {
                super::deserialize::<DbUser>(doc.into())
                    .map(|user| user.to_backend_user(&self, friendships))
            })
    }

    pub fn get_id(&self, id: &UserId, friendships: &FriendshipCollection) -> Option<BackendUserMe> {
        self.collection
            .find_one(doc! {"_id": id.to_string()}, None)
            .ok()
            .flatten()
            .and_then(|doc| {
                super::deserialize::<DbUser>(doc)
                    .map(|user| user.to_backend_user(&self, friendships))
            })
    }

    pub fn get_id_other(&self, id: &UserId) -> Option<BackendUserOther> {
        self.collection
            .find_one(doc! {"_id": id.to_string()}, None)
            .ok()
            .flatten()
            .and_then(|doc| {
                super::deserialize::<DbUser>(doc).map(|user| BackendUserOther {
                    id: user.id,
                    username: user.username,
                    game_info: user.game_info,
                    playing: self.playing_users_cache.contains_key(&user.id),
                })
            })
    }

    pub fn query(&self, query: &str) -> Vec<BackendUserOther> {
        let query = query.to_lowercase();

        // println!("query: {} -> {:?}", query, x);
        self.collection
            .find(doc! {"username": {"$regex": &query}}, None)
            .ok()
            .map(|cursor| deserialize_vec::<DbUser>(cursor))
            .unwrap_or(Vec::new())
            .into_iter()
            .map(|user| BackendUserOther {
                id: user.id,
                username: user.username,
                game_info: user.game_info,

                playing: self.playing_users_cache.contains_key(&user.id),
            })
            .collect()
    }

    pub fn insert(&self, user: BackendUserMe) -> bool {
        self.collection
            .insert_one(
                bson::to_document(&DbUser::from_backend_user(user)).unwrap(),
                None,
            )
            .is_ok()
    }

    pub fn update(&self, user: BackendUserMe) -> bool {
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

    #[serde(skip)]
    pub session_tokens: Vec<SessionToken>,
}

impl DbUser {
    fn from_backend_user(user: BackendUserMe) -> Self {
        DbUser {
            id: user.id,
            username: user.username,
            password: user.password,
            email: user.email,
            game_info: user.game_info,

            session_tokens: vec![],
        }
    }
    fn to_backend_user(
        self,
        users: &UserCollection,
        friendships: &FriendshipCollection,
    ) -> BackendUserMe {
        BackendUserMe {
            id: self.id,
            username: self.username,
            password: self.password,
            email: self.email,
            game_info: self.game_info,
            playing: users.playing_users_cache.get(&self.id).map(|p| p.clone()),
            friendships: friendships.get_for(self.id),
        }
    }
}
