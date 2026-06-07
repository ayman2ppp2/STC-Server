use actix_web::{HttpResponse, Responder};

#[utoipa::path(
    get,
    path = "/health_check",
    tag = "Public API",
    responses((status = 200, description = "Service is reachable"))
)]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok()
}

pub async fn hello() -> impl Responder {
    "Hello from STC Actix server!\n"
}
