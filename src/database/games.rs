use mongodb::Collection;

use crate::api::users::user::PlayedGameInfo;

pub struct GameCollection {
    collection: Collection<PlayedGameInfo>,
}

impl GameCollection {
    pub fn new(collection: Collection<PlayedGameInfo>) -> Self {
        GameCollection { collection }
    }

    pub async fn insert(&self, game: PlayedGameInfo) -> bool {
        self.collection.insert_one(game, None).await.is_ok()
    }
}
