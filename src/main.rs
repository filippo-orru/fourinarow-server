mod api;
mod game;

use actix::Actor;
use actix_files as fs;
use actix_web::{middleware, web, App, HttpResponse, HttpServer};

const BIND_ADDR: &str = "0.0.0.0:40146";

#[actix_rt::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "actix_web=info");
    }
    env_logger::init();

    println!("Running on {}.", BIND_ADDR);
    let lobby_mgr_addr = game::lobby_mgr::LobbyManager::new().start();
    let _close_res = HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .data(lobby_mgr_addr.clone())
            .route(
                "/",
                web::get().to(|| {
                    HttpResponse::Found()
                        .header("LOCATION", "/index.html")
                        .finish()
                }),
            )
            .service(web::scope("/api").configure(api::config))
            .service(web::scope("/game").configure(|cfg| game::config(cfg)))
            .service(fs::Files::new("/", "static/"))
            .default_service(web::to(HttpResponse::NotFound))
    })
    .bind(BIND_ADDR)
    .unwrap()
    .run()
    .await;
}
