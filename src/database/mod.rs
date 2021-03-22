pub mod chat_msg;
pub mod friendships;
pub mod games;
pub mod users;

use mongodb::{
    bson::{self, Document},
    options::ClientOptions,
    sync::{Client, Cursor},
};
use serde::de::DeserializeOwned;

use self::{
    chat_msg::ChatMsgCollection, friendships::FriendshipCollection, games::GameCollection,
    users::UserCollection,
};

const DB_URL: &str = "mongodb://localhost:27017";

pub struct DatabaseManager {
    pub users: UserCollection,
    pub games: GameCollection,
    pub friendships: FriendshipCollection,
    pub chat_msgs: ChatMsgCollection,
}

impl DatabaseManager {
    pub async fn new() -> DatabaseManager {
        let opt = ClientOptions::parse(DB_URL).unwrap();
        let client = Client::with_options(opt).expect("Failed to start mongodb client");
        let db = client.database("fourinarow");

        DatabaseManager {
            users: UserCollection::new(db.collection("users")),
            games: GameCollection::new(db.collection("games")),
            friendships: FriendshipCollection::new(db.collection("friendships")),
            chat_msgs: ChatMsgCollection::new(db.collection("chat_messages")),
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
