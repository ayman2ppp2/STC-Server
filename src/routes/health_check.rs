use actix_web::{HttpResponse, Responder};

pub async fn health_check() -> impl Responder {
    HttpResponse::Ok()
}

pub async fn hello() -> impl Responder {
    "Hello from STC Actix server!\n"
}
