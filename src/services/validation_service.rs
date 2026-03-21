use anyhow::{Context, bail};
use sqlx::PgPool;

use crate::{
    config::{crypto_config::Crypto, xsd_config::SchemaValidator},
    models::submit_invoice_dto::IntermediateInvoiceDto,
    services::{
        check_uuid::check_uuid,
        pki_service::{compute_hash, verify_cert_with_ca, verify_signature_with_cert},
        schema_validation::validate_schema,
        verify_pih::verify_pih,
    },
};

pub async fn validate_invoice(
    intermediate: &IntermediateInvoiceDto,
    db_pool: &PgPool,
    crypto: &Crypto,
    sandbox: bool,
    schema: &SchemaValidator,
) -> anyhow::Result<()> {
    // 1. Check UUID
    if !sandbox {
        check_uuid(&intermediate.uuid, db_pool).await?;
    }

    // 2. Validate Schema   
    let xml_body = std::str::from_utf8(&intermediate.invoice_bytes)
        .context("Invoice XML is not valid UTF-8")?;
    validate_schema(schema, xml_body)?;

    // 3. Verify Hash
    let received_hash = &intermediate.invoice_hash;
    let computed_hash = compute_hash(&intermediate.canonicalized_invoice_bytes)?;
    if !openssl::memcmp::eq(received_hash, &computed_hash) {
        bail!("Invoice hash mismatch");
    }

    // 4. Verify PIH (Previous Invoice Hash) chain
    if !sandbox {
        verify_pih(&intermediate.invoice_bytes, db_pool, &intermediate.company).await?;
    }

    // 5. Verify Cryptography
    verify_cert_with_ca(&crypto.certificate, &intermediate.certificate).await?;
    verify_signature_with_cert(
        &intermediate.invoice_hash,
        &intermediate.invoice_signature,
        &intermediate.certificate,
    )?;

    Ok(())
}
