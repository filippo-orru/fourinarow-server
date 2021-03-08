use std::time::SystemTime;

use mongodb::{
    bson::{self, doc},
    sync::Collection,
};
use serde::{Deserialize, Serialize};

use crate::api::users::user::{PublicFriendRequest, PublicFriendRequestDirection, UserId};

use super::{deserialize_vec, users::UserCollection};

pub struct FriendRequestCollection {
    pub collection: Collection,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct DbFriendRequest {
    from_id: UserId,
    to_id: UserId,
    date: i64,
}

impl DbFriendRequest {
    fn new(from_id: UserId, to_id: UserId) -> Self {
        DbFriendRequest {
            from_id,
            to_id,
            date: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
        }
    }
}

impl FriendRequestCollection {
    pub fn new(collection: Collection) -> Self {
        FriendRequestCollection { collection }
    }

    pub fn get_requests_for(
        &self,
        user_id: UserId,
        users: &UserCollection,
    ) -> Vec<PublicFriendRequest> {
        self.collection
            .find(
                doc! {"$or": [{"from_id": user_id.to_string()}, {"to_id": user_id.to_string()}]},
                None,
            )
            .ok()
            .map(|cursor| deserialize_vec::<DbFriendRequest>(cursor))
            .map(|db_friend_requests| {
                db_friend_requests
                    .into_iter()
                    .filter_map(|friend_request| {
                        if friend_request.from_id == user_id {
                            users.get_id_public(&friend_request.to_id).map(|u| {
                                PublicFriendRequest {
                                    direction: PublicFriendRequestDirection::Outgoing,
                                    other: u,
                                }
                            })
                        } else {
                            users.get_id_public(&friend_request.from_id).map(|u| {
                                PublicFriendRequest {
                                    direction: PublicFriendRequestDirection::Incoming,
                                    other: u,
                                }
                            })
                        }
                    })
                    .collect::<Vec<PublicFriendRequest>>()
            })
            .unwrap_or(Vec::new())
    }

    pub fn insert(&self, from_id: UserId, to_id: UserId) -> bool {
        self.collection
            .insert_one(
                bson::to_document(&DbFriendRequest::new(from_id, to_id)).unwrap(),
                None,
            )
            .is_ok()
    }

    pub fn remove(&self, from_id: UserId, to_id: UserId) -> bool {
        let from_id = from_id.to_string();
        let to_id = to_id.to_string();
        self.collection
            .delete_one(
                doc! {"$or": [{"from_id": &from_id, "to_id": &to_id},
                {"from_id": to_id, "to_id": from_id}]},
                None,
            )
            .is_ok()
    }
}
