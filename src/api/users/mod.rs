pub mod session_token;
pub mod user;
pub mod user_mgr;

use super::{get_session_token, ApiError, ApiResponse};
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
    match user_mgr
        .send(user_mgr::msg::Register(payload.into_inner()))
        .await
    {
        Ok(Ok(session_token)) => HR::Ok().json(ApiResponse::with_content(
            "Registration successful.",
            session_token,
        )),
        Ok(Err(api_err)) => ApiResponse::from(api_err),
        Err(_) => ApiResponse::from(ApiError::InternalServerError),
    }
}

async fn login(
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
    payload: web::Form<user_mgr::UserAuth>,
) -> HttpResponse {
    match user_mgr
        .send(user_mgr::msg::Login(payload.into_inner()))
        .await
    {
        Ok(Ok(session_token)) => HR::Ok().json(ApiResponse::with_content(
            "Login successful.",
            session_token,
        )),
        Ok(Err(api_err)) => ApiResponse::from(api_err),
        Err(_) => ApiResponse::from(ApiError::InternalServerError),
    }
}

async fn logout(
    req: HttpRequest,
    user_mgr: web::Data<Addr<user_mgr::UserManager>>,
) -> HttpResponse {
    match get_session_token(&req) {
        Some(session_token) => match user_mgr.send(user_mgr::msg::Logout(session_token)).await {
            Ok(Ok(_)) => HR::Ok().json(ApiResponse::new("Logout successful.")),
            Ok(Err(api_err)) => ApiResponse::from(api_err),
            _ => ApiResponse::from(ApiError::InternalServerError),
        },
        None => ApiResponse::from(ApiError::MissingSessionToken),
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
        let user_res: Result<Option<Vec<user::PublicUserOther>>, _> = user_mgr
            .send(user_mgr::msg::SearchUsers {
                query: query.search.clone(),
            })
            .await;
        match user_res {
            Ok(Some(users)) => HR::Ok().json(users),
            _ => ApiResponse::from(ApiError::InternalServerError),
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
    match user_res {
        Ok(Some(user)) => HR::Ok().json(user),
        _ => ApiResponse::from(ApiError::InternalServerError),
    }
}

async fn me(req: HttpRequest, user_mgr: web::Data<Addr<user_mgr::UserManager>>) -> HR {
    match get_session_token(&req) {
        Some(session_token) => match user_mgr.send(user_mgr::msg::GetUserMe(session_token)).await {
            Ok(Some(user)) => HR::Ok().json(user),
            Ok(None) => ApiResponse::from(ApiError::IncorrectCredentials),
            Err(_) => ApiResponse::from(ApiError::InternalServerError),
        },
        None => ApiResponse::from(ApiError::MissingSessionToken),
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
        match get_session_token(&req) {
            Some(session_token) => {
                match user_mgr
                    .send(UserAction {
                        action: Action::FriendsAction(action),
                        session_token,
                    })
                    .await
                {
                    Ok(true) => HR::Ok().into(),
                    Ok(false) => ApiResponse::from(ApiError::IncorrectCredentials),
                    Err(_) => ApiResponse::from(ApiError::InternalServerError),
                }
            }
            None => ApiResponse::from(ApiError::MissingSessionToken),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct UserIdQuery {
    id: user::UserId,
}
