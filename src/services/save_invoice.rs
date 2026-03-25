use sqlx::PgPool;
use uuid::Uuid;

use crate::models::submit_invoice_dto::InvoiceType;

pub async fn save_invoice(
    pool: &PgPool,
    invoiceb64: &String,
    uuid: &Uuid,
    hash: Vec<u8>,
    company: String,
    invoice_type : InvoiceType,
) -> anyhow::Result<(), sqlx::Error> {
    sqlx::query!(
        "
    INSERT INTO invoices (invoiceb64, uuid, hash, company,invoice_type)
    VALUES ($1, $2, $3, $4,$5)
    ",
        invoiceb64,
        uuid,
        hash,
        company,
        invoice_type.as_str(),
    )
    .execute(pool)
    .await?;
    Ok(())

}
