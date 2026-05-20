use actix_web::web::Data;
use anyhow::{Context, anyhow, bail};
use fastxml::schema::CompiledSchema;
use sqlx::PgPool;
use tracing::{error, instrument};

use crate::{
    config::crypto_config::Crypto,
    models::submit_invoice::{IntermediateInvoiceDto, InvoiceType},
    services::{
        crypto::pki_service::{
            check_cert_serial, compute_hash, verfiy_supplier_tin_with_ca, verify_cert_with_ca,
            verify_signature_with_cert,
        }, db::{check_uuid::check_uuid, pih_service::verify_pih, tin_service::{verify_customer_tin, verify_supplier_tin}}, pipeline::invoice_type_service::verify_invoice_type, xml::{extractors::{extract_crt_serial, extract_customer_tin, extract_signed_info}, schema_validation::validate_schema}
    },
};

#[instrument(skip_all)]
pub async fn validate_invoice(
    intermediate: &IntermediateInvoiceDto,
    db_pool: &PgPool,
    crypto: &Crypto,
    sandbox: bool,
    schema: Data<CompiledSchema>,
    invoice_type: InvoiceType,
) -> anyhow::Result<()> {
    let uuid = &intermediate.uuid;
    let supplier_tin = &intermediate.supplier;

    // 1. Check UUID
    if !sandbox && let Err(e) = check_uuid(uuid, db_pool).await {
        error!(uuid = %uuid, "UUID check failed: {}", e);
        return Err(e);
    }

    // 2. Check the Serial Number of the certificate
    if !check_cert_serial(
        &intermediate.certificate,
        extract_crt_serial(&intermediate.invoice_bytes)?,
    )? {
        bail!("Certificate serial mismatch");
    }

    // 3. Validate Schema
    let xml_body = std::str::from_utf8(&intermediate.invoice_bytes)
        .context("Invoice XML is not valid UTF-8")?;
    if let Err(e) = validate_schema(schema, xml_body) {
        error!(uuid = %uuid, "Schema validation failed: {}", e);
        return Err(e);
    }

    // 4. verify invoice type
    match verify_invoice_type(&intermediate.invoice_bytes, &invoice_type) {
        Ok(_) => {}
        Err(e) => {
            error!(uuid = %uuid, invoice_type = ?invoice_type, "Invoice type mismatch: {}", e);
            bail!("invoice type mismatch : {}", e)
        }
    }

    // 5. Verify Hash
    let received_hash = &intermediate.invoice_hash;
    let computed_hash = compute_hash(&intermediate.canonicalized_invoice_bytes)?;
    if !openssl::memcmp::eq(received_hash, &computed_hash) {
        error!(uuid = %uuid, "Invoice hash mismatch");
        bail!("Invoice hash mismatch");
    }

    // 6. Verify PIH (Previous Invoice Hash) chain
    if !sandbox
        && let Err(e) = verify_pih(
            &intermediate.invoice_bytes,
            db_pool,
            &intermediate.device.device_uuid,
        )
        .await
    {
        error!(uuid = %uuid, device_uuid = %intermediate.device.device_uuid, "PIH verification failed: {}", e);
        return Err(e);
    }

    // 7. Verify Cryptography
    if let Err(e) = verify_cert_with_ca(&crypto.certificate, &intermediate.certificate).await {
        error!(uuid = %uuid, "Certificate verification failed: {}", e);
        return Err(e);
    }

    // 8. Verify signature
    if !verify_signature_with_cert(
        &extract_signed_info(&intermediate.invoice_bytes)?,
        &intermediate.invoice_signature,
        &intermediate.certificate,
    )? {
        error!(uuid = %uuid, "Signature verification failed");
        bail!("Invalid invoice signature");
    }

    // 9. verify supplier tin with cert
    if let Err(e) = verfiy_supplier_tin_with_ca(supplier_tin, &intermediate.certificate) {
        error!(uuid = %uuid, supplier_tin = %supplier_tin, "Supplier TIN mismatch with certificate: {}", e);
        return Err(e);
    }

    // 10. verify supplier/customer ID's in database
    if let Err(e) = verify_supplier_tin(supplier_tin.as_bytes(), db_pool).await {
        error!(uuid = %uuid, supplier_tin = %supplier_tin, "Supplier TIN not found in database: {}", e);
        return Err(e);
    }

    match invoice_type {
        InvoiceType::Reporting => {}
        InvoiceType::Clearance => {
            // 11. Extract customer TIN and verify it against the database
            let customer_tin = extract_customer_tin(&intermediate.invoice_bytes)?;
            if let Err(e) = verify_customer_tin(customer_tin.as_bytes(), db_pool).await {
                error!(uuid = %uuid, supplier_tin = %supplier_tin, customer_id = %customer_tin, "Customer TIN not found in database: {}", e);
                return Err(e);
            }

            // 12. verify customer tin != supplier tin
            if &customer_tin == supplier_tin {
                let e = anyhow!("Customer TIN equals Supplier TIN");
                error!(uuid = %uuid, supplier_tin = %supplier_tin, customer_id = %customer_tin, "Customer TIN equals Supplier TIN: {}", e);
                bail!(e);
            }
        }
    }

    Ok(())
}
