//#![allow(dead_code)]
mod client_conn;
mod client_state;
mod game;
mod lobby;
mod lobby_mgr;
mod msg;

use client_conn::ClientConnection;
use lobby_mgr::LobbyManager;

use actix::{Actor, Addr};
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;

//use rustls::internal::pemfile::{certs, rsa_private_keys};
//use rustls::{NoClientAuth, ServerConfig};
use std::fs::File;
use std::io::BufReader;

const BIND_ADDR: &str = "0.0.0.0:40146";

#[actix_rt::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "actix_web=info");
    }
    env_logger::init();

    let lobby_mgr_addr = LobbyManager::new().start();

    /*    let mut config = ServerConfig::new(NoClientAuth::new());
        let cert_file = &mut BufReader::new(File::open("server.crt").unwrap());
        let key_file = &mut BufReader::new(File::open("server.pem").unwrap());
        let cert_chain = certs(cert_file).unwrap();
        let mut keys = rsa_private_keys(key_file).unwrap();
        config.set_single_cert(cert_chain, keys.remove(0)).unwrap();
    */
    println!("Running on {}.", BIND_ADDR);
    let _close_res = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .route("/", web::get().to(|| HttpResponse::from("working fine")))
            .route(
                "/ws/hello",
                web::get().to(|| HttpResponse::from("WS working fine")),
            )
            .data(lobby_mgr_addr.clone())
            .service(web::resource("/ws/").to(websocket_route))
    })
    //    .bind_rustls(BIND_ADDR, config)
    .bind(BIND_ADDR)
    .unwrap()
    .run()
    .await;

    // if close_res.is_err() {
    // }
}

async fn websocket_route(
    req: HttpRequest,
    stream: web::Payload,
    lobby_mgr: web::Data<Addr<LobbyManager>>,
) -> Result<HttpResponse, Error> {
    ws::start(
        ClientConnection::new(lobby_mgr.get_ref().clone()),
        &req,
        stream,
    )
}
