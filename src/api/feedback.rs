use std::sync::Arc;

/// Send feedback to me via mail
use actix_web::{web, HttpResponse};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, SmtpTransport, Transport};
use serde::Deserialize;

use crate::database::DatabaseManager;

use super::users::user::UserId;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("", web::post().to(create_feedback));
}

async fn create_feedback(
    db: web::Data<Arc<DatabaseManager>>,
    feedback: web::Json<Feedback>,
) -> HttpResponse {
    let maybe_user_name = feedback
        .user_id
        .and_then(|id| db.users.get_id_public(&id).await)
        .map(|user| user.username.clone());

    actix::spawn(send_feedback_mail_wrap(
        maybe_user_name,
        feedback.content.clone(),
    ));

    HttpResponse::Ok().finish()
}

async fn send_feedback_mail_wrap(maybe_user_name: Option<String>, content: String) {
    // Idk I need this wrapper because actix::spawn needs result = () and the send_feedback_mail
    //  has Option<()> to easily unwrap the env vars
    if send_feedback_mail(maybe_user_name, content).await.is_none() {
        println!("Error sending mail. Environment variables are missing.");
    }
}

async fn send_feedback_mail(maybe_user_name: Option<String>, content: String) -> Option<()> {
    let from_mail = std::env::var("GMAIL_MAIL_FROM").ok()?;
    let to_mail = std::env::var("GMAIL_MAIL_TO").ok()?;
    let password = std::env::var("GMAIL_PW").ok()?;

    let body = if let Some(user) = maybe_user_name {
        format!(
            "The user \"{}\" left the following feedback:\n{}",
            user, content
        )
    } else {
        format!("A guest user left the following feedback:\n{}", content)
    };
    let email = Message::builder()
        .from(
            format!("Four in a Row Feedback <{}>", from_mail)
                .parse()
                .unwrap(),
        )
        .to(format!("Filippo Orru <{}>", to_mail).parse().unwrap())
        .subject("New feedback for Four in a Row!")
        .body(body)
        .unwrap();

    let creds = Credentials::new(from_mail, password);

    // Open a remote connection to gmail
    let mailer = SmtpTransport::relay("smtp.gmail.com")
        .unwrap()
        .credentials(creds)
        .build();

    // Send the email
    match mailer.send(&email) {
        Ok(_) => println!("Feedback email sent successfully!"),
        Err(e) => println!("Could not send feedback email: {:?}", e),
    }

    Some(())
}

#[derive(Deserialize, Debug, Clone)]
struct Feedback {
    user_id: Option<UserId>,
    content: String,
}
