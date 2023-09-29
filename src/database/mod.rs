pub mod chat_msg;
pub mod friendships;
pub mod games;
pub mod users;

use mongodb::{options::ClientOptions, Client};

use self::{
    chat_msg::ChatMsgCollection, friendships::FriendshipCollection, games::GameCollection,
    users::UserCollection,
};

const MONGO_URL_DEFAULT: &str = "mongodb://localhost:27017";

pub struct DatabaseManager {
    pub users: UserCollection,
    pub games: GameCollection,
    pub friendships: FriendshipCollection,
    pub chat_msgs: ChatMsgCollection,
}

impl DatabaseManager {
    pub async fn new() -> DatabaseManager {
        let url = std::env::var("MONGO_URL").unwrap_or(MONGO_URL_DEFAULT.to_string());
        println!("Connecting to mongodb at '{}'", url);
        let opt = ClientOptions::parse(&url).await.unwrap();
        let client = Client::with_options(opt).expect("Failed to start mongodb client");
        let db = client.database("fourinarow");

        DatabaseManager {
            users: UserCollection::new(&db),
            games: GameCollection::new(&db),
            friendships: FriendshipCollection::new(&db),
            chat_msgs: ChatMsgCollection::new(&db),
        }
    }

    // pub fn friend_requests(&self) -> FriendRequestCollection {
    //     *self.friend_requests.borrow()
    // }
}
/*
pub fn deserialize_vec<T>(cursor: Cursor<T>) -> Vec<T>
where
    T: DeserializeOwned + Unpin + Send + Sync,
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
*/
// impl Actor for DatabaseManager {
//     type Context = Context<Self>;
// }
