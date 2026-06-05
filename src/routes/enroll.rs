use actix_web::{HttpResponse, web};
use sqlx::PgPool;

use crate::{
    config::crypto_config::Crypto,
    errors::ApiError,
    models::{enrollment::EnrollDTO, responses::ApiResponse},
    services::pipeline::enrollment_service,
};

pub async fn enroll(
    dto: web::Json<EnrollDTO>,
    crypto: web::Data<Crypto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let intermediate_dto = dto.into_inner().parse().map_err(|e| {
        tracing::error!(error = %e, "CSR parse failed in enrollment");
        ApiError::from_csr_parse(&e)
    })?;

    let certificate = enrollment_service::enroll_device(&intermediate_dto, crypto.get_ref(), &pool)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Enrollment failed");
            ApiError::from_enrollment(&e)
        })?;

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "enrolled".to_string(),
        data: Some(serde_json::json!({"certificate": certificate})),
    }))
}
