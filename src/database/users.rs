use actix::Addr;
use dashmap::DashMap;
use mongodb::{
    bson::{self, doc},
    sync::Collection,
};
use serde::{Deserialize, Serialize};

use crate::{
    api::users::{
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

    pub fn get_auth(
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

    pub fn get_username(
        &self,
        username: &str,
        friend_requests: &FriendRequestCollection,
    ) -> Option<BackendUser> {
        self.collection
            .find_one(doc! {"username": username}, None)
            .ok()
            .flatten()
            .and_then(|doc| {
                super::deserialize::<DbUser>(doc)
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
    ) -> Vec<BackendUser> {
        let query = query.to_lowercase();

        self.collection
            .find(doc! {"username": { "$contains": query} }, None)
            .ok()
            .map(|cursor| deserialize_vec::<DbUser>(cursor))
            .unwrap_or(Vec::new())
            .into_iter()
            .map(|user| user.to_backend_user(&self, friend_requests))
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
        }

        self.collection
            .update_one(
                doc! { "_id": user.id.to_string()},
                bson::to_document(&DbUser::from_backend_user(user)).unwrap(),
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

#[derive(Serialize, Deserialize)]
struct DbUser {
    #[serde(rename = "_id")]
    pub id: UserId,
    pub username: String,
    pub password: HashedPassword,
    pub email: Option<String>,
    pub game_info: UserGameInfo,
    pub friends: Vec<UserId>,
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
            friend_requests: friend_requests.get_requests_for(self.id, users),
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
