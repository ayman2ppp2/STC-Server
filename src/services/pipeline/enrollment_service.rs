use sqlx::PgPool;

use crate::config::crypto_config::Crypto;
use crate::models::enrollment::IntermediateEnrollDto;
use crate::services::crypto::pki_service::{compute_hash, handle_enrollment};
use crate::services::db::device_service::create_new_device;
use crate::services::db::tin_service::verify_supplier_tin;
use crate::services::db::token_checking::{fetch_token, mark_token_used};

pub async fn enroll_device(
    intermediate: &IntermediateEnrollDto,
    crypto: &Crypto,
    pool: &PgPool,
) -> anyhow::Result<String> {
    // compute hash of the received token
    let computed_hash = compute_hash(intermediate.token.as_bytes())?;
    // fetch the stored token hash from the database
    let stored_token_hash = fetch_token(&computed_hash, pool)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Token not found or expired"))?;
    // compare the computed hash with the stored hash
    if !openssl::memcmp::eq(&computed_hash, &stored_token_hash) {
        anyhow::bail!("Token hash mismatch");
    }
    // generate the certificate and sign it
    let certificate = handle_enrollment(intermediate, crypto).await?;
    // get the device ID from the CSR
    let device_id_str = intermediate.get_device_id()?;
    // parse the device ID as a UUID
    let device_uuid = uuid::Uuid::parse_str(&device_id_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse device ID as UUID: {}", e))?;
    // extract the TIN from the CSR
    let tin = intermediate.get_tin()?;
    // verify the TIN against the database
    verify_supplier_tin(tin.as_bytes(), pool).await?;
    // create a new device in the database
    create_new_device(&device_uuid, &tin, pool).await?;
    // mark the token as used
    mark_token_used(&stored_token_hash, pool).await?;

    Ok(certificate)
}
