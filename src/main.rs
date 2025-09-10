use actix_web::{get, App, HttpServer, Responder};

#[get("/")]
async fn hello() -> impl Responder {
    "Hello from STC Actix server!"
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Render sets PORT env variable
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a number");

    println!("🚀 Server running on port {}", port);

    HttpServer::new(|| App::new().service(hello))
        .bind(("0.0.0.0", port))?
        .run()
        .await
}
