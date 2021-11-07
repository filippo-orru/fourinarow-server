use actix_web::{web, HttpRequest, HttpResponse};
use futures::future::OptionFuture;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{api::users::user::UserId, database::DatabaseManager};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublicChatMsg {
    pub id: i64, // Monotonically increasing index of messages in this thread_id
    pub content: String,
    pub timestamp: i64,
    pub from: Option<UserId>,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/{thread_id}", web::get().to(get_messages_by_thread_id))
        .route("/{thread_id}", web::post().to(post_chat_msg));
}
#[derive(Deserialize)]
struct BeforeIdQuery {
    before_id: Option<u64>,
}
async fn get_messages_by_thread_id(
    db_mgr: web::Data<Arc<DatabaseManager>>,
    web::Path(thread_id): web::Path<String>,
    web::Query(query): web::Query<BeforeIdQuery>,
) -> HttpResponse {
    HttpResponse::Ok().json(
        db_mgr
            .chat_msgs
            .get_messages_in_thread(thread_id, query.before_id)
            .await,
    )
}

#[derive(Deserialize)]
pub struct PostedChatMsg {
    pub content: String,
}

async fn post_chat_msg(
    req: HttpRequest,
    db_mgr: web::Data<Arc<DatabaseManager>>,
    web::Path(thread_id): web::Path<String>,
    web::Json(msg): web::Json<PostedChatMsg>,
) -> HttpResponse {
    let user: OptionFuture<_> = get_session_token(&req)
        .map(|session_token| {
            db_mgr
                .users
                .get_session_token(session_token, &db_mgr.friendships)
        })
        .into();
    let user_id = user.await.flatten().map(|u| u.id);

    if let Ok(_) = db_mgr.chat_msgs.add(thread_id, user_id, msg).await {
        HttpResponse::Ok()
    } else {
        HttpResponse::InternalServerError()
    }
    .finish()
}

pub use chat_thread_id::*;

use super::get_session_token;
mod chat_thread_id {
    use rand::{distributions::Alphanumeric, thread_rng, Rng};
    use serde::{Deserialize, Serialize};
    use std::fmt;

    #[derive(Deserialize, Serialize, Clone, Debug, Hash, PartialEq, Eq)]
    pub struct ChatThreadId(String);

    impl ChatThreadId {
        pub fn new() -> ChatThreadId {
            ChatThreadId(
                thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(16)
                    .map(char::from)
                    .collect::<String>(),
            )
        }

        pub fn parse(text: &str) -> ChatThreadId {
            ChatThreadId(text.to_string())
        }
    }
    impl fmt::Display for ChatThreadId {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(&self.0)
        }
    }

    impl From<&str> for ChatThreadId {
        fn from(s: &str) -> Self {
            ChatThreadId::parse(s)
        }
    }

    impl From<String> for ChatThreadId {
        fn from(s: String) -> Self {
            ChatThreadId::parse(&s)
        }
    }
}
