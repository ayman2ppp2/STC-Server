use sqlx::PgPool;

use crate::{
    config::{crypto_config::Crypto, xsd_config::SchemaValidator},
    models::submit_invoice_dto::IntermediateInvoiceDto,
    services::{
        clear_invoice::clear_invoice, save_invoice::save_invoice,
        validation_service::validate_invoice,
    },
};

pub async fn process_clearance(
    intermediate: IntermediateInvoiceDto,
    db_pool: &PgPool,
    crypto: &Crypto,
    sandbox: bool,
    schema: &SchemaValidator,
) -> anyhow::Result<String> {
    // Run shared pipeline
    validate_invoice(&intermediate, db_pool, crypto, sandbox, schema).await?;

    // Clearance-specific logic: Stamping
    let (hash, cleared_invoice) = clear_invoice(&intermediate, crypto)?;

    // Store it
    if !sandbox {
        save_invoice(
            db_pool,
            &cleared_invoice,
            &intermediate.uuid,
            hash,
            intermediate.company,
        )
        .await?;
    }
    Ok(cleared_invoice)
}
