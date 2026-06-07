use actix_web::{HttpResponse, web};
use sqlx::PgPool;

use crate::{
    config::crypto_config::Crypto,
    errors::ApiError,
    models::{
        enrollment::{EnrollDTO, EnrollmentCertificateDto},
        responses::{ApiResponse, ErrorData},
    },
    services::pipeline::enrollment_service,
};

#[utoipa::path(
    post,
    path = "/prod/enrollment/enroll",
    tag = "Public API",
    request_body = EnrollDTO,
    responses(
        (status = 200, description = "Device enrolled", body = ApiResponse<EnrollmentCertificateDto>),
        (status = 400, description = "Invalid CSR or enrollment request", body = ApiResponse<ErrorData>),
        (status = 401, description = "Invalid or expired enrollment token", body = ApiResponse<ErrorData>),
        (status = 404, description = "Supplier TIN not registered", body = ApiResponse<ErrorData>),
        (status = 409, description = "Device is already enrolled", body = ApiResponse<ErrorData>),
        (status = 413, description = "Request body is too large", body = ApiResponse<ErrorData>),
        (status = 415, description = "Content-Type must be application/json", body = ApiResponse<ErrorData>),
        (status = 500, description = "Internal server error", body = ApiResponse<ErrorData>)
    )
)]
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
        data: Some(EnrollmentCertificateDto { certificate }),
    }))
}
