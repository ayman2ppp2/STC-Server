use actix_web::web::Data;
use fastxml::schema::CompiledSchema;
use sqlx::PgPool;
use tracing::instrument;

use crate::{
    config::{crypto_config::Crypto},
    models::submit_invoice::{IntermediateInvoiceDto, InvoiceType},
    services::{
        pipeline::clear_invoice::clear_invoice,
        db::device_service::fetch_device_for_update,
        xml::extractors::extract_icv,
        db::icv_service::{update_icv_and_pih, verify_icv},
        db::save_invoice::save_invoice,
        pipeline::validation_service::validate_invoice,
    },
};

#[instrument(skip_all)]
pub async fn process_clearance(
    intermediate: IntermediateInvoiceDto,
    db_pool: &PgPool,
    crypto: &Crypto,
    sandbox: bool,
    schema: Data<CompiledSchema>,
    invoice_type: InvoiceType,
) -> anyhow::Result<String> {
    // Run shared pipeline
    validate_invoice(&intermediate, db_pool, crypto, sandbox, schema, invoice_type).await?;

    // Clearance-specific logic: Stamping
    let (hash, cleared_invoice) = clear_invoice(&intermediate, crypto)?;

    // Store it
    if !sandbox {
        let mut tx = db_pool.begin().await?;

        // Fetch device with lock to prevent race conditions
        let device = fetch_device_for_update(&intermediate.device.device_uuid, &mut tx).await?;

        // Verify ICV 
        let icv = extract_icv(&intermediate.invoice_bytes)?;
        verify_icv(icv, device.current_icv)?;

        // Update ICV and PIH
        update_icv_and_pih(&mut tx, &device.device_uuid, device.current_icv + 1, hash.clone()).await?;

        // Save invoice
        save_invoice(
            &mut tx,
            &cleared_invoice,
            &intermediate.uuid,
            hash,
            &device.device_uuid,
            InvoiceType::Clearance,
        )
        .await?;

        tx.commit().await?;
    }
    Ok(cleared_invoice)
}
