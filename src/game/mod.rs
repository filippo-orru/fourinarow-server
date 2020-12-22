pub mod client_adapter;
pub mod client_connection;
pub mod client_state;
pub mod connection_mgr;
pub mod lobby_mgr;
pub mod msg;

mod game_info;
mod lobby;

use crate::api::users::user_mgr::UserManager;
pub use client_connection::ClientConnection;
use lobby_mgr::LobbyManager;

use actix::Addr;
use actix_web::{web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;

use self::connection_mgr::ConnectionManager;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/").to(websocket_route));
}

async fn websocket_route(
    req: HttpRequest,
    stream: web::Payload,
    lobby_mgr: web::Data<Addr<LobbyManager>>,
    user_mgr: web::Data<Addr<UserManager>>,
    connection_mgr: web::Data<Addr<ConnectionManager>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        ClientConnection::new(
            lobby_mgr.get_ref().clone(),
            user_mgr.get_ref().clone(),
            connection_mgr.get_ref().clone(),
        ),
        &req,
        stream,
    )
}
