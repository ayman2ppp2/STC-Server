use actix_web::{HttpResponse, web};
use sqlx::PgPool;

use crate::{
    errors::ApiError,
    models::onboard::{OnBoardResponseDto, OnboardDto},
    services::pipeline::onboarding_service,
};

pub async fn token_generator(
    data: web::Json<OnboardDto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    let company_id = data.company_id.clone();
    let onboarding = onboarding_service::generate_token(&company_id, &pool)
        .await
        .map_err(|e| {
            tracing::error!(company_id = %company_id, error = %e, "Token generation failed");
            ApiError::from_token_generation(&e)
        })?;

    Ok(HttpResponse::Ok().json(OnBoardResponseDto {
        message: "Token generated successfully. Use this token within 5 minutes.".to_string(),
        token: onboarding.token,
    }))
}
