use anyhow::Context;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{errors::ApiError, models::submit_invoice::SubmitInvoiceDto};

pub struct RejectedInvoiceRecord<'a> {
    pub submitted: &'a SubmitInvoiceDto,
    pub endpoint: &'static str,
    pub invoice_type: &'static str,
    pub api_error: ApiError,
    pub supplier_tin: Option<&'a str>,
    pub device_id: Option<Uuid>,
}

pub async fn save_rejected_invoice(
    pool: &PgPool,
    record: RejectedInvoiceRecord<'_>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        INSERT INTO rejected_invoices (
            id,
            submitted_uuid,
            submitted_invoice_hash,
            submitted_invoice,
            endpoint,
            invoice_type,
            error_code,
            error_message,
            http_status,
            supplier_tin,
            device_id
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(Uuid::new_v4())
    .bind(&record.submitted.uuid)
    .bind(&record.submitted.invoice_hash)
    .bind(&record.submitted.invoice)
    .bind(record.endpoint)
    .bind(record.invoice_type)
    .bind(record.api_error.public_code())
    .bind(record.api_error.public_message())
    .bind(i32::from(record.api_error.public_status().as_u16()))
    .bind(record.supplier_tin)
    .bind(record.device_id)
    .execute(pool)
    .await
    .context("failed to store rejected invoice")?;

    Ok(())
}
