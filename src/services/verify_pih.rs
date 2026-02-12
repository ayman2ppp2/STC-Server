use anyhow::Context;
use base64::{Engine, engine::general_purpose};
use openssl::memcmp;
use sqlx::PgPool;

use crate::services::{extractors::extract_pih, pki_service::compute_hash};

pub async fn verify_pih (invoice:&[u8],pool:&PgPool,company_id:&String)->anyhow::Result<bool>{
  let ex_pih = extract_pih(invoice).context("failed to extract the PIH")?;
  let ex_pih = general_purpose::STANDARD.decode(ex_pih).context("failed to decode the extracted PIH")?;

  let fetched_pih =match sqlx::query!(
        r#"
        SELECT hash
        FROM invoices
        WHERE company = $1
        ORDER BY created_at DESC
        LIMIT 1
        "#,
        company_id
    )
    .fetch_optional(pool)
    .await? {
      Some(hash) => {hash.hash},
      None => compute_hash(String::from('0').as_bytes())?,
  };
  if !memcmp::eq(&ex_pih, &fetched_pih){
    anyhow::bail!("PIH hash mismatch")
  }

  Ok(true)
}
