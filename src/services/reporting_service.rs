use sqlx::PgPool;

use crate::{config::{crypto_config::Crypto, xsd_config::SchemaValidator}, models::submit_invoice_dto::{IntermediateInvoiceDto, InvoiceType}, services::{pki_service::compute_hash, save_invoice::save_invoice, validation_service::validate_invoice}};

pub async fn process_reporting(
    intermediate: IntermediateInvoiceDto,
    db_pool: &PgPool,
    crypto: &Crypto,
    sandbox: bool,
    schema: &SchemaValidator,
    invoice_type : InvoiceType,
) -> anyhow::Result<()> {
    // Run shared pipeline
    // Note: Reporting might have relaxed rules. You can pass a config struct 
    // instead of just `sandbox` if you need to skip PIH for reporting, for example.
    validate_invoice(&intermediate, db_pool, crypto, sandbox, schema,invoice_type).await?;

    // Reporting-specific logic: No stamping! 
    // Just calculate the hash to save it.
    let hash = compute_hash(&intermediate.canonicalized_invoice_bytes)?;

    // Store the raw invoice directly
    if !sandbox {
        save_invoice(
        db_pool, 
        &String::from_utf8(intermediate.invoice_bytes)?, 
        &intermediate.uuid, 
        hash, 
        intermediate.company,
        InvoiceType::Reporting
    ).await?;
    }

    Ok(())
}