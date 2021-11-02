use std::time::SystemTime;

use mongodb::{
    bson::{self, doc},
    options::FindOptions,
    sync::Collection,
};
use serde::{Deserialize, Serialize};

use super::deserialize_vec;
use crate::api::chat::{PostedChatMsg, PublicChatMsg};
use crate::api::users::user::UserId;

const CHAT_MESSAGES_PER_REQUEST_LIMIT: usize = 50;

pub struct ChatMsgCollection {
    pub collection: Collection,
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
    pub fn new(collection: Collection) -> Self {
        ChatMsgCollection { collection }
    }

    pub fn get_messages_in_thread(
        &self,
        thread_id: String,
        maybe_before_id: Option<u64>,
    ) -> ChatGetMessagesResponse {
        let doc = if let Some(before_id) = maybe_before_id {
            doc! { "thread_id": thread_id, "id": { "$lt": before_id } }
        } else {
            doc! { "thread_id": thread_id}
        };
        let mut options = FindOptions::default();
        options.limit = Some(CHAT_MESSAGES_PER_REQUEST_LIMIT as i64);
        options.sort = Some(doc! { "id": -1 });
        let messages = self
            .collection
            .find(doc, Some(options))
            .map(|cursor| deserialize_vec::<DbChatMsg>(cursor))
            .map(|db_msgs| {
                db_msgs
                    .into_iter()
                    .map(|db_msg| db_msg.to_public())
                    .collect()
            })
            .unwrap_or(Vec::new());

        ChatGetMessagesResponse {
            more_messages_available: messages.len() == CHAT_MESSAGES_PER_REQUEST_LIMIT,
            messages,
        }
    }

    pub fn insert(
        &self,
        thread_id: String,
        from_id: Option<UserId>,
        msg: PostedChatMsg,
    ) -> Result<PublicChatMsg, ()> {
        let msg_id = self
            .get_messages_in_thread(thread_id.clone(), None)
            .messages
            .first()
            .map_or(0, |m| m.msg_id + 1);

        let db_msg = DbChatMsg::from(thread_id, msg_id, from_id, msg);
        let doc = bson::to_document(&db_msg).unwrap();
        self.collection
            .insert_one(doc, None)
            .map(|_| db_msg.to_public())
            .map_err(|_| ())
    }
}

#[derive(Debug, Serialize)]
pub struct ChatGetMessagesResponse {
    messages: Vec<PublicChatMsg>,
    more_messages_available: bool,
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

    fn to_public(self) -> PublicChatMsg {
        PublicChatMsg {
            msg_id: self.id,
            from: self.from,
            timestamp: self.timestamp,
            content: self.content,
        }
    }
}
