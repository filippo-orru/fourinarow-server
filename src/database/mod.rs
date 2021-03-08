pub mod friend_requests;
pub mod games;
pub mod users;

use mongodb::{
    bson::{self, Document},
    options::ClientOptions,
    sync::{Client, Cursor},
};
use serde::de::DeserializeOwned;

use self::{
    friend_requests::FriendRequestCollection, games::GameCollection, users::UserCollection,
};

const DB_URL: &str = "mongodb://localhost:27017";

pub struct DatabaseManager {
    pub users: UserCollection,
    pub games: GameCollection,
    pub friend_requests: FriendRequestCollection,
}

impl DatabaseManager {
    pub async fn new() -> DatabaseManager {
        let opt = ClientOptions::parse(DB_URL).await.unwrap();
        let client = Client::with_options(opt).expect("Failed to start mongodb client");
        let db = client.database("fourinarow");

        DatabaseManager {
            users: UserCollection::new(db.collection("users")),
            games: GameCollection::new(db.collection("games")),
            friend_requests: FriendRequestCollection::new(db.collection("friend_requests")),
        }
    }

    // pub fn friend_requests(&self) -> FriendRequestCollection {
    //     *self.friend_requests.borrow()
    // }
}

pub fn deserialize_vec<T>(cursor: Cursor) -> Vec<T>
where
    T: DeserializeOwned,
{
    cursor
        .collect::<Vec<mongodb::error::Result<Document>>>()
        .iter()
        .filter_map(|res| {
            res.clone()
                .ok()
                .and_then(|bson_data| deserialize(bson_data))
        })
        .collect::<Vec<_>>()
}

pub fn deserialize<T>(doc: Document) -> Option<T>
where
    T: DeserializeOwned,
{
    bson::from_bson::<T>(doc.into()).ok()
}

// impl Actor for DatabaseManager {
//     type Context = Context<Self>;
// }
