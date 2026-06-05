use actix_web::{HttpResponse, web};

use crate::{
    config::crypto_config::Crypto,
    errors::ApiError,
    models::{qr_verification::QrVerificationDto, responses::ApiResponse},
    services::crypto::verify_qr::verify_qr_signature,
};

pub async fn verify_qr(
    qr_dto: web::Json<QrVerificationDto>,
    crypto: web::Data<Crypto>,
) -> Result<HttpResponse, ApiError> {
    verify_qr_signature(&qr_dto.into_inner().qr_b64, crypto.get_ref()).map_err(|e| {
        tracing::error!(error = %e, "QR verification failed");
        ApiError::from_qr(&e)
    })?;

    Ok(HttpResponse::Ok().json(ApiResponse::<()> {
        success: true,
        message: "verified".into(),
        data: None,
    }))
}
