use sqlx::PgPool;

use crate::{
    config::{crypto_config::Crypto, xsd_config::SchemaValidator},
    models::submit_invoice_dto::{IntermediateInvoiceDto, InvoiceType},
    services::{
        device_service::fetch_device_for_update,
        extractors::extract_icv,
        icv_service::{update_icv_and_pih, verify_icv},
        pki_service::compute_hash,
        save_invoice::save_invoice,
        validation_service::validate_invoice,
    },
};

pub async fn process_reporting(
    intermediate: IntermediateInvoiceDto,
    db_pool: &PgPool,
    crypto: &Crypto,
    sandbox: bool,
    schema: &SchemaValidator,
    invoice_type: InvoiceType,
) -> anyhow::Result<()> {
    // Run shared pipeline
    validate_invoice(&intermediate, db_pool, crypto, sandbox, schema, invoice_type).await?;

    // Reporting-specific logic: No stamping!
    // Just calculate the hash to save it.
    let hash = compute_hash(&intermediate.canonicalized_invoice_bytes)?;

    // Store the raw invoice directly
    if !sandbox {
        let mut tx = db_pool.begin().await?;

        // Fetch device with lock to prevent race conditions
        let device = fetch_device_for_update(&intermediate.device.device_uuid, &mut tx).await?;

        // Verify ICV hasn't changed since validation
        let icv = extract_icv(&intermediate.invoice_bytes)?;
        verify_icv(icv, device.current_icv)?;

        // Update ICV and PIH
        update_icv_and_pih(&mut tx, &device.device_uuid, device.current_icv + 1, hash.clone()).await?;

        // Save invoice
        save_invoice(
            &mut tx,
            &String::from_utf8(intermediate.invoice_bytes)?,
            &intermediate.uuid,
            hash,
            &device.device_uuid,
            InvoiceType::Reporting,
        )
        .await?;

        tx.commit().await?;
    }

    Ok(())
}