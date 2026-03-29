use anyhow::Context;
use base64::{Engine, engine::general_purpose};
use openssl::memcmp;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::{extractors::extract_pih, pki_service::compute_hash};

pub async fn verify_pih(
    invoice: &[u8],
    pool: &PgPool,
    device_id: &Uuid,
    // 1. Added this parameter
) -> anyhow::Result<bool> {
    let ex_pih_b64 = extract_pih(invoice).context("failed to extract the PIH")?;
    let ex_pih = general_purpose::STANDARD
        .decode(ex_pih_b64)
        .context("failed to decode the extracted PIH")?;

    let fetched_pih = match sqlx::query!(
        r#"
        SELECT last_pih
        FROM devices
        WHERE device_uuid = $1
        "#,
        device_id
    )
    .fetch_optional(pool)
    .await?
    {
        Some(row) => row.last_pih,
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
        anyhow::bail!("PIH hash mismatch: chains do not match ")
    }

    Ok(true)
}