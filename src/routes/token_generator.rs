use actix_web::{HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    models::{onboard::OnBoardResponseDto, responses::ApiResponse},
    services::pipeline::onboarding_service,
};

pub async fn token_generator(
    data: web::Json<crate::models::onboard::OnboardDto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let result = onboarding_service::generate_token(&data.company_id, &pool).await;

    match result {
        Ok(onboarding) => {
            let message = "Token generated successfully. Use this token within 5 minutes.".to_string();
            Ok(HttpResponse::Ok().json(OnBoardResponseDto {
                message,
                token: onboarding.token,
            }))
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.contains("not found in taxpayer registry") {
                Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                    success: false,
                    message: "Invalid company ID".to_string(),
                    data: Some(json!({"details": msg})),
                }))
            } else {
                Ok(HttpResponse::InternalServerError().json(ApiResponse::<serde_json::Value> {
                    success: false,
                    message: "Internal server error".to_string(),
                    data: Some(json!({"details": msg})),
                }))
            }
        }
    }
}
