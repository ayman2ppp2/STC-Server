use anyhow::bail;
use sqlx::PgPool;

pub async fn verify_supplier_tin(supplier_tin: &[u8],pool: &PgPool) -> anyhow::Result<()> {
    match check_supplier_tin(supplier_tin,pool).await {
        Ok(b) => match b {
            true => {}
            false => bail!("invalid supplier TIN"),
        },
        Err(e) => bail!("Error checking for the supplier TIN : {}", e),
    }

    Ok(())
}
pub async fn verify_customer_tin(customer_tin:&[u8],pool: &PgPool) -> anyhow::Result<()> {
    match check_customer_tin(customer_tin,pool).await {
        Ok(b) => match b {
            true => {}
            false => bail!("invalid customer TIN"),
        },
        Err(e) => bail!("Error checking for the customer TIN : {}", e),
    }

    Ok(())
}

async fn check_supplier_tin(extracted_tin: &[u8], pool: &PgPool) -> anyhow::Result<bool> {
    let exists = sqlx::query_scalar!(
        r#"
    SELECT EXISTS(
        SELECT 1 
        FROM taxpayers 
        WHERE tin = $1
    ) AS "exists!"
    "#,
        std::str::from_utf8(extracted_tin)?
    )
    .fetch_one(pool)
    .await?;
    Ok(exists)
}
async fn check_customer_tin(extracted_tin: &[u8],pool: &PgPool) -> anyhow::Result<bool> {
   let exists = sqlx::query_scalar!(
        r#"
    SELECT EXISTS(
        SELECT 1 
        FROM taxpayers 
        WHERE tin = $1
    ) AS "exists!"
    "#,
        std::str::from_utf8(extracted_tin)?
    )
    .fetch_one(pool)
    .await?;
    Ok(exists)
}
