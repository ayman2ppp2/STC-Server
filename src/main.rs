use crate::{
    config::{db_config, xsd_config::{schema_validator_from_temp}},
    routes::{
        enroll::enroll,
        invoice_controller::{clearance, reporting},
        on_boarding::on_board,
        token_generator::token_generator,
        verify_qr::verify_qr,
    },
};
use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use tracing_actix_web::TracingLogger;

use config::crypto_config::Crypto;
use sqlx::PgPool;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
use tracing_subscriber::fmt::format::FmtSpan;

use crate::routes::health_check::health_check;
use crate::services::token_checking::cleanup_expired_tokens;
use tracing::instrument;
mod config;
mod models;
mod routes;
mod services;

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));
//test
    let fmt_layer = fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_span_events(FmtSpan::FULL);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();
}
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
            tracing::error!(?e, "DB error in get_invoices");
            HttpResponse::InternalServerError().body("Failed to fetch invoices")
        }
    }
}

#[instrument(skip(pool))]
async fn token_cleanup_loop(pool: PgPool) {
    use tokio::time::{Duration, interval};

    let mut cleanup_interval = interval(Duration::from_secs(3600));
    loop {
        cleanup_interval.tick().await;

        match cleanup_expired_tokens(&pool).await {
            Ok(count) if count > 0 => tracing::info!(count, "Cleaned expired tokens"),
            Ok(_) => {}
            Err(e) => tracing::error!(%e, "Token cleanup failed"),
        }
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    init_tracing();

    // Render sets PORT env variable
    let port: u16 = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .expect("PORT must be a number");

    tracing::info!(port, "Server starting");

    let pool = db_config::db_from_env()
        .await
        .unwrap_or_else(|e| panic!("Failed to connect to Postgres: {}", e));

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    // Spawn background task for token cleanup
    tokio::spawn(token_cleanup_loop(pool.clone()));

    let crypto_config = match Crypto::from_env().await {
        Ok(crypto_config) => crypto_config,
        Err(e) => panic!("Error in the reading of the crypto_config from env :{}", e),
    };
    let xsd_schema = schema_validator_from_temp()
        .unwrap_or_else(|e| panic!("failed to obtain the XSD schema : {}", e));
    let crypto_data = web::Data::new(crypto_config);
    let pool_data = web::Data::new(pool);
    let xsd_schema = web::Data::new(xsd_schema);
    HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .app_data(xsd_schema.clone())
            .app_data(pool_data.clone())
            .app_data(crypto_data.clone())
            .app_data(web::JsonConfig::default().limit(256 * 1024))
            .route("/", web::get().to(hello))
            .route("/health_check", web::get().to(health_check))
            .route("/clear", web::post().to(clearance))
            .route("/report", web::post().to(reporting))
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
