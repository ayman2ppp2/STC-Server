use sqlx::PgPool;
use uuid::Uuid;

pub async fn save_invoice(
    pool: &PgPool,
    invoiceb64: &String,
    uuid: &Uuid,
    hash: Vec<u8>,
    company: String,
) -> anyhow::Result<(), sqlx::Error> {
    sqlx::query!(
        "
    INSERT INTO invoices (invoiceb64, uuid, hash, company)
    VALUES ($1, $2, $3, $4)
    ",
        invoiceb64,
        uuid,
        hash,
        company,
    )
    .execute(pool)
    .await?;
    Ok(())

}
