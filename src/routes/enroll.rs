use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use tracing;

use crate::{
    config::crypto_config::Crypto,
    models::{enrollment::EnrollDTO, responses::ApiResponse},
    services::pipeline::enrollment_service,
};

pub async fn enroll(
    dto: web::Json<EnrollDTO>,
    crypto: web::Data<Crypto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let intermediate_dto = match dto.into_inner().parse() {
        Ok(intermediate) => intermediate,
        Err(e) => {
            tracing::error!(error = %e, "CSR parse failed in enrollment");
            return Ok(
                HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                    success: false,
                    message: "CSR Parsing Error".to_string(),
                    data: None,
                }),
            );
        }
    };

    match enrollment_service::enroll_device(&intermediate_dto, crypto.get_ref(), &pool).await {
        Ok(crt) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "enrolled".to_string(),
            data: Some(serde_json::json!({"certificate": crt})),
        })),
        Err(e) => {
            tracing::error!(error = %e, "Enrollment failed");
            let status = if e.to_string().contains("not found or expired") || e.to_string().contains("hash mismatch")
            {
                "Invalid or expired token"
            } else if e.to_string().contains("Supplier TIN not found in database") {
                "Supplier TIN not registered"
            } else {
                "Enrollment failed"
            };
            Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                success: false,
                message: status.to_string(),
                data: None,
            }))
        }
    }
}
