use sqlx::PgPool;

pub async fn fetch_token_hash(id: &str, pool: &PgPool) -> anyhow::Result<Option<Vec<u8>>> {
    let record = sqlx::query!(
        r#"
    SELECT token_hash as "token_hash!"
    FROM csr_challenges
    WHERE company_id = $1
      AND used_at IS NULL
      AND expires_at > now()
    LIMIT 1
    "#,
        &id,
    )
    .fetch_optional(pool)
    .await?;
    Ok(record.map(|r| r.token_hash))
}

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