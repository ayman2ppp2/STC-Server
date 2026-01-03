use actix_web::{HttpResponse, web};

use crate::{
    config::crypto_config::Crypto, models::enrollment_DTO::EnrollDTO,
    services::pki_service::handle_enrollment,
};

pub async fn enroll(
    dto: web::Json<EnrollDTO>,
    crypto: web::Data<Crypto>,
) -> Result<HttpResponse, actix_web::Error> {
    handle_enrollment(&dto.into_inner(), crypto.into_inner())
        .await
        .map_err(actix_web::error::ErrorBadRequest)?;

    Ok(HttpResponse::Ok().finish())
}
