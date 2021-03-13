// mod routes;

pub mod users;

use actix_web::{web, HttpResponse};
use serde::Serialize;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/")
            .route(web::get().to(HttpResponse::Ok))
            .route(web::head().to(HttpResponse::MethodNotAllowed)),
    )
    .service(web::scope("/users").configure(users::config));
    // .route("/stats", web::get().to(stats))
    // .service(
    //     web::resource("/shutdown").route(web::post().to(routes::shutdown)), // .route(web::get().to(|| HttpResponse::Ok().body("get ok"))),
}

#[derive(Serialize)]
pub struct ApiResponse<T> {
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<T>,
}
impl ApiResponse<()> {
    pub fn new<T: Into<String>>(message: T) -> Self {
        ApiResponse {
            message: message.into(),
            content: None,
        }
    }
    #[allow(unreachable_patterns)]
    pub fn from_api_error(err: ApiError) -> Self {
        ApiResponse::new(
            String::from("Registration failed")
                + match err {
                    ApiError::PasswordInsufficient => ": insufficient password",
                    ApiError::EmailInUse => ": email in use",
                    ApiError::UsernameInUse => ": username in use",
                    ApiError::InvalidUsername => {
                        ": username invalid (too short, long or containing invalid characters)"
                    }
                    ApiError::AlreadyPlaying => ": user is already playing",
                    ApiError::IncorrectCredentials => ": the credentials are incorrect",
                    ApiError::InternalServerError => ": internal server error",
                    _ => "",
                },
        )
    }
}

impl<T> ApiResponse<T> {
    #[allow(dead_code)]
    pub fn with_content(message: &str, content: T) -> Self {
        ApiResponse {
            message: message.to_owned(),
            content: Some(content),
        }
    }
}

#[allow(dead_code)]
#[non_exhaustive]
pub enum ApiError {
    UsernameInUse,
    EmailInUse,
    PasswordInsufficient,
    InvalidUsername,
    IncorrectCredentials,
    AlreadyPlaying,
    InternalServerError,
}
