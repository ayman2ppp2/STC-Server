use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::cookie::Key;
use actix_web::{App, HttpMessage, HttpResponse, HttpServer, dev::Service, http::header, web};
use stc_server::{
    config::crypto_config::Crypto,
    config::{db_config, xsd_config::schema_validator_from_temp},
    docs::ApiDoc,
    errors::json_error_handler,
    routes::{
        enroll::enroll,
        health_check::health_check,
        invoice_controller::{
            clearance_prod, clearance_sandbox, reporting_prod, reporting_sandbox,
        },
        pages::{e_invoicing_page, home, login_page, sandbox_page},
        taxpayer_portal::{
            generate_enrollment_token, invoice_report, prepare_invoice_payload, sign_in, sign_out,
            taxpayer_me,
        },
        verify_qr::verify_qr,
    },
    services::db::token_checking::token_cleanup_loop,
};
use tracing_actix_web::{RequestId, TracingLogger};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

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

    let migrator = sqlx::migrate::Migrator::new(std::path::Path::new("./migrations"))
        .await
        .expect("Failed to load migrations");
    migrator.run(&pool).await.expect("Failed to run migrations");

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
    let session_key = match std::env::var("SESSION_SECRET") {
        Ok(val) => Key::from(val.as_bytes()),
        Err(_) => {
            tracing::warn!(
                "SESSION_SECRET not set; using ephemeral session key. Logged-in sessions will not survive restarts."
            );
            Key::generate()
        }
    };

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
            .wrap(SessionMiddleware::new(
                CookieSessionStore::default(),
                session_key.clone(),
            ))
            .app_data(xsd_schema.clone())
            .app_data(pool_data.clone())
            .app_data(crypto_data.clone())
            .app_data(
                web::JsonConfig::default()
                    .limit(256 * 1024)
                    .error_handler(json_error_handler),
            )
            .route("/", web::get().to(home))
            .route("/e-invoicing", web::get().to(e_invoicing_page))
            .route("/e-invoicing/login", web::get().to(login_page))
            .route("/e-invoicing/signin", web::post().to(sign_in))
            .route("/e-invoicing/logout", web::post().to(sign_out))
            .route("/e-invoicing/me", web::get().to(taxpayer_me))
            .route(
                "/e-invoicing/token",
                web::post().to(generate_enrollment_token),
            )
            .route("/e-invoicing/invoices", web::post().to(invoice_report))
            .route("/sandbox", web::get().to(sandbox_page))
            .route(
                "/sandbox/invoice-payload",
                web::post().to(prepare_invoice_payload),
            )
            .route(
                "/api",
                web::get().to(|| async {
                    HttpResponse::Found()
                        .append_header(("Location", "/api/"))
                        .finish()
                }),
            )
            .service(SwaggerUi::new("/api/{_:.*}").url("/api/openapi.json", ApiDoc::openapi()))
            .route("/health_check", web::get().to(health_check))
            .service(
                web::scope("/prod")
                    .service(
                        web::scope("/invoices")
                            .route("/clear", web::post().to(clearance_prod))
                            .route("/report", web::post().to(reporting_prod)),
                    )
                    .route("/enrollment/enroll", web::post().to(enroll)),
            )
            .service(
                web::scope("/sandbox").service(
                    web::scope("/invoices")
                        .route("/clear", web::post().to(clearance_sandbox))
                        .route("/report", web::post().to(reporting_sandbox)),
                ),
            )
            .route("/verify_qr", web::post().to(verify_qr))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
