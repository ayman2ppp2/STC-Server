use actix_web::{HttpResponse, web};
use openssl::memcmp;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    config::crypto_config::Crypto,
    models::{enrollment_dto::EnrollDTO, responses::ApiResponse},
    services::{pki_service::enroll_device, token_checking::{fetch_token_hash, mark_token_used}},
};

pub async fn enroll(
    dto: web::Json<EnrollDTO>,
    crypto: web::Data<Crypto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // extract the company id from the csr and comapre it with the company id associated with the token the user provides
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

    let extracted_device_id = match intermediate_dto.get_device_id() {
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
    //fetch for token that have the same device id if any ,
    let fetched_token = match fetch_token_hash(&extracted_device_id, &pool).await {
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
    // compare the received token hash with the fetched token hash,
    match fetched_token {
        Some(token_hash) => {
            if !memcmp::eq(&intermediate_dto.token, &token_hash) {
                return Err(actix_web::error::ErrorBadRequest("token hash mismatch"));
            }
            // if valid handle the enrollment otherwise , return an invalid token error,
            match enroll_device(&intermediate_dto, crypto.get_ref(), &pool).await {
                Ok(crt) => {
                    // Mark token as used after successful enrollment
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
