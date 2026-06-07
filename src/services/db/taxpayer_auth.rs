use anyhow::{Context, anyhow};
use argon2::{
    Argon2, PasswordVerifier,
    password_hash::{Error as PasswordHashError, PasswordHash},
};
use sqlx::{FromRow, PgPool};
use tracing::instrument;

pub struct AuthenticatedTaxpayer {
    pub tin: String,
    pub name: String,
}

pub struct TaxpayerProfile {
    pub tin: String,
    pub name: String,
    pub address: Option<String>,
    pub created_at: String,
}

#[derive(FromRow)]
struct TaxpayerAuthRecord {
    tin: String,
    name: String,
    password_hash: String,
}

#[derive(FromRow)]
struct TaxpayerProfileRecord {
    tin: String,
    name: String,
    address: Option<String>,
    created_at: String,
}

#[instrument(skip(pool))]
pub async fn fetch_taxpayer_profile(
    tin: &str,
    pool: &PgPool,
) -> anyhow::Result<Option<TaxpayerProfile>> {
    let record = sqlx::query_as::<_, TaxpayerProfileRecord>(
        r#"
        SELECT tin, name, address,
               to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS created_at
        FROM taxpayers
        WHERE tin = $1
        "#,
    )
    .bind(tin)
    .fetch_optional(pool)
    .await
    .context("failed to fetch taxpayer profile")?;

    Ok(record.map(|r| TaxpayerProfile {
        tin: r.tin,
        name: r.name,
        address: r.address,
        created_at: r.created_at,
    }))
}

#[instrument(skip(pool, password), fields(tin = %tin))]
pub async fn authenticate_taxpayer(
    tin: &str,
    password: &str,
    pool: &PgPool,
) -> anyhow::Result<Option<AuthenticatedTaxpayer>> {
    let record = sqlx::query_as::<_, TaxpayerAuthRecord>(
        r#"
        SELECT tin, name, password_hash
        FROM taxpayers
        WHERE tin = $1
        "#,
    )
    .bind(tin)
    .fetch_optional(pool)
    .await
    .context("failed to fetch taxpayer credentials")?;

    let Some(record) = record else {
        return Ok(None);
    };

    let password_hash = PasswordHash::new(&record.password_hash)
        .map_err(|error| anyhow!("stored taxpayer password hash is invalid: {error}"))?;

    match Argon2::default().verify_password(password.as_bytes(), &password_hash) {
        Ok(()) => Ok(Some(AuthenticatedTaxpayer {
            tin: record.tin,
            name: record.name,
        })),
        Err(PasswordHashError::Password) => Ok(None),
        Err(error) => Err(anyhow!("failed to verify taxpayer password: {error}")),
    }
}
