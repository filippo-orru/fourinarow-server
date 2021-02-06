use mongodb::{
    bson::{self, doc},
    sync::Collection,
};

use crate::api::users::{
    user::{User, UserId},
    user_mgr::UserAuth,
};

use super::deserialize_vec;

pub struct UserCollection {
    pub collection: Collection,
}

impl UserCollection {
    pub fn new(collection: Collection) -> Self {
        UserCollection { collection }
    }

    pub fn get_auth(&self, auth: UserAuth) -> Option<User> {
        if let Some(user) = self.get_username(&auth.username) {
            if user.password.matches(&auth.password) {
                return Some(user);
            }
        }
        None
    }

    pub fn get_username(&self, username: &str) -> Option<User> {
        self.collection
            .find_one(doc! {"username": username}, None)
            .ok()
            .flatten()
            .and_then(|doc| super::deserialize(doc))
    }

    pub fn get_id(&self, id: &UserId) -> Option<User> {
        self.collection
            .find_one(doc! {"id": id.to_string()}, None)
            .ok()
            .flatten()
            .and_then(|doc| super::deserialize(doc))
    }

    pub fn query(&self, query: &str) -> Vec<User> {
        let query = query.to_lowercase();

        self.collection
            .find(doc! {"username": { "$contains": query} }, None)
            .ok()
            .map(|cursor| deserialize_vec(cursor))
            .unwrap_or(Vec::new())
    }

    pub fn insert(&self, user: User) -> bool {
        self.collection
            .insert_one(bson::to_document(&user).unwrap(), None)
            .is_ok()
    }

    pub fn update(&self, user: User) -> bool {
        self.collection
            .update_one(
                doc! { "id": user.id.to_string()},
                bson::to_document(&user).unwrap(),
                None,
            )
            .is_ok()
    }
}
