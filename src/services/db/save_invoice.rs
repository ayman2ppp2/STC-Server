use sqlx::{Postgres, Transaction};
use tracing::instrument;
use uuid::Uuid;

use crate::models::submit_invoice::InvoiceType;

#[instrument(skip(tx, invoice_bytes, hash), fields(uuid = %uuid, device_uuid = %device_id, invoice_type = %invoice_type.as_str()))]
pub async fn save_invoice<'a>(
    tx: &mut Transaction<'a, Postgres>,
    invoice_bytes: &[u8],
    uuid: &Uuid,
    hash: Vec<u8>,
    device_id: &Uuid,
    invoice_type: InvoiceType,
) -> anyhow::Result<()> {
    let result = sqlx::query!(
        r#"
        INSERT INTO invoices (invoice_bytes, uuid, hash, device_id, invoice_type)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        invoice_bytes,
        uuid,
        hash,
        device_id,
        invoice_type.as_str(),
    )
    .execute(&mut **tx)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(e)) if e.constraint() == Some("invoices_pkey") => {
            anyhow::bail!("Invoice UUID already exists")
        }
        Err(e) => Err(e.into()),
    }
}
