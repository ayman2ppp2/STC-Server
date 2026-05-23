use sqlx::PgPool;
use tracing::instrument;
use uuid::Uuid;

use crate::services::crypto::pki_service::compute_hash;
use crate::services::db::token_checking::validate_taxpayer_exists;

pub struct OnboardingResult {
    pub token: String,
}

#[instrument(skip(pool), fields(company_id = %company_id))]
pub async fn generate_token(company_id: &str, pool: &PgPool) -> anyhow::Result<OnboardingResult> {
    if !validate_taxpayer_exists(company_id, pool).await? {
        anyhow::bail!("Company ID not found in taxpayer registry");
    }

    let rand = Uuid::new_v4();
    let token = format!("{}:{}", company_id, &rand.to_string()[..]);
    let hash = compute_hash(token.as_bytes())?;

    sqlx::query!(
        "INSERT INTO csr_challenges (token_hash,company_id)
        VALUES($1,$2)",
        hash,
        company_id,
    )
    .execute(pool)
    .await?;

    Ok(OnboardingResult { token })
}
