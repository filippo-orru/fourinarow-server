pub mod session_token;
pub mod user;
pub mod user_mgr;

use super::{get_session_token, ApiResponse};
use actix::{Addr, MailboxError};
use actix_web::*;
use serde::{Deserialize, Serialize};
use HttpResponse as HR;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(search_user))
        .route("", web::post().to(register))
        .service(
            web::scope("/me")
                .route("", web::get().to(me))
                .service(web::scope("/friends").configure(friends::config)),
        )
        .route("/register", web::post().to(register))
        .route("/login", web::post().to(login))
        .route("/logout", web::post().to(logout))
        .route("/{user_id}", web::get().to(get_user));
}

async fn register(
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
    payload: web::Form<user_mgr::UserAuth>,
) -> HttpResponse {
    if let Ok(session_token_res) = user_mgr
        .send(user_mgr::msg::Register(payload.into_inner()))
        .await
    {
        match session_token_res {
            Ok(session_token) => HR::Ok().json(ApiResponse::with_content(
                "Registration successful.",
                session_token,
            )),
            Err(api_err) => HR::Forbidden().json(ApiResponse::from_api_error(api_err)),
        }
    } else {
        HR::InternalServerError().json(ApiResponse::new("Registration failed. Internal Error."))
    }
}

async fn login(
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
    payload: web::Form<user_mgr::UserAuth>,
) -> HttpResponse {
    if let Ok(session_token_res) = user_mgr
        .send(user_mgr::msg::Login(payload.into_inner()))
        .await
    {
        match session_token_res {
            Ok(session_token) => HR::Ok().json(ApiResponse::with_content(
                "Login successful.",
                session_token,
            )),
            Err(api_err) => HR::Forbidden().json(ApiResponse::from_api_error(api_err)),
        }
    } else {
        HR::InternalServerError().json(ApiResponse::new("Login failed. Internal Error."))
    }
}

async fn logout(
    req: HttpRequest,
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
) -> HttpResponse {
    if let Some(session_token) = get_session_token(&req) {
        match user_mgr.send(user_mgr::msg::Logout(session_token)).await {
            Ok(Ok(_)) => HR::Ok().json(ApiResponse::new("Logout successful.")),
            Ok(Err(api_err)) => HR::Forbidden().json(ApiResponse::from_api_error(api_err)),
            _ => HR::InternalServerError().json(ApiResponse::new("Logout failed. Internal Error.")),
        }
    } else {
        HR::InternalServerError().json(ApiResponse::new("Logout failed. Internal Error."))
    }
}

#[derive(Serialize, Deserialize)]
struct SearchQuery {
    search: String,
}

async fn search_user(
    req: HttpRequest,
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
    query: web::Query<SearchQuery>,
) -> HR {
    if let Some(session_token) = get_session_token(&req) {
        if query.0.search.len() > 25 && query.0.search.len() < 4 {
            HR::Ok().json(Vec::<user::PublicUserOther>::new())
        } else {
            let user_res: Result<_, MailboxError> = user_mgr
                .send(user_mgr::msg::SearchUsers {
                    session_token,
                    query: query.search.clone(),
                })
                .await;
            if let Ok(users) = user_res {
                HR::Ok().json(users.0)
            } else {
                HR::InternalServerError().json(ApiResponse::new("Failed to retrieve users"))
            }
        }
    } else {
        HR::Unauthorized().json(ApiResponse::new("Missing session token"))
    }
}
async fn get_user(
    req: HttpRequest,
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
    path: web::Path<user::UserId>,
) -> HR {
    if let Some(session_token) = get_session_token(&req) {
        let user_res: Result<_, MailboxError> = user_mgr
            .send(user_mgr::msg::GetUserOther {
                session_token,
                user_id: path.into_inner(),
            })
            .await;
        if let Ok(Some(user)) = user_res {
            HR::Ok().json(user)
        } else {
            HR::InternalServerError().json(ApiResponse::new("Failed to retrieve users"))
        }
    } else {
        HR::Unauthorized().json(ApiResponse::new("Missing session token"))
    }
}

async fn me(req: HttpRequest, user_mgr: web::Data<Addr<user_mgr::UserManager>>) -> HR {
    if let Some(session_token) = get_session_token(&req) {
        let user_res: Result<Option<user::PublicUserMe>, MailboxError> =
            user_mgr.send(user_mgr::msg::GetUserMe(session_token)).await;
        if let Ok(maybe_user) = user_res {
            if let Some(user) = maybe_user {
                HR::Ok().json(user)
            } else {
                HR::Forbidden().json(ApiResponse::new(
                    "Could not find user. Invalid credentials.",
                ))
            }
        } else {
            HR::InternalServerError().json(ApiResponse::new("Failed to retrieve user"))
        }
    } else {
        HR::Forbidden().json(ApiResponse::new("Missing auth header"))
    }
}

mod friends {
    use crate::api::get_session_token;

    use super::*;
    use user::UserId;
    use user_mgr::msg::*;

    pub fn config(cfg: &mut web::ServiceConfig) {
        cfg
            // .route("/", web::get().to(friends::get))
            .route("", web::post().to(friends::post))
            .route("/{id}", web::delete().to(friends::delete));
    }

    /*pub async fn get(
        user_mgr: web::Data<Addr<user_mgr::UserManager>>,
        auth: web::Form<user_mgr::UserAuth>,
    ) -> HR {
        let user_res: Result<bool, MailboxError> = user_mgr
            .send(UserAction {
                action: Action::FriendsAction(action),
                auth,
            })
            .await;
        if let Ok(b) = user_res {
            if b {
                HR::Ok().into()
            } else {
                HR::Forbidden().json(ApiResponse::new(
                    "Could not find user or invalid credentials.",
                ))
            }
        } else {
            HR::InternalServerError().json(ApiResponse::new("Failed to retrieve user"))
        }
    }*/

    pub async fn post(
        req: HttpRequest,
        user_mgr: web::Data<Addr<user_mgr::UserManager>>,
        query: web::Query<UserIdQuery>,
    ) -> HR {
        modify(req, FriendsAction::Request(query.id), user_mgr.get_ref()).await
    }

    pub async fn delete(
        req: HttpRequest,
        user_mgr: web::Data<Addr<user_mgr::UserManager>>,
        id: web::Path<UserId>,
    ) -> HR {
        modify(req, FriendsAction::Delete(id.0), user_mgr.get_ref()).await
    }

    async fn modify(
        req: HttpRequest,
        action: FriendsAction,
        user_mgr: &Addr<user_mgr::UserManager>,
    ) -> HR {
        if let Some(session_token) = get_session_token(&req) {
            let user_res: Result<bool, MailboxError> = user_mgr
                .send(UserAction {
                    action: Action::FriendsAction(action),
                    session_token,
                })
                .await;
            if let Ok(b) = user_res {
                if b {
                    HR::Ok().into()
                } else {
                    HR::Forbidden().json(ApiResponse::new(
                        "Could not find user or invalid credentials.",
                    ))
                }
            } else {
                HR::InternalServerError().json(ApiResponse::new("Failed to retrieve user"))
            }
        } else {
            HR::Unauthorized().finish()
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct UserIdQuery {
    id: user::UserId,
}
