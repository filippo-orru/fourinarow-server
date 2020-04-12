// mod routes;

use actix_web::{web, HttpResponse};

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::resource("/")
            .route(web::get().to(HttpResponse::Ok))
            .route(web::head().to(HttpResponse::MethodNotAllowed)),
    )
    // .route("/stats", web::get().to(stats))
    // .service(
    //     web::resource("/shutdown").route(web::post().to(routes::shutdown)), // .route(web::get().to(|| HttpResponse::Ok().body("get ok"))),
    ;
}
