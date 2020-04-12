use crate::game::lobby_mgr::*;

use actix::Addr;
use actix_web::{web, Error, HttpRequest, HttpResponse};

// pub async fn shutdown(
//     _req: HttpRequest,
//     lobby_mgr: web::Data<Addr<LobbyManager>>,
// ) -> Result<HttpResponse, Error> {
//     lobby_mgr.do_send(LobbyManagerMsg::Shutdown);
//     Ok("Okay".into())
// }

// pub async fn stats(
//     _req: HttpRequest,
//     lobby_mgr: web::Data<Addr<LobbyManager>>,
// ) -> Result<HttpResponse, Error> {
//     let lobbies_info_res = lobby_mgr.send(GetInfo).await;
//     if let Ok(lobbies_info) = lobbies_info_res {
//         Ok(format!("Active lobbies: {}", lobbies_info.len()).into())
//     } else {
//         Ok("error".into())
//     }
//     // .into_actor().then(|mail_res, _, _| {

//     // }).wait(ctx);
// }
