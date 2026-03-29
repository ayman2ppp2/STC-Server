use actix_web::{HttpResponse, web};
use openssl::memcmp;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    config::crypto_config::Crypto,
    models::{enrollment_dto::EnrollDTO, responses::ApiResponse},
    services::{pki_service::enroll_device, token_checking::{fetch_token, mark_token_used}},
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
                    data: Some(json!({"details":e.to_string()})),
                }),
            );
        }
    };

    let _extracted_device_id = match intermediate_dto.get_device_id() {
        Ok(id) => id,
        Err(e) => {
            return Ok(
                HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                    success: false,
                    message: "Failed to fetch the device id".to_string(),
                    data: Some(json!({"details":e.to_string()})),
                }),
            );
        }
    };

    let stored_token_hash = match fetch_token(&intermediate_dto.token, &pool).await {
        Ok(token_hash) => token_hash,
        Err(e) => {
            return Ok(
                HttpResponse::InternalServerError().json(ApiResponse::<serde_json::Value> {
                    success: false,
                    message: "Internal server error".to_string(),
                    data: Some(json!({ "details": e.to_string() })),
                }),
            );
        }
    };

    match stored_token_hash {
        Some(token_hash) => {
            let token_bytes = intermediate_dto.token.as_bytes();
            let computed_hash = crate::services::pki_service::compute_hash(token_bytes)
                .map_err(|e| {
                    actix_web::error::ErrorInternalServerError(format!("Hash error: {}", e))
                })?;

            if !memcmp::eq(&computed_hash, &token_hash) {
                return Err(actix_web::error::ErrorBadRequest("token hash mismatch"));
            }

            match enroll_device(&intermediate_dto, crypto.get_ref(), &pool).await {
                Ok(crt) => {
                    if let Err(e) = mark_token_used(&token_hash, &pool).await {
                        return Ok(HttpResponse::InternalServerError().json(ApiResponse {
                            success: false,
                            message: "Enrollment succeeded but failed to mark token".to_string(),
                            data: Some(json!({"details" : e.to_string()})),
                        }));
                    }
                    Ok(HttpResponse::Ok().json(ApiResponse {
                        success: true,
                        message: "enrolled".to_string(),
                        data: Some(json!({"certificate": crt,})),
                    }))
                }
                Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse {
                    success: false,
                    message: "Enrollment failed".to_string(),
                    data: Some(json!({"details" : e.to_string()})),
                })),
            }
        }
        None => Ok(HttpResponse::BadRequest().json(ApiResponse {
                    success: false,
                    message: "Enrollment failed".to_string(),
                    data: Some(json!({"details" : "failed to find a valid token"})),
                })),
    }
}
