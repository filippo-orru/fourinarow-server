use mongodb::{Collection, Database};

use crate::api::users::user::PlayedGameInfo;

pub struct GameCollection {
    collection: Collection<PlayedGameInfo>,
}

impl GameCollection {
    pub fn new(db: &Database) -> Self {
        GameCollection {
            collection: db.collection_with_type("games"),
        }
    }

    pub async fn insert(&self, game: PlayedGameInfo) -> bool {
        self.collection.insert_one(game, None).await.is_ok()
    }
}
