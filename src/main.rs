use crate::routes::{enroll::enroll, on_boarding::on_board, token_generator::token_generator};
use actix_web::{App, HttpResponse, HttpServer, Responder, web};

use config::crypto_config::Crypto;
use sqlx::PgPool;

use crate::routes::{health_check::health_check, submit_invoice::submit_invoice};
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

    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let db_user = std::env::var("POSTGRES_USER").unwrap_or_else(|_| "postgres".to_string());
        let db_password =
            std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "password".to_string());
        let db_name = std::env::var("POSTGRES_DB").unwrap_or_else(|_| "stc-server".to_string());
        let db_host = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
        let db_port = std::env::var("POSTGRES_PORT").unwrap_or_else(|_| "5432".to_string());
        format!(
            "postgres://{}:{}@{}:{}/{}",
            db_user, db_password, db_host, db_port, db_name
        )
    });

    // println!("PORT = {:?}", std::env::var("PORT"));
    // println!("DATABASE_URL = {:?}", std::env::var("DATABASE_URL"));

    let pool = PgPool::connect(&database_url)
        .await
        .unwrap_or_else(|_| panic!("Failed to connect to Postgres: {}", database_url));

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    let crypto_config = match Crypto::from_env().await {
        Ok(crypto_config) => crypto_config,
        Err(e) => panic!("error in the reading of the crypto_config from env :{}", e),
    };
    let crypto_data = web::Data::new(crypto_config);
    print!("this is just a silly change to see if again image build time is reasonable");
    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .app_data(crypto_data.clone())
            .app_data(web::JsonConfig::default().limit(256 * 1024))
            .route("/", web::get().to(hello))
            .route("/health_check", web::get().to(health_check))
            .route("/submit_invoice", web::post().to(submit_invoice))
            .route("/enroll", web::post().to(enroll))
            .route("/onboard", web::get().to(on_board))
            .route("/onboard", web::post().to(token_generator))
            .route("/get_invoices", web::get().to(get_invoices))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
