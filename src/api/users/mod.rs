pub mod user;
pub mod user_manager;

use super::ApiResponse;
use actix::{Addr, MailboxError};
use actix_web::*;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg
        .route("/", web::get().to(users))        
        .route("/register", web::post().to(register)
        // .service(
        //     web::scope("/account")
            // .route("/login", web::post().to(login))
                // .route("/register", web::post().to(register))
                // .route("/login", web::post().to(login)),
        );
}

async fn register(
    _req: HttpRequest,
    register_payload: web::Json<user_manager::UserInfoPayload>,
    user_mgr: web::Data<Addr<user_manager::UserManager>>,
) -> HttpResponse {
    use HttpResponse as HR;
    if let Ok(reg_res) = user_mgr
        .send(user_manager::msg::Register(register_payload.into_inner()))
        .await
    {
        match reg_res {
            Ok(_) => HR::Ok().json(ApiResponse::new("Registration successful")),
            Err(api_err) => HR::Forbidden().json(ApiResponse::from_api_error(api_err)),
        }
    } else {
        HR::Forbidden().json(ApiResponse::new("Registration failed"))
    }
}

// async fn login(
//     _req: HttpRequest,
//     register_payload: web::Json<UserInfoPayload>,
//     user_mgr: web::Data<Addr<UserManager>>,
// ) -> HttpResponse {
//     if let Ok(Ok(_)) = user_mgr
//         .send(StartPlaying(register_payload.into_inner(), false))
//         .await
//     {
//         HttpResponse::Ok().json(ApiResponse::new("Login successful"))
//     // Ok("Logged in".into())
//     } else {
//         HttpResponse::Forbidden().json(ApiResponse::new("Login failed"))
//     }
// }

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
