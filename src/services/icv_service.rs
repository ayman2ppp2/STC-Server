use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub fn verify_icv(icv: i32, current_icv: i32) -> anyhow::Result<()> {
    if icv != current_icv + 1 {
        anyhow::bail!("ICV mismatch: expected {}, got {}", current_icv + 1, icv);
    }
    Ok(())
}

pub async fn update_icv_and_pih<'a>(
    tx: &mut Transaction<'a, Postgres>,
    device_id: &Uuid,
    new_icv: i32,
    new_pih: Vec<u8>,
) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        UPDATE devices
        SET current_icv = $2, last_pih = $3
        WHERE device_uuid = $1
        "#,
        device_id,
        new_icv,
        new_pih
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
