use sqlx::PgPool;

pub async fn db_from_env ()-> anyhow::Result<PgPool>{
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

    // println!("PORT = {:?}", std::env::var("PORT"));
    // println!("DATABASE_URL = {:?}", std::env::var("DATABASE_URL"));

    Ok(PgPool::connect(&database_url)
        .await?)
        

}