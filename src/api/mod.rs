pub mod chat;
mod feedback;
pub mod users;

use actix_web::{web, HttpRequest, HttpResponse};
use serde::Serialize;

use self::users::session_token::SessionToken;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/")
            .route(web::get().to(HttpResponse::Ok))
            .route(web::head().to(HttpResponse::MethodNotAllowed)),
    )
    .service(web::scope("/users").configure(users::config))
    .service(web::scope("/chat").configure(chat::config))
    .service(web::scope("/feedback").configure(feedback::config));
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
            String::from("Access failed")
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

pub fn get_session_token(req: &HttpRequest) -> Option<SessionToken> {
    req.headers()
        .get("session_token")
        .map(|s| s.to_str().ok().map(|s| SessionToken::parse(s)))
        .flatten()
}
