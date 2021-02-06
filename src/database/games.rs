use mongodb::{bson, sync::Collection};

use crate::api::users::user::PlayedGameInfo;

pub struct GameCollection {
    collection: Collection,
}

impl GameCollection {
    pub fn new(collection: Collection) -> Self {
        GameCollection { collection }
    }

    pub fn insert(&self, game: PlayedGameInfo) -> bool {
        self.collection
            .insert_one(bson::to_document(&game).unwrap(), None)
            .is_ok()
    }
}
