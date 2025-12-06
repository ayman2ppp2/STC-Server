use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Responder, web};

async fn hello() -> impl Responder {
    "Hello from STC Actix server!\n"
}
async fn health_check() -> impl Responder {
    HttpResponse::Ok()
}
async fn submit_invoice(req: HttpRequest) -> impl Responder {
    let length = req
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("0")
        .to_string();
    HttpResponse::Ok().body(format!("Received invoice with content-length: {}", length))
}
// this is the main function
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Render sets PORT env variable
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a number");

    println!("🚀 Server running on port {}", port);

    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(hello))
            .route("/health_check", web::get().to(health_check))
            .route("/submit_invoice", web::get().to(submit_invoice))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
