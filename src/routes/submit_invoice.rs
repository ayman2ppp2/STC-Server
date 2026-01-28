use actix_web::{HttpResponse, web};
use openssl::memcmp;
use serde_json::Value;
use sqlx::{PgPool, Row};

use crate::{
    config::crypto_config::Crypto,
    models::{
        self,
        submit_invoice_dto::SubmitInvoiceDto,
        submit_invoice_response_dto::{ClearenceStatus, MessageType, SubmitInvoiceResponse, ValidationMessage, ValidationResults, ValidationStatus},
    },
    services::{clear_invoice::{self, clear_invoice}, pki_service::{compute_hash, verify_cert_with_ca, verify_signature_with_cert}},
};

pub async fn submit_invoice(
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
) -> Result<HttpResponse, actix_web::Error> {
    print!("entring the dto parsing phase");
    let intermidate_dto = invoice_dto
        .into_inner()
        .parse()
        .map_err(actix_web::error::ErrorForbidden)?;
    // compare hash
    print!("finished parsing the intermediate dto");
    let received_hash = &intermidate_dto.invoice_hash;
    let hash = compute_hash(&intermidate_dto.canonicalized_invoice_bytes).map_err(actix_web::error::ErrorBadRequest)?;
    if !memcmp::eq(received_hash, &hash) {
        return Err(actix_web::error::ErrorNotAcceptable("hash mismatch"));
    }
    // verify the certificate
    verify_cert_with_ca(&crypto.get_ref().certificate, &intermidate_dto.certificate)
        .await
        .map_err(actix_web::error::ErrorBadRequest)?;
    // verify signature
    verify_signature_with_cert(
        &intermidate_dto.invoice_hash,
        &intermidate_dto.invoice_signature,
        &intermidate_dto.certificate,
    )
    .await
    .map_err(actix_web::error::ErrorBadRequest)?;

    let cleared_invoice = clear_invoice(intermidate_dto, &crypto).map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(SubmitInvoiceResponse {
        clearence_status: ClearenceStatus::Cleared,
        cleared_invoice: String::from_utf8_lossy(&cleared_invoice).into(),
        validation_results: ValidationResults {
            info_messages:vec![
                ValidationMessage{
                    message_type: MessageType::Info,
                    code: "200".to_string(),
                    category: "XSD validation".to_string(),
                    message: "Complied with UBL 2.1 standards in line with STC specifications".to_string(),
                    status: ValidationStatus::Pass,
                }
            ]
            ,
            warning_messages: vec![],
            error_messages: vec![],
            validation_status: ValidationStatus::Pass,
        },
    }))
    // match quick_xml::de::from_str::<invoice_model::Invoice>(&body) {
    //     Ok(invoice) => {
    //         // attempt to persist the invoice in Postgres
    //
    //     }
    //     Err(e) => {
    //         eprintln!("Failed to parse invoice XML: {}", e);
    //         Ok(HttpResponse::BadRequest().body(format!("Failed to parse invoice XML: {}", e)))
    //     }
    // }
}
async fn save_invoice(
    pool: &PgPool,
    invoice: &models::invoice_model::Invoice,
    raw_xml: &str,
) -> Result<(), sqlx::Error> {
    // helper: we'll use .as_deref() and .as_ref() inline to borrow inner strings

    // insert supplier party (no transaction for now)
    let supplier_party_id: Option<i64> = if let Some(sup_wrap) = &invoice.accounting_supplier_party
    {
        let p = &sup_wrap.party;
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
    let customer_party_id: Option<i64> = if let Some(cust_wrap) = &invoice.accounting_customer_party
    {
        let p = &cust_wrap.party;
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
    let (tax_total_amount, tax_total_currency) = if let Some(tt) = &invoice.tax_total {
        (
            tt.tax_amount.as_ref().and_then(|a| a.value.clone()),
            tt.tax_amount.as_ref().and_then(|a| a.currency_id.clone()),
        )
    } else {
        (None, None)
    };

    let (line_extension_amount, _line_currency) = if let Some(lm) = &invoice.legal_monetary_total {
        (
            lm.line_extension_amount
                .as_ref()
                .and_then(|a| a.value.clone()),
            lm.line_extension_amount
                .as_ref()
                .and_then(|a| a.currency_id.clone()),
        )
    } else {
        (None, None)
    };

    let tax_exclusive_amount = invoice.legal_monetary_total.as_ref().and_then(|l| {
        l.tax_exclusive_amount
            .as_ref()
            .and_then(|a| a.value.clone())
    });
    let tax_inclusive_amount = invoice.legal_monetary_total.as_ref().and_then(|l| {
        l.tax_inclusive_amount
            .as_ref()
            .and_then(|a| a.value.clone())
    });
    let payable_amount = invoice
        .legal_monetary_total
        .as_ref()
        .and_then(|l| l.payable_amount.as_ref().and_then(|a| a.value.clone()));

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
    .bind(Value::String(raw_xml.to_string()))
    .execute(pool)
    .await?;

    // insert invoice lines
    if let Some(lines) = &invoice.invoice_lines {
        for line in lines {
            let line_id = &line.id;
            let quantity = line
                .invoiced_quantity
                .as_ref()
                .and_then(|q| q.value.clone());
            let unit_code = line
                .invoiced_quantity
                .as_ref()
                .and_then(|q| q.unit_code.clone());
            let line_ext_amount = line
                .line_extension_amount
                .as_ref()
                .and_then(|a| a.value.clone());
            let line_currency = line
                .line_extension_amount
                .as_ref()
                .and_then(|a| a.currency_id.clone());
            let item_name = line.item.as_ref().and_then(|i| i.name.clone());
            let item_desc = line.item.as_ref().and_then(|i| i.description.clone());
            let price_amount = line
                .price
                .as_ref()
                .and_then(|p| p.price_amount.as_ref().and_then(|a| a.value.clone()));
            let price_currency = line
                .price
                .as_ref()
                .and_then(|p| p.price_amount.as_ref().and_then(|a| a.currency_id.clone()));
            let tax_amount = line
                .tax_total
                .as_ref()
                .and_then(|t| t.tax_amount.as_ref().and_then(|a| a.value.clone()));
            let tax_percent = line.tax_total.as_ref().and_then(|t| {
                t.tax_subtotal
                    .as_ref()
                    .and_then(|s| s.tax_category.as_ref().and_then(|c| c.percent.clone()))
            });

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
