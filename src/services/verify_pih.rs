use anyhow::Context;
use base64::{Engine, engine::general_purpose};
use openssl::memcmp;
use sqlx::PgPool;

use crate::{models::submit_invoice_dto::InvoiceType, services::{extractors::extract_pih, pki_service::compute_hash}};

pub async fn verify_pih(
    invoice: &[u8],
    pool: &PgPool,
    company_id: &String,
    invoice_type: InvoiceType, // 1. Added this parameter
) -> anyhow::Result<bool> {
    let ex_pih_b64 = extract_pih(invoice).context("failed to extract the PIH")?;
    let ex_pih = general_purpose::STANDARD
        .decode(ex_pih_b64)
        .context("failed to decode the extracted PIH")?;

    // 2. Filter the query by invoice_type
    let fetched_pih = match sqlx::query!(
        r#"
        SELECT hash
        FROM invoices
        WHERE company = $1 AND invoice_type = $2
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        company_id,
        invoice_type.as_str() // 3. Pass the string representation
    )
    .fetch_optional(pool)
    .await?
    {
        Some(row) => row.hash,
        None => compute_hash(b"0")?, // Initial hash for the start of the chain
    };

    if ex_pih.len() != fetched_pih.len() {
        anyhow::bail!(
            "PIH length mismatch: expected {}, got {}",
            fetched_pih.len(),
            ex_pih.len()
        );
    }

    if !memcmp::eq(&ex_pih, &fetched_pih) {
        anyhow::bail!("PIH hash mismatch: chains do not match for {}", invoice_type.as_str())
    }

    Ok(true)
}