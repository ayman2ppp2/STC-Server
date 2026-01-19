use actix_web::HttpResponse;

pub async fn on_board() -> Result<HttpResponse, actix_web::error::Error> {
    let html = include_str!("../static/token_form.html");

    Ok(HttpResponse::Ok().content_type("text/html").body(html))
}
