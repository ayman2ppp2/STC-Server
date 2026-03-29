use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    models::onboard_dto::{OnBoardResponseDto, OnboardDto},
    services::{pki_service::compute_hash, token_checking::validate_taxpayer_exists},
};

use crate::models::responses::ApiResponse;

pub async fn token_generator(
    data: web::Json<OnboardDto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // Validate that company_id (TIN) exists in taxpayers table
    match validate_taxpayer_exists(&data.company_id, &pool).await {
        Ok(true) => {}
        Ok(false) => {
            return Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                success: false,
                message: "Invalid company ID".to_string(),
                data: Some(json!({"details": "Company ID not found in taxpayer registry"})),
            }));
        }
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(ApiResponse::<serde_json::Value> {
                success: false,
                message: "Internal server error".to_string(),
                data: Some(json!({"details": e.to_string()})),
            }));
        }
    }

    let rand = Uuid::new_v4();
    let token = data.company_id.to_owned() + ":" + &rand.to_string()[..];
    let message = "Token generated successfully. Use this token within 5 minutes.".to_string();
    let hash = compute_hash(token.as_bytes()).map_err(actix_web::error::ErrorInternalServerError)?;
    sqlx::query!(
        "INSERT INTO csr_challenges (token_hash,company_id)
        VALUES($1,$2)",
        hash,
        &data.company_id,
    )
    .execute(pool.get_ref())
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(OnBoardResponseDto { message, token }))
}