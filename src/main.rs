use actix_web::{App, HttpMessage, HttpServer, dev::Service, http::header, web};
use stc_server::{
    config::crypto_config::Crypto,
    config::{db_config, xsd_config::schema_validator_from_temp},
    routes::{
        enroll::enroll,
        health_check::{health_check, hello},
        invoice_controller::{clearance, get_invoices, reporting},
        on_boarding::on_board,
        token_generator::token_generator,
        verify_qr::verify_qr,
    },
    services::db::token_checking::token_cleanup_loop,
};
use tracing_actix_web::{RequestId, TracingLogger};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("stc_server=info,tracing_actix_web=info,actix_web=warn")
    });
    let fmt_layer = fmt::layer()
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_span_events(FmtSpan::CLOSE);

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt_layer)
        .init();
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    init_tracing();

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
            .wrap_fn(|req, srv| {
                let fut = srv.call(req);
                async move {
                    let mut res = fut.await?;
                    let request_id = res
                        .request()
                        .extensions()
                        .get::<RequestId>()
                        .map(ToString::to_string);
                    if let Some(request_id) = request_id
                        && let Ok(value) = header::HeaderValue::from_str(&request_id)
                    {
                        res.headers_mut()
                            .insert(header::HeaderName::from_static("x-request-id"), value);
                    }
                    Ok(res)
                }
            })
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
            // https://stc-server.onrender.com/clear
            // https://stc-server.onrender.com/report
            .route("/verify_qr", web::post().to(verify_qr))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
