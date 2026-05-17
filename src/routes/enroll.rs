use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;

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
            return Ok(
                HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                    success: false,
                    message: "CSR Parsing Error".to_string(),
                    data: Some(json!({"details": e.to_string()})),
                }),
            );
        }
    };

    match enrollment_service::enroll_device(&intermediate_dto, crypto.get_ref(), &pool).await {
        Ok(crt) => Ok(HttpResponse::Ok().json(ApiResponse {
            success: true,
            message: "enrolled".to_string(),
            data: Some(json!({"certificate": crt})),
        })),
        Err(e) => {
            let msg = e.to_string();
            let status = if msg.contains("not found or expired") || msg.contains("hash mismatch")
            {
                "Invalid or expired token"
            } else if msg.contains("Supplier TIN not found in database") {
                "Supplier TIN not registered"
            } else {
                "Enrollment failed"
            };
            Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                success: false,
                message: status.to_string(),
                data: Some(json!({"details": msg})),
            }))
        }
    }
}
