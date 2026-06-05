use actix_web::web::Data;
use fastxml::schema::CompiledSchema;
use sqlx::PgPool;
use tracing::instrument;

use crate::{
    config::crypto_config::Crypto,
    models::submit_invoice::{IntermediateInvoiceDto, InvoiceType},
    services::{
        db::device_service::fetch_device_for_update,
        db::icv_service::{update_icv_and_pih, verify_icv},
        db::pih_service::verify_pih,
        db::save_invoice::save_invoice,
        pipeline::validation_service::validate_invoice,
        xml::extractors::extract_icv,
    },
};

#[instrument(
    skip(db_pool, crypto, schema, intermediate),
    fields(
        uuid = %intermediate.uuid,
        device_uuid = %intermediate.device.device_uuid,
        supplier_tin = %intermediate.supplier,
        sandbox,
        invoice_type = ?invoice_type
    )
)]
pub async fn process_reporting(
    intermediate: IntermediateInvoiceDto,
    db_pool: &PgPool,
    crypto: &Crypto,
    sandbox: bool,
    schema: Data<CompiledSchema>,
    invoice_type: InvoiceType,
) -> anyhow::Result<()> {
    // Run shared pipeline
    let hash = validate_invoice(&intermediate, db_pool, crypto, schema, invoice_type).await?;

    // Store the raw invoice directly
    if !sandbox {
        let mut tx = db_pool.begin().await?;

        // Fetch device with lock to prevent race conditions
        let device = fetch_device_for_update(&intermediate.device.device_uuid, &mut tx).await?;

        // Verify ICV hasn't changed since validation
        let icv = extract_icv(&intermediate.invoice_bytes)?;
        verify_icv(icv, device.current_icv)?;

        // Verify PIH against the locked device row.
        verify_pih(&intermediate.invoice_bytes, &device.last_pih)?;

        // Update ICV and PIH
        update_icv_and_pih(
            &mut tx,
            &device.device_uuid,
            device.current_icv + 1,
            hash.clone(),
        )
        .await?;

        // Save invoice
        save_invoice(
            &mut tx,
            &intermediate.invoice_bytes,
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
