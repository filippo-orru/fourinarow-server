use std::{cmp::Ordering, fmt, time::SystemTime};

use mongodb::{
    bson::{self, doc},
    sync::Collection,
};
use serde::{Deserialize, Serialize};

use crate::api::{
    chat::ChatThreadId,
    users::user::{BackendFriendshipMe, BackendFriendshipState, BackendFriendshipsMe, UserId},
};

use super::deserialize_vec;

pub struct FriendshipCollection {
    pub collection: Collection,
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
    pub fn new(collection: Collection) -> Self {
        FriendshipCollection { collection }
    }

    pub fn get_for(&self, user_id: UserId) -> BackendFriendshipsMe {
        self.collection
            .find(
                doc! {"$or": [{"from_id": user_id.to_string()}, {"to_id": user_id.to_string()}]},
                None,
            )
            .ok()
            .map(|cursor| deserialize_vec::<DbFriendship>(cursor))
            .map(|db_friend_requests| {
                BackendFriendshipsMe::from(
                    db_friend_requests
                        .into_iter()
                        .map(|friend_request| friend_request.to_backend(user_id))
                        .collect::<Vec<BackendFriendshipMe>>(),
                )
            })
            .unwrap_or(BackendFriendshipsMe::new())
    }

    pub fn insert(&self, from_id: UserId, to_id: UserId) -> bool {
        self.collection
            .insert_one(
                bson::to_document(&DbFriendship::new(from_id, to_id)).unwrap(),
                None,
            )
            .is_ok()
    }

    pub fn upgrade_to_friends(
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
            .is_ok()
    }

    pub fn remove(&self, from_id: UserId, to_id: UserId) -> bool {
        self.collection
            .delete_one(
                doc! { "_id": FriendshipId::new(&from_id, &to_id).to_string() },
                None,
            )
            .is_ok()
    }
}
