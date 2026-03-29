use anyhow::{Context, bail};
use sqlx::PgPool;

use crate::{
    config::{crypto_config::Crypto, xsd_config::SchemaValidator},
    models::submit_invoice_dto::{IntermediateInvoiceDto, InvoiceType},
    services::{
        check_uuid::check_uuid, extractors::{extract_customer_id, extract_supplier_id}, invoice_type_service::verify_invoice_type, pih_service::verify_pih, pki_service::{compute_hash, verfiy_supplier_tin_with_ca, verify_cert_with_ca, verify_signature_with_cert}, schema_validation::validate_schema, tin_service::{verify_customer_tin, verify_supplier_tin}
    },
};

pub async fn validate_invoice(
    intermediate: &IntermediateInvoiceDto,
    db_pool: &PgPool,
    crypto: &Crypto,
    sandbox: bool,
    schema: &SchemaValidator,
    invoice_type: InvoiceType,
) -> anyhow::Result<()> {
    // 1. Check UUID
    if !sandbox {
        check_uuid(&intermediate.uuid, db_pool).await?;
    }

    // 2. Validate Schema
    let xml_body = std::str::from_utf8(&intermediate.invoice_bytes)
        .context("Invoice XML is not valid UTF-8")?;
    validate_schema(schema, xml_body)?;

    // 3. verify invoice type

    match verify_invoice_type(&intermediate.invoice_bytes, &invoice_type) {
        Ok(_) => {}
        Err(e) => bail!("invoice type mismatch : {}", e),
    }

    // 4. Verify Hash
    let received_hash = &intermediate.invoice_hash;
    let computed_hash = compute_hash(&intermediate.canonicalized_invoice_bytes)?;
    if !openssl::memcmp::eq(received_hash, &computed_hash) {
        bail!("Invoice hash mismatch");
    }

    // 5. Verify PIH (Previous Invoice Hash) chain
    if !sandbox {
        verify_pih(
            &intermediate.invoice_bytes,
            db_pool,
            &intermediate.device.device_uuid,
        )
        .await?;
    }

    // 6. Verify Cryptography
    verify_cert_with_ca(&crypto.certificate, &intermediate.certificate).await?;
    verify_signature_with_cert(
        &intermediate.invoice_hash,
        &intermediate.invoice_signature,
        &intermediate.certificate,
    )?;
    // 7. veerify supplier tin with cert
    let supplier_tin = extract_supplier_id(&intermediate.invoice_bytes)?;
    verfiy_supplier_tin_with_ca(&supplier_tin,&intermediate.certificate)?;
    // 8. verify supplier/customer ID's
    verify_supplier_tin(
        supplier_tin.as_bytes(),
        db_pool,
    )
    .await?;
    match invoice_type {
        InvoiceType::Reporting => (),
        InvoiceType::Clearance => {
            verify_customer_tin(
                extract_customer_id(&intermediate.invoice_bytes)?.as_bytes(),
                db_pool,
            )
            .await?
        }
    }

    Ok(())
}
