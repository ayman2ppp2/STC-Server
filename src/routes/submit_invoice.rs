use actix_web::{HttpResponse, web};
use openssl::memcmp;
use sqlx::PgPool;
use uuid::Uuid;
// use chrono::DateTime;

use crate::{
    config::crypto_config::Crypto,
    models::{
        submit_invoice_dto::SubmitInvoiceDto,
        submit_invoice_response_dto::{
            ClearenceStatus, MessageType, SubmitInvoiceResponse, ValidationMessage,
            ValidationResults, ValidationStatus,
        },
    },
    services::{
        clear_invoice::clear_invoice,
        pki_service::{compute_hash, verify_cert_with_ca, verify_signature_with_cert},
    },
};

pub async fn submit_invoice(
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
) -> Result<HttpResponse, actix_web::Error> {

    let intermidate_dto = invoice_dto
        .into_inner()
        .parse()
        .map_err(actix_web::error::ErrorForbidden)?;
    // compare hash

    let received_hash = &intermidate_dto.invoice_hash;
    let hash = compute_hash(&intermidate_dto.canonicalized_invoice_bytes)
        .map_err(actix_web::error::ErrorBadRequest)?;
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

    let (hash,cleared_invoice,) = clear_invoice(&intermidate_dto, &crypto)
        .map_err(actix_web::error::ErrorInternalServerError)?;
    save_invoice(&db_pool, &cleared_invoice, &intermidate_dto.uuid, hash, intermidate_dto.company).await.map_err(actix_web::error::ErrorInternalServerError)?;
    Ok(HttpResponse::Ok().json(SubmitInvoiceResponse {
        clearence_status: ClearenceStatus::Cleared,
        cleared_invoice,
        validation_results: ValidationResults {
            info_messages: vec![ValidationMessage {
                message_type: MessageType::Info,
                code: "200".to_string(),
                category: "XSD validation".to_string(),
                message: "Complied with UBL 2.1 standards in line with STC specifications"
                    .to_string(),
                status: ValidationStatus::Pass,
            }],
            warning_messages: vec![],
            error_messages: vec![],
            validation_status: ValidationStatus::Pass,
        },
    }))

}
async fn save_invoice(pool: &PgPool, invoiceb64: &String, uuid: &Uuid, hash: Vec<u8>, company: String) -> anyhow::Result<(), sqlx::Error> {
    sqlx::query!(
    "
    INSERT INTO invoices (invoiceb64, uuid, hash, company)
    VALUES ($1, $2, $3, $4)
    ",
    invoiceb64,
    uuid,
    hash,
    company,

)
.execute(pool)
.await?;
    Ok(())
}
