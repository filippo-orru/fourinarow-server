use std::time::SystemTime;

use futures::future::OptionFuture;
use mongodb::Database;
use mongodb::{bson::*, options::FindOptions, Collection};
use serde::{Deserialize, Serialize};
use tokio::stream::StreamExt;

use crate::api::chat::{PostedChatMsg, PublicChatMsg};
use crate::api::users::user::UserId;

pub struct ChatMsgCollection {
    collection: Collection<DbChatMsg>,
}

#[derive(Debug, Deserialize, Serialize)]
struct DbChatMsg {
    thread_id: String,
    id: i64,        // (u64) Monotonically increasing index of messages in this thread_id
    timestamp: i64, // (u64)
    from: Option<UserId>,
    content: String,
}

impl ChatMsgCollection {
    pub fn new(db: &Database) -> Self {
        ChatMsgCollection {
            collection: db.collection_with_type("chat_messages"),
        }
    }

    pub async fn get_messages_in_thread(
        &self,
        thread_id: String,
        maybe_before_id: Option<u64>,
    ) -> Vec<PublicChatMsg> {
        let doc = if let Some(before_id) = maybe_before_id {
            doc! { "thread_id": thread_id, "id": { "$lt": before_id } }
        } else {
            doc! { "thread_id": thread_id}
        };
        let mut options = FindOptions::default();
        options.limit = Some(50);
        let minus_one: i32 = -1;
        options.sort = Some(doc! { "id": minus_one });
        let messages: OptionFuture<_> = self
            .collection
            .find(doc, Some(options))
            .await
            .map(|cursor| {
                cursor
                    .map(|result| {
                        result.map(|db_msg| PublicChatMsg {
                            id: db_msg.id,
                            from: db_msg.from,
                            timestamp: db_msg.timestamp,
                            content: db_msg.content,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .ok()
            .into();
        messages
            .await
            .map(|r| r.ok())
            .flatten()
            .unwrap_or(Vec::new())
    }

    pub async fn add(
        &self,
        thread_id: String,
        from_id: Option<UserId>,
        msg: PostedChatMsg,
    ) -> Result<(), ()> {
        let msg_id = self
            .get_messages_in_thread(thread_id.clone(), None)
            .await
            .first()
            .map_or(0, |m| m.id + 1);

        let db_msg = DbChatMsg::from(thread_id, msg_id, from_id, msg);
        self.collection
            .insert_one(db_msg, None)
            .await
            .map(|_| ())
            .map_err(|_| ())
    }
}

impl DbChatMsg {
    fn from(
        thread_id: String,
        msg_id: i64,
        from_id: Option<UserId>,
        posted_msg: PostedChatMsg,
    ) -> Self {
        DbChatMsg {
            thread_id,
            id: msg_id,
            timestamp: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            from: from_id,
            content: posted_msg.content,
        }
    }
}
