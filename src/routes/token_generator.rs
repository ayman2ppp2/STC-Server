use actix_web::{HttpResponse, web};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    models::onboard_dto::{OnBoardResponseDto, OnboardDto},
    services::pki_service::compute_hash,
};

pub async fn token_generator(
    data: web::Json<OnboardDto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::error::Error> {
    let rand = Uuid::new_v4();
    let token = data.company_id.to_owned() + ":" + &rand.to_string()[..];
    let message = "here you droped it 🧠".to_owned();
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
