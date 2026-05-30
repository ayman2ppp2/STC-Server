use actix_web::{HttpResponse, web};
use tracing;

use crate::{
    config::crypto_config::Crypto,
    models::{qr_verification::QrVerificationDto, responses::ApiResponse},
    services::crypto::verify_qr::verify_qr_signature,
};

pub async fn verify_qr(
    qr_dto: web::Json<QrVerificationDto>,
    crypto: web::Data<Crypto>,
) -> Result<HttpResponse, actix_web::Error> {
    match verify_qr_signature(&qr_dto.into_inner().qr_b64, crypto.get_ref()) {
        Ok(()) => Ok(HttpResponse::Ok().json(ApiResponse::<()> {
            success: true,
            message: "verified".into(),
            data: None,
        })),
        Err(e) => {
            tracing::error!(error = %e, "QR verification failed");
            Ok(
                HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
                    success: false,
                    message: "QR verification failed".into(),
                    data: None,
                }),
            )
        }
    }
}
