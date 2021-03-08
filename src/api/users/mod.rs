pub mod user;
pub mod user_mgr;

use super::ApiResponse;
use actix::{Addr, MailboxError};
use actix_web::*;
use serde::{Deserialize, Serialize};
use HttpResponse as HR;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::get().to(search_user))
        .route("", web::post().to(register))
        .service(
            web::scope("/me")
                .route(
                    "",
                    web::get()
                        .guard(guard::fn_guard(|head| {
                            head.headers().contains_key("Authorization")
                        }))
                        .to(me_headers),
                )
                .route("", web::get().to(me))
                .service(web::scope("/friends").configure(friends::config)),
        )
        .route("/{user_id}", web::get().to(get_user))
        .route("/register", web::post().to(register))
        .route("/login", web::post().to(login));
}

async fn register(
    _req: HttpRequest,
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
    payload: web::Form<user_mgr::UserAuth>,
) -> HttpResponse {
    if let Ok(reg_res) = user_mgr
        .send(user_mgr::msg::Register(payload.into_inner()))
        .await
    {
        match reg_res {
            Ok(_) => HR::Ok().json(ApiResponse::new("Registration successful.")),
            Err(api_err) => HR::Forbidden().json(ApiResponse::from_api_error(api_err)),
        }
    } else {
        HR::InternalServerError().json(ApiResponse::new("Registration failed. Internal Error."))
    }
}

async fn login(
    _req: HttpRequest,
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
    payload: web::Form<user_mgr::UserAuth>,
) -> HttpResponse {
    if let Ok(msg_res) = user_mgr
        .send(user_mgr::msg::Login(payload.into_inner()))
        .await
    {
        if msg_res.is_ok() {
            HR::Ok().json(ApiResponse::new("Login successful."))
        } else {
            HR::Forbidden().json(ApiResponse::new("Login failed."))
        }
    } else {
        HR::InternalServerError().json(ApiResponse::new("Login failed. Internal Error."))
    }
}

#[derive(Serialize, Deserialize)]
struct SearchQuery {
    search: String,
}

async fn search_user(
    _: HttpRequest,
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
    query: web::Query<SearchQuery>,
) -> HR {
    if query.0.search.len() > 25 && query.0.search.len() < 4 {
        HR::Ok().json(Vec::<user::PublicUserOther>::new())
    } else {
        let user_res: Result<Option<Vec<user::PublicUserOther>>, MailboxError> = user_mgr
            .send(user_mgr::msg::SearchUsers(query.search.clone()))
            .await;
        if let Ok(Some(users)) = user_res {
            HR::Ok().json(users)
        } else {
            HR::InternalServerError().json(ApiResponse::new("Failed to retrieve users"))
        }
    }
}
async fn get_user(
    _: HttpRequest,
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
    path: web::Path<user::UserId>,
) -> HR {
    let user_res: Result<Option<user::PublicUserOther>, MailboxError> = user_mgr
        .send(user_mgr::msg::GetUserOther(path.into_inner()))
        .await;
    if let Ok(Some(user)) = user_res {
        HR::Ok().json(user)
    } else {
        HR::InternalServerError().json(ApiResponse::new("Failed to retrieve users"))
    }
}

async fn me(
    _: HttpRequest,
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
    payload: web::Form<user_mgr::UserAuth>,
) -> HR {
    let user_res: Result<Option<user::PublicUserMe>, MailboxError> = user_mgr
        .send(user_mgr::msg::GetUserMe(payload.into_inner()))
        .await;
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
}

/// Method that allows access to /me endpoint using basic auth headers instead of
/// x-www-form-urlencoded
///
/// Logic: get header, convert to str, split into "Basic" and auth, decode auth using
/// base64, split into "username":"password" and use that to authenticate
async fn me_headers(req: HttpRequest, user_mgr: web::Data<Addr<user_mgr::UserManager>>) -> HR {
    if let Some(Ok(auth)) = req.headers().get("Authorization").map(|a| a.to_str()) {
        let parts = auth.split(' ').collect::<Vec<_>>();
        if parts.len() == 2 && parts[0] == "Basic" {
            if let Ok(Ok(uname_pw)) = base64::decode(parts[1]).map(String::from_utf8) {
                if let [username, password] = uname_pw.split(':').collect::<Vec<_>>()[0..=1] {
                    let user_res: Result<Option<user::PublicUserMe>, MailboxError> = user_mgr
                        .send(user_mgr::msg::GetUserMe(user_mgr::UserAuth::new(
                            username.to_owned(),
                            password.to_owned(),
                        )))
                        .await;
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
                    HR::Forbidden().json(ApiResponse::new("Invalid username:pw encoding"))
                }
            } else {
                HR::Forbidden().json(ApiResponse::new("Invalid base64"))
            }
        } else {
            HR::Forbidden().json(ApiResponse::new("Missing auth header"))
        }
    } else {
        HR::Forbidden().json(ApiResponse::new("Missing auth header"))
    }
}

mod friends {
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
        user_mgr: web::Data<Addr<user_mgr::UserManager>>,
        auth: web::Form<user_mgr::UserAuth>,
        query: web::Query<UserIdQuery>,
    ) -> HR {
        modify(
            FriendsAction::Request(query.id),
            user_mgr.get_ref(),
            auth.into_inner(),
        )
        .await
    }

    pub async fn delete(
        user_mgr: web::Data<Addr<user_mgr::UserManager>>,
        auth: web::Form<user_mgr::UserAuth>,
        id: web::Path<UserId>,
    ) -> HR {
        modify(
            FriendsAction::Delete(id.0),
            user_mgr.get_ref(),
            auth.into_inner(),
        )
        .await
    }

    async fn modify(
        action: FriendsAction,
        user_mgr: &Addr<user_mgr::UserManager>,
        auth: user_mgr::UserAuth,
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
    }
}

#[derive(Serialize, Deserialize)]
pub struct UserIdQuery {
    id: user::UserId,
}
