use sqlx::PgPool;
use tracing::instrument;

#[instrument(skip(pool))]
pub async fn fetch_token(token_hash: &[u8], pool: &PgPool) -> anyhow::Result<Option<Vec<u8>>> {
    let record = sqlx::query!(
        r#"
    SELECT token_hash as "token_hash!"
    FROM csr_challenges
    WHERE token_hash = $1
      AND used_at IS NULL
      AND expires_at > now()
    LIMIT 1
    "#,
        token_hash,
    )
    .fetch_optional(pool)
    .await?;
    Ok(record.map(|r| r.token_hash))
}

#[instrument(skip(pool))]
pub async fn mark_token_used(token_hash: &[u8], pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query!(
        r#"
        UPDATE csr_challenges
        SET used_at = NOW()
        WHERE token_hash = $1
        "#,
        token_hash
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[instrument(skip(pool), fields(company_id = %tin))]
pub async fn validate_taxpayer_exists(tin: &str, pool: &PgPool) -> anyhow::Result<bool> {
    let exists = sqlx::query_scalar!(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM taxpayers
            WHERE tin = $1
        ) AS "exists!"
        "#,
        tin
    )
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

#[instrument(skip(pool))]
pub async fn cleanup_expired_tokens(pool: &PgPool) -> anyhow::Result<u64> {
    let result = sqlx::query!(
        r#"
        DELETE FROM csr_challenges
        WHERE expires_at < now()
          AND used_at IS NULL
        "#
    )
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

#[instrument(skip(pool))]
pub async fn token_cleanup_loop(pool: PgPool) {
    use tokio::time::{Duration, interval};

    let mut cleanup_interval = interval(Duration::from_secs(3600));
    loop {
        cleanup_interval.tick().await;

        match cleanup_expired_tokens(&pool).await {
            Ok(count) if count > 0 => tracing::info!(count, "Cleaned expired tokens"),
            Ok(_) => {}
            Err(e) => tracing::error!(%e, "Token cleanup failed"),
        }
    }
}