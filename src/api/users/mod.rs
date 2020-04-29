pub mod user;
pub mod user_manager;

use super::ApiResponse;
use actix::{Addr, MailboxError};
use actix_web::*;
use HttpResponse as HR;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(users))
        .route("/register", web::post().to(register))
        .route("/login", web::post().to(login));
    // .service(
    //     web::scope("/account")
    // .route("/register", web::post().to(register))
    // .route("/login", web::post().to(login)),
    // ;
}

async fn register(
    _req: HttpRequest,
    register_payload: web::Json<user_manager::UserInfoPayload>,
    user_mgr: web::Data<Addr<user_manager::UserManager>>,
) -> HttpResponse {
    if let Ok(reg_res) = user_mgr
        .send(user_manager::msg::Register(register_payload.into_inner()))
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
    register_payload: web::Json<user_manager::UserInfoPayload>,
    user_mgr: web::Data<Addr<user_manager::UserManager>>,
) -> HttpResponse {
    if let Ok(msg_res) = user_mgr
        .send(user_manager::msg::Login(register_payload.into_inner()))
        .await
    {
        if let Ok(_) = msg_res {
        HR::Ok().json(ApiResponse::new("Login successful."))
        } else {
            HR::Forbidden().json(ApiResponse::new("Login failed."))
        }
    } else {
        HR::InternalServerError().json(ApiResponse::new("Login failed. Internal Error."))
    }
}

async fn users(
    _: HttpRequest,
    user_mgr: web::Data<Addr<user_manager::UserManager>>,
) -> HttpResponse {
    let users_res: Result<Option<Vec<user::User>>, MailboxError> =
        user_mgr.send(user_manager::msg::GetUsers).await;
    if let Ok(Some(users)) = users_res {
        HttpResponse::Ok().json(users)
    } else {
        HttpResponse::InternalServerError().json(ApiResponse::new("Failed to retrieve users"))
    }
}
