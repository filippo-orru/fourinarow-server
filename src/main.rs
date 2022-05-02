mod api;
mod database;
mod game;
mod logging;

use std::io;
use std::sync::Arc;

use actix::Actor;
use actix_cors::Cors;
use actix_files as fs;
use actix_web::http::header;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};

use api::users::user_mgr::UserManager;
use database::DatabaseManager;
use dotenv::dotenv;
use game::connection_mgr::ConnectionManager;
use game::lobby_mgr::LobbyManager;
use logging::Logger;

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:40146";

#[actix_rt::main]
async fn main() {
    dotenv().expect("Failed to load .env file");

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "actix_web=info");
    }
    let args: Vec<String> = std::env::args().collect();
    let bind_addr = if let Some(addr) = args.get(1) {
        addr
    } else {
        DEFAULT_BIND_ADDR
    };

    env_logger::builder()
        .format_timestamp_secs()
        .format_module_path(false)
        .init();

    let server = start_server(&bind_addr);

    let res = server.await;
    println!();
    match res {
        Ok(_) => println!("Server terminated cleanly"),
        Err(err) => println!("Server terminated with an error!.\nErr: {:?}", err,),
    }
}

async fn start_server(bind_addr: &str) -> io::Result<()> {
    println!("Running on {}.", bind_addr);
    let db_mgr = Arc::new(DatabaseManager::new().await);
    let user_mgr_addr = UserManager::new(db_mgr.clone()).start();
    let logger_addr = Logger::new().start();
    let connection_mgr_addr = ConnectionManager::new(logger_addr.clone()).start();
    let lobby_mgr_addr = LobbyManager::new(
        user_mgr_addr.clone(),
        connection_mgr_addr.clone(),
        logger_addr.clone(),
    )
    .start();
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .data(db_mgr.clone())
            .data(lobby_mgr_addr.clone())
            .data(connection_mgr_addr.clone())
            .data(user_mgr_addr.clone())
            .route(
                "/",
                web::get().to(|| {
                    HttpResponse::Found()
                        .header("LOCATION", "/index.html")
                        .finish()
                }),
            )
            .service(
                web::scope("/api")
                    .wrap(
                        Cors::default()
                            .allowed_origin("https://play.fourinarow.ffactory.me")
                            .allowed_origin("localhost")
                            .allowed_methods(vec!["GET", "POST", "DELETE"])
                            .allow_any_header()
                            .max_age(3600),
                    )
                    .configure(api::config),
            )
            .service(web::scope("/game").configure(|cfg| game::config(cfg)))
            .service(fs::Files::new("/", "static/").default_handler(web::to(|| {
                HttpResponse::Found()
                    .header("LOCATION", "/404.html")
                    .finish()
            })))
            .default_service(web::to(HttpResponse::NotFound))
    })
    .keep_alive(1)
    .bind(bind_addr)
    .expect("Failed to bind address.")
    .run()
    .await
}
