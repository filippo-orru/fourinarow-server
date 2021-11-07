use actix::Addr;
use dashmap::DashMap;
use futures::future::OptionFuture;
use mongodb::{
    bson::{self, doc},
    Collection, Database,
};
use serde::{Deserialize, Serialize};
use tokio::stream::StreamExt;

use super::friendships::FriendshipCollection;
use crate::{
    api::users::{
        session_token::SessionToken,
        user::{BackendUserMe, HashedPassword, PublicUserOther, UserGameInfo, UserId},
        user_mgr::UserAuth,
    },
    game::client_state::ClientState,
};

pub struct UserCollection {
    collection: Collection<DbUser>,
    playing_users_cache: DashMap<UserId, Addr<ClientState>>,
}

impl UserCollection {
    pub fn new(db: &Database) -> Self {
        UserCollection {
            collection: db.collection_with_type("users"),
            playing_users_cache: DashMap::new(),
        }
    }

    async fn get_auth(
        &self,
        auth: UserAuth,
        friendships: &FriendshipCollection,
    ) -> Option<BackendUserMe> {
        if let Some(user) = self.get_username(&auth.username, friendships).await {
            if user.password.matches(&auth.password) {
                return Some(user);
            }
        }
        None
    }

    pub async fn get_session_token(
        &self,
        session_token: SessionToken,
        friendships: &FriendshipCollection,
    ) -> Option<BackendUserMe> {
        let user: OptionFuture<_> = self
            .collection
            .find_one(doc! {"session_tokens": session_token.to_string() }, None)
            .await
            .ok()
            .flatten()
            .map(|user| user.to_backend_user(&self, friendships))
            .into();
        user.await
    }

    pub async fn create_session_token(
        &self,
        auth: UserAuth,
        friendships: &FriendshipCollection,
    ) -> Option<SessionToken> {
        if let Some(user) = self.get_auth(auth, friendships).await {
            let session_token = SessionToken::new();
            return self
                .collection
                .update_one(
                    doc! {"username": user.username},
                    doc! { "$push": { "session_tokens": session_token.to_string() } },
                    None,
                )
                .await
                .map(|_| session_token)
                .ok();
        }
        None
    }

    pub async fn remove_session_token(&self, session_token: SessionToken) -> Result<(), ()> {
        let session_token_str = session_token.to_string();
        self.collection
            .update_one(
                doc! { "session_tokens": &session_token_str },
                doc! { "$pull": { "session_tokens": session_token_str } },
                None,
            )
            .await
            .map(|_| ())
            .map_err(|_| ())
    }

    pub async fn get_username(
        &self,
        username: &str,
        friendships: &FriendshipCollection,
    ) -> Option<BackendUserMe> {
        let user: OptionFuture<_> = self
            .collection
            .find_one(doc! { "username": username }, None)
            .await
            .ok()
            .flatten()
            .map(|user| user.to_backend_user(&self, friendships))
            .into();
        user.await
    }

    pub async fn get_id(
        &self,
        id: &UserId,
        friendships: &FriendshipCollection,
    ) -> Option<BackendUserMe> {
        let user: OptionFuture<_> = self
            .collection
            .find_one(doc! {"_id": id.to_string()}, None)
            .await
            .ok()
            .flatten()
            .map(|user| user.to_backend_user(&self, friendships))
            .into();
        user.await
    }

    pub async fn get_id_public(&self, id: UserId) -> Option<PublicUserOther> {
        self.collection
            .find_one(doc! {"_id": id.to_string()}, None)
            .await
            .ok()
            .flatten()
            .map(|user| PublicUserOther {
                id: user.id,
                username: user.username,
                game_info: user.game_info,
                playing: self.playing_users_cache.contains_key(&user.id),
            })
    }

    pub async fn query(&self, query: &str) -> Vec<PublicUserOther> {
        let query = query.to_lowercase();

        // println!("query: {} -> {:?}", query, x);
        let users: OptionFuture<_> = self
            .collection
            .find(doc! {"username": {"$regex": &query}}, None)
            .await
            .map(|cursor| {
                cursor
                    .map(|result| {
                        result.map(|user| PublicUserOther {
                            id: user.id,
                            username: user.username,
                            game_info: user.game_info,

                            playing: self.playing_users_cache.contains_key(&user.id),
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .ok()
            .into();
        users.await.map(|r| r.ok()).flatten().unwrap_or(Vec::new())
    }

    pub async fn insert(&self, user: BackendUserMe) -> bool {
        self.collection
            .insert_one(DbUser::from_backend_user(user), None)
            .await
            .is_ok()
    }

    pub async fn update(&self, user: BackendUserMe) -> bool {
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
            .await
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
    #[allow(dead_code)]
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
    async fn to_backend_user(
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
            friendships: friendships.get_for(self.id).await,
        }
    }
}
