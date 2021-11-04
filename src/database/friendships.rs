use futures::{future::OptionFuture, stream::Collect};
use mongodb::{
    bson::{self, doc},
    Collection,
};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, fmt, time::SystemTime};
use tokio::stream::StreamExt;

use crate::api::{
    chat::ChatThreadId,
    users::user::{BackendFriendshipMe, BackendFriendshipState, BackendFriendshipsMe, UserId},
};

pub struct FriendshipCollection {
    pub collection: Collection<DbFriendship>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct FriendshipId(String);

impl FriendshipId {
    /// Generates the same id, no matter which order from_id and to_id are passed
    fn new(from_id: &UserId, to_id: &UserId) -> FriendshipId {
        let (id_one, id_two) = if from_id.cmp(to_id) != Ordering::Less {
            (from_id, to_id)
        } else {
            (to_id, from_id)
        };
        FriendshipId(format!("{}{}", id_one, id_two))
    }
}

impl fmt::Display for FriendshipId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DbFriendship {
    #[serde(rename = "_id")]
    friendship_id: FriendshipId,
    date: i64,
    friendship_type: DbFriendshipType,

    /// Order undefined
    from_id: UserId,
    to_id: UserId,
}

#[derive(Debug, Deserialize, Serialize)]
enum DbFriendshipType {
    FromFromToTo, // ;)
    Friends { chat_thread_id: String },
}

impl DbFriendship {
    fn new(from_id: UserId, to_id: UserId) -> Self {
        DbFriendship {
            friendship_id: FriendshipId::new(&from_id, &to_id),
            from_id,
            to_id,
            date: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            friendship_type: DbFriendshipType::FromFromToTo,
        }
    }

    fn to_backend(self, from_id: UserId) -> BackendFriendshipMe {
        let (other_id, friendship_type) = if self.from_id == from_id {
            (
                self.to_id,
                match self.friendship_type {
                    DbFriendshipType::FromFromToTo => BackendFriendshipState::ReqOutgoing,
                    DbFriendshipType::Friends { chat_thread_id } => {
                        BackendFriendshipState::Friends {
                            chat_thread_id: chat_thread_id.into(),
                        }
                    }
                },
            )
        } else {
            (
                self.from_id,
                match self.friendship_type {
                    DbFriendshipType::FromFromToTo => BackendFriendshipState::ReqIncoming,
                    DbFriendshipType::Friends { chat_thread_id } => {
                        BackendFriendshipState::Friends {
                            chat_thread_id: chat_thread_id.into(),
                        }
                    }
                },
            )
        };

        BackendFriendshipMe {
            other_id,
            state: friendship_type,
        }
    }
}

impl FriendshipCollection {
    pub fn new(collection: Collection<DbFriendship>) -> Self {
        FriendshipCollection { collection }
    }

    pub async fn get_for(&self, user_id: UserId) -> BackendFriendshipsMe {
        let friends: OptionFuture<Collect<_, Result<DbFriendship, _>>> = self
            .collection
            .find(
                doc! {"$or": [{"from_id": user_id.to_string()}, {"to_id": user_id.to_string()}]},
                None,
            )
            .await
            .ok()
            .map(|cursor| cursor.collect::<Result<_, _>>())
            .into();

        friends
            .await
            .map(|r| {
                r.map(|friendships| {
                    BackendFriendshipsMe::from(
                        friendships
                            .into_iter()
                            .map(|friend_request| friend_request.to_backend(user_id))
                            .collect::<Vec<BackendFriendshipMe>>(),
                    )
                })
            })
            .unwrap_or(BackendFriendshipsMe::new())
    }

    pub async fn insert(&self, from_id: UserId, to_id: UserId) -> bool {
        self.collection
            .insert_one(DbFriendship::new(from_id, to_id), None)
            .await
            .is_ok()
    }

    pub async fn upgrade_to_friends(
        &self,
        from_id: UserId,
        to_id: UserId,
        chat_thread_id: ChatThreadId,
    ) -> bool {
        let friendship_type = bson::to_document(&DbFriendshipType::Friends {
            chat_thread_id: chat_thread_id.to_string(),
        })
        .unwrap();

        self.collection
            .update_one(
                doc! { "_id": FriendshipId::new(&from_id, &to_id).to_string() },
                doc! { "$set": { "friendship_type": friendship_type }},
                None,
            )
            .await
            .is_ok()
    }

    pub async fn remove(&self, from_id: UserId, to_id: UserId) -> bool {
        self.collection
            .delete_one(
                doc! { "_id": FriendshipId::new(&from_id, &to_id).to_string() },
                None,
            )
            .await
            .is_ok()
    }
}
