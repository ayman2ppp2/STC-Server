use actix_session::Session;
use actix_web::{HttpResponse, Responder};

pub async fn home() -> impl Responder {
    html(include_str!("../static/home.html"))
}

pub async fn e_invoicing_page(session: Session) -> HttpResponse {
    if session.get::<String>("tin").ok().flatten().is_none() {
        return HttpResponse::Found()
            .append_header(("Location", "/e-invoicing/login"))
            .finish();
    }
    html(include_str!("../static/e_invoicing.html"))
}

pub async fn login_page() -> impl Responder {
    html(include_str!("../static/e_invoicing_login.html"))
}

pub async fn sandbox_page() -> impl Responder {
    html(include_str!("../static/sandbox.html"))
}

fn html(body: &'static str) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
}
