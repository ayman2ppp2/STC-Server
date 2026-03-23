use sqlx::PgPool;

pub async fn fetch_token_hash(id : &str,pool : &PgPool)->anyhow::Result<Option<Vec<u8>>>{
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