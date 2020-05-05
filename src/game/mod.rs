pub mod client_conn;
mod client_state;
mod game_info;
mod lobby;
pub mod lobby_mgr;
pub mod msg;

use crate::api::users::user_mgr::UserManager;
pub use client_conn::ClientConnection;
use lobby_mgr::LobbyManager;

use actix::Addr;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/").to(websocket_route));
}

async fn websocket_route(
    req: HttpRequest,
    stream: web::Payload,
    lobby_mgr: web::Data<Addr<LobbyManager>>,
    user_mgr: web::Data<Addr<UserManager>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        ClientConnection::new(lobby_mgr.get_ref().clone(), user_mgr.get_ref().clone()),
        &req,
        stream,
    )
}
