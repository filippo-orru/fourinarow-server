mod api;
mod game;

use actix::Actor;
use actix_files as fs;
use actix_service::Service;
use actix_web::dev::Server;
use actix_web::http::{header, HeaderValue};
use actix_web::{middleware, web, App, HttpResponse, HttpServer};

use api::users::user_mgr::UserManager;
use game::lobby_mgr::LobbyManager;

const BIND_ADDR: &str = "0.0.0.0:40146";

#[actix_rt::main]
async fn main() {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "actix_web=info");
    }
    env_logger::init();
    let server = start_server();

    match server.await {
        Ok(_) => println!("Server terminated cleanly"),
        Err(err) => println!("Server terminated with an error!.\nErr: {:?}", err,),
    }
}

fn start_server() -> Server {
    println!("Running on {}.", BIND_ADDR);
    let user_mgr_addr = UserManager::new().start();
    let lobby_mgr_addr = LobbyManager::new(user_mgr_addr.clone()).start();
    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(middleware::Compress::default())
            .data(lobby_mgr_addr.clone())
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
                    .wrap_fn(|req, srv| {
                        let fut = srv.call(req);
                        async {
                            let mut res = fut.await?;
                            let headers = res.headers_mut();
                            headers.insert(
                                header::CONTENT_TYPE,
                                HeaderValue::from_static("application/json"),
                            );
                            headers.insert(
                                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                                HeaderValue::from_static("*"),
                            );
                            Ok(res)
                        }
                    })
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
    .bind(BIND_ADDR)
    .unwrap()
    .run()
}
