
use crate::{config::{db_config, xsd_config::SchemaValidator}, routes::{enroll::enroll, invoice_controller::{clearance, reporting}, on_boarding::on_board, token_generator::token_generator, verify_qr::verify_qr}};
use actix_web::{App, HttpResponse, HttpServer, Responder, web};

use config::crypto_config::Crypto;
use sqlx::PgPool;

use crate::routes::{health_check::health_check, };
mod config;
mod models;
mod routes;
mod services;
async fn hello() -> impl Responder {
    "Hello from STC Actix server!\n"
}

async fn get_invoices(db: web::Data<PgPool>) -> impl Responder {
    let result = sqlx::query(
        r#"
        SELECT
            *
        FROM invoices
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(db.get_ref())
    .await;

    match result {
        Ok(invoices) => HttpResponse::Ok().json(invoices.len()),
        Err(e) => {
            eprintln!("DB error: {:?}", e);
            HttpResponse::InternalServerError().body("Failed to fetch invoices")
        }
    }
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

    let pool = db_config::db_from_env().await
        .unwrap_or_else(|e| panic!("Failed to connect to Postgres: {}",e));

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let crypto_config = match Crypto::from_env().await {
        Ok(crypto_config) => crypto_config,
        Err(e) => panic!("Error in the reading of the crypto_config from env :{}", e),
    };
    let validator = SchemaValidator::new().unwrap_or_else(|e| panic!("failed to obtain the schema validator path : {}",e));
    let crypto_data = web::Data::new(crypto_config);
    let pool_data = web::Data::new(pool);
    let validator = web::Data::new(validator);
    HttpServer::new(move || {
        App::new()
            .app_data(validator.clone())
            .app_data(pool_data.clone())
            .app_data(crypto_data.clone())
            .app_data(web::JsonConfig::default().limit(256 * 1024))
            .route("/", web::get().to(hello))
            .route("/health_check", web::get().to(health_check))
            .route("/clear", web::post().to(clearance))
            .route("/reporting", web::post().to(reporting))
            .route("/enroll", web::post().to(enroll))
            .route("/onboard", web::get().to(on_board))
            .route("/onboard", web::post().to(token_generator))
            .route("/get_invoices", web::get().to(get_invoices))
            .route("/verify_qr", web::post().to(verify_qr))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
