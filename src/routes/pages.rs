use actix_web::{HttpResponse, Responder};

pub async fn home() -> impl Responder {
    html(include_str!("../static/home.html"))
}

pub async fn e_invoicing_page() -> impl Responder {
    html(include_str!("../static/e_invoicing.html"))
}

pub async fn sandbox_page() -> impl Responder {
    html(include_str!("../static/sandbox.html"))
}

pub async fn api_page() -> impl Responder {
    html(include_str!("../static/api.html"))
}

fn html(body: &'static str) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
}
