use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use tracing;

use crate::{
    models::{onboard::OnBoardResponseDto, responses::ApiResponse},
    services::pipeline::onboarding_service,
};

pub async fn token_generator(
    data: web::Json<crate::models::onboard::OnboardDto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let company_id = data.company_id.clone();
    let result = onboarding_service::generate_token(&company_id, &pool).await;

    match result {
        Ok(onboarding) => {
            let message = "Token generated successfully. Use this token within 5 minutes.".to_string();
            Ok(HttpResponse::Ok().json(OnBoardResponseDto {
                message,
                token: onboarding.token,
            }))
        }
        Err(e) => {
            tracing::error!(company_id = %company_id, error = %e, "Token generation failed");
            if e.to_string().contains("not found in taxpayer registry") {
                Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                    success: false,
                    message: "Invalid company ID".to_string(),
                    data: None,
                }))
            } else {
                Ok(HttpResponse::InternalServerError().json(ApiResponse::<serde_json::Value> {
                    success: false,
                    message: "Internal server error".to_string(),
                    data: None,
                }))
            }
        }
    }
}
