use sqlx::PgPool;

pub async fn check_uuid(uuid: &uuid::Uuid,db_pool: &PgPool) -> anyhow::Result<bool> {
    let exists: bool = sqlx::query_scalar!(
        "SELECT EXISTS (SELECT 1 FROM invoices WHERE uuid = $1)",
        uuid
    )
    .fetch_one(db_pool)
    .await?.unwrap_or(false);

    Ok(exists)
}