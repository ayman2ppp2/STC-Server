use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::models::submit_invoice_dto::InvoiceType;

pub async fn save_invoice<'a>(
    tx: &mut Transaction<'a, Postgres>,
    invoiceb64: &String,
    uuid: &Uuid,
    hash: Vec<u8>,
    device_id: &Uuid,
    invoice_type: InvoiceType,
) -> anyhow::Result<()> {
    let result = sqlx::query!(
        r#"
        INSERT INTO invoices (invoiceb64, uuid, hash, device_id, invoice_type)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        invoiceb64,
        uuid,
        hash,
        device_id,
        invoice_type.as_str(),
    )
    .execute(&mut **tx)
    .await;

    match result {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(e)) if e.constraint() == Some("invoices_uuid_unique") => {
            anyhow::bail!("Invoice UUID already exists")
        }
        Err(e) => Err(e.into()),
    }
}
