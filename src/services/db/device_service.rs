use crate::{models::device::Device, services::crypto::pki_service::extract_device_id};
use anyhow::Context;
use openssl::x509::X509;
use sqlx::PgPool;
use sqlx::{Postgres, Transaction, types::time::OffsetDateTime};
use tracing::instrument;
use uuid::Uuid;

#[instrument]
pub async fn get_device(crt: &X509, pool: &PgPool) -> anyhow::Result<Device> {
    let device_id = extract_device_id(crt)?;
    let device = fetch_device(&device_id, pool).await?;
    Ok(device)
}

#[instrument(skip(pool), fields(device_uuid = %id))]
pub async fn fetch_device(id: &Uuid, pool: &PgPool) -> anyhow::Result<Device> {
    sqlx::query_as!(
        Device,
        r#"
        SELECT device_uuid, tin, current_icv, last_pih, COALESCE(is_active, false) AS "is_active!: bool", COALESCE(onboarded_at, NOW()) AS "onboarded_at!: OffsetDateTime"
        FROM devices
        WHERE device_uuid = $1::uuid
        "#,
        id
    )
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

#[instrument(skip(tx), fields(device_uuid = %id))]
pub async fn fetch_device_for_update<'a>(
    id: &Uuid,
    tx: &mut Transaction<'a, Postgres>,
) -> anyhow::Result<Device> {
    sqlx::query_as!(
        Device,
        r#"
        SELECT device_uuid, tin, current_icv, last_pih, COALESCE(is_active, false) AS "is_active!: bool", COALESCE(onboarded_at, NOW()) AS "onboarded_at!: OffsetDateTime"
        FROM devices
        WHERE device_uuid = $1::uuid
        FOR UPDATE
        "#,
        id
    )
    .fetch_one(&mut **tx)
    .await
    .map_err(Into::into)
}
#[instrument(skip(pool), fields(tin = %tin))]
pub async fn create_new_device(
    device_uuid: &Uuid,
    tin: &str,
    pool: &PgPool,
) -> anyhow::Result<Device> {
    let initial_pih: Vec<u8> =
        hex::decode("5feceb66ffc86f38d952786c6d696c79c2dbc239dd4e91b46729d73a27fb57e9")
            .context("Failed to decode initial PIH hex")?;

    sqlx::query!(
        r#"
        INSERT INTO devices (device_uuid, tin, current_icv, last_pih, is_active, onboarded_at)
        VALUES ($1::uuid, $2, 0, $3, true, NOW())
        "#,
        device_uuid,
        tin,
        initial_pih
    )
    .execute(pool)
    .await
    .context("Failed to insert device")?;

    fetch_device(device_uuid, pool).await
}
