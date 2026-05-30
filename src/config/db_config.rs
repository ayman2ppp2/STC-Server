use std::time::Duration;

use sqlx::{PgPool, postgres::PgPoolOptions};

pub async fn db_from_env() -> anyhow::Result<PgPool> {
    let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| {
        let db_user = std::env::var("POSTGRES_USER").unwrap_or_else(|_| "postgres".to_string());
        let db_password =
            std::env::var("POSTGRES_PASSWORD").unwrap_or_else(|_| "password".to_string());
        let db_name = std::env::var("POSTGRES_DB").unwrap_or_else(|_| "stc-server".to_string());
        let db_host = std::env::var("POSTGRES_HOST").unwrap_or_else(|_| "localhost".to_string());
        let db_port = std::env::var("POSTGRES_PORT").unwrap_or_else(|_| "5432".to_string());
        format!(
            "postgres://{}:{}@{}:{}/{}",
            db_user, db_password, db_host, db_port, db_name
        )
    });

    let max_connections = env_u32("DB_MAX_CONNECTIONS", 30);
    let min_connections = env_u32("DB_MIN_CONNECTIONS", 10).min(max_connections);
    let acquire_timeout_secs = env_u64("DB_ACQUIRE_TIMEOUT_SECS", 5);
    let idle_timeout_secs = env_u64("DB_IDLE_TIMEOUT_SECS", 300);
    let max_lifetime_secs = env_u64("DB_MAX_LIFETIME_SECS", 1800);

    Ok(PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(min_connections)
        .acquire_timeout(Duration::from_secs(acquire_timeout_secs))
        .idle_timeout(Duration::from_secs(idle_timeout_secs))
        .max_lifetime(Duration::from_secs(max_lifetime_secs))
        .connect(&database_url)
        .await?)
}

fn env_u32(key: &str, default: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_u64(key: &str, default: u64) -> u64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}
