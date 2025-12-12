use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use sqlx::{PgPool, Row};
use serde_json::Value as JsonValue;
mod models;

async fn hello() -> impl Responder {
    "Hello from STC Actix server!\n"
}
async fn health_check() -> impl Responder {
    HttpResponse::Ok()
}
// pub async fn get_invoices(db: web::Data<PgPool>) -> impl Responder {
//     let result = sqlx::query_as::<_, Invoice>(
//         r#"
//         SELECT
//             id,
//             issue_date,
//             invoice_type_code,
//             document_currency_code,
//             supplier_party_id,
//             customer_party_id,
//             tax_total_amount,
//             tax_total_currency,
//             line_extension_amount,
//             tax_exclusive_amount,
//             tax_inclusive_amount,
//             payable_amount,
//             raw_xml,
//             created_at
//         FROM invoices
//         ORDER BY created_at DESC
//         "#
//     )
//     .fetch_all(db.get_ref())
//     .await;

//     match result {
//         Ok(invoices) => HttpResponse::Ok().json(invoices),
//         Err(e) => {
//             eprintln!("DB error: {:?}", e);
//             HttpResponse::InternalServerError().body("Failed to fetch invoices")
//         }
//     }
// }
async fn submit_invoice(db: web::Data<PgPool>, body: String) -> actix_web::Result<HttpResponse> {
    // read body length and attempt to parse XML into our models::Invoice
    let length = body.len();

    match quick_xml::de::from_str::<models::Invoice>(&body) {
        Ok(invoice) => {
            // attempt to persist the invoice in Postgres
            match save_invoice(db.get_ref(), invoice, &body).await {
                Ok(_) => {
                    println!("Saved invoice successfully");
                    Ok(HttpResponse::Ok().body(format!(
                        "Received invoice with content-length: {}",
                        length
                    )))
                }
                Err(e) => {
                    eprintln!("DB error saving invoice: {}", e);
                    Ok(HttpResponse::InternalServerError()
                        .body(format!("Failed to save invoice: {}", e)))
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to parse invoice XML: {}", e);
            Ok(HttpResponse::BadRequest().body(format!("Failed to parse invoice XML: {}", e)))
        }
    }
}

async fn save_invoice(pool: &PgPool, invoice: models::Invoice, raw_xml: &str) -> Result<(), sqlx::Error> {
    // helper: we'll use .as_deref() and .as_ref() inline to borrow inner strings

    // insert supplier party (no transaction for now)
    let supplier_party_id: Option<i64> = if let Some(sup_wrap) = invoice.accounting_supplier_party {
        let p = sup_wrap.party;
        let row = sqlx::query("INSERT INTO parties (name, company_id, telephone, email) VALUES ($1,$2,$3,$4) RETURNING id")
            .bind(p.name.as_deref())
            .bind(p.party_tax_scheme.as_ref().and_then(|pts| pts.company_id.as_deref()))
            .bind(p.contact.as_ref().and_then(|c| c.telephone.as_deref()))
            .bind(p.contact.as_ref().and_then(|c| c.electronic_mail.as_deref()))
            .fetch_one(pool)
            .await?;
        Some(row.get::<i64, _>("id"))
    } else {
        None
    };

    // insert customer party
    let customer_party_id: Option<i64> = if let Some(cust_wrap) = invoice.accounting_customer_party {
        let p = cust_wrap.party;
        let row = sqlx::query("INSERT INTO parties (name, company_id, telephone, email) VALUES ($1,$2,$3,$4) RETURNING id")
            .bind(p.name.as_deref())
            .bind(p.party_tax_scheme.as_ref().and_then(|pts| pts.company_id.as_deref()))
            .bind(p.contact.as_ref().and_then(|c| c.telephone.as_deref()))
            .bind(p.contact.as_ref().and_then(|c| c.electronic_mail.as_deref()))
            .fetch_one(pool)
            .await?;
        Some(row.get::<i64, _>("id"))
    } else {
        None
    };

    // invoice-level monetary fields
    let (tax_total_amount, tax_total_currency) = if let Some(tt) = invoice.tax_total {
        (tt.tax_amount.as_ref().and_then(|a| a.value.clone()), tt.tax_amount.as_ref().and_then(|a| a.currency_id.clone()))
    } else {
        (None, None)
    };

    let (line_extension_amount, _line_currency) = if let Some(lm) = &invoice.legal_monetary_total {
        (lm.line_extension_amount.as_ref().and_then(|a| a.value.clone()), lm.line_extension_amount.as_ref().and_then(|a| a.currency_id.clone()))
    } else { (None, None) };

    let tax_exclusive_amount = invoice.legal_monetary_total.as_ref().and_then(|l| l.tax_exclusive_amount.as_ref().and_then(|a| a.value.clone()));
    let tax_inclusive_amount = invoice.legal_monetary_total.as_ref().and_then(|l| l.tax_inclusive_amount.as_ref().and_then(|a| a.value.clone()));
    let payable_amount = invoice.legal_monetary_total.as_ref().and_then(|l| l.payable_amount.as_ref().and_then(|a| a.value.clone()));

    // insert invoice (cast numeric fields to numeric in SQL; NULL is allowed)
    let _ = sqlx::query(
        "INSERT INTO invoices (id, issue_date, invoice_type_code, document_currency_code, supplier_party_id, customer_party_id, tax_total_amount, tax_total_currency, line_extension_amount, tax_exclusive_amount, tax_inclusive_amount, payable_amount, raw_xml) \
         VALUES ($1, $2::date, $3, $4, $5, $6, $7::numeric, $8, $9::numeric, $10::numeric, $11::numeric, $12::numeric, $13::jsonb)"
    )
    .bind(&invoice.id)
    .bind(&invoice.issue_date)
    .bind(invoice.invoice_type_code.as_deref())
    .bind(invoice.document_currency_code.as_deref())
    .bind(supplier_party_id)
    .bind(customer_party_id)
    .bind(tax_total_amount.as_deref())
    .bind(tax_total_currency.as_deref())
    .bind(line_extension_amount.as_deref())
    .bind(tax_exclusive_amount.as_deref())
    .bind(tax_inclusive_amount.as_deref())
    .bind(payable_amount.as_deref())
    .bind(JsonValue::String(raw_xml.to_string()))
    .execute(pool)
    .await?;

    // insert invoice lines
    if let Some(lines) = invoice.invoice_lines {
        for line in lines {
            let line_id = line.id;
            let quantity = line.invoiced_quantity.as_ref().and_then(|q| q.value.clone());
            let unit_code = line.invoiced_quantity.as_ref().and_then(|q| q.unit_code.clone());
            let line_ext_amount = line.line_extension_amount.as_ref().and_then(|a| a.value.clone());
            let line_currency = line.line_extension_amount.as_ref().and_then(|a| a.currency_id.clone());
            let item_name = line.item.as_ref().and_then(|i| i.name.clone());
            let item_desc = line.item.as_ref().and_then(|i| i.description.clone());
            let price_amount = line.price.as_ref().and_then(|p| p.price_amount.as_ref().and_then(|a| a.value.clone()));
            let price_currency = line.price.as_ref().and_then(|p| p.price_amount.as_ref().and_then(|a| a.currency_id.clone()));
            let tax_amount = line.tax_total.as_ref().and_then(|t| t.tax_amount.as_ref().and_then(|a| a.value.clone()));
            let tax_percent = line.tax_total.as_ref().and_then(|t| t.tax_subtotal.as_ref().and_then(|s| s.tax_category.as_ref().and_then(|c| c.percent.clone())));

            let _ = sqlx::query(
                "INSERT INTO invoice_lines (invoice_id, line_id, quantity, unit_code, line_extension_amount, line_currency, item_name, item_description, price_amount, price_currency, tax_amount, tax_percent) \
                 VALUES ($1, $2, $3::numeric, $4, $5::numeric, $6, $7, $8, $9::numeric, $10, $11::numeric, $12::numeric)"
            )
            .bind(&invoice.id)
            .bind(line_id.as_deref())
            .bind(quantity.as_deref())
            .bind(unit_code.as_deref())
            .bind(line_ext_amount.as_deref())
            .bind(line_currency.as_deref())
            .bind(item_name.as_deref())
            .bind(item_desc.as_deref())
            .bind(price_amount.as_deref())
            .bind(price_currency.as_deref())
            .bind(tax_amount.as_deref())
            .bind(tax_percent.as_deref())
            .execute(pool)
            .await?;
        }
    }

    Ok(())
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

    // create DB pool from DATABASE_URL env var or fall back to init_db.sh defaults
    // init_db.sh defaults:
    //   POSTGRES_USER=postgres
    //   POSTGRES_PASSWORD=password
    //   POSTGRES_DB=stc-server
    //   POSTGRES_HOST=localhost
    //   POSTGRES_PORT=5432
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let db_user = std::env::var("POSTGRES_USER").unwrap_or_else(|_| "postgres".to_string());
        let db_password = std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "password".to_string());
        let db_name = std::env::var("POSTGRES_DB").unwrap_or_else(|_| "stc-server".to_string());
        let db_host = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
        let db_port = std::env::var("POSTGRES_PORT").unwrap_or_else(|_| "5432".to_string());
        format!("postgres://{}:{}@{}:{}/{}", db_user, db_password, db_host, db_port, db_name)
    });

    let pool = PgPool::connect(&database_url).await.expect(&format!("Failed to connect to Postgres: {}", database_url));

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .route("/", web::get().to(hello))
            .route("/health_check", web::get().to(health_check))
            .route("/submit_invoice", web::post().to(submit_invoice))
            // .route("/invoices", web::get().to(get_invoices))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
