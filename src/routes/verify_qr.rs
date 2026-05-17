use actix_web::{HttpResponse, web};
use serde_json::json;

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
            message: "verfied".into(),
            data: None,
        })),
        Err(e) => Ok(HttpResponse::BadRequest().json(ApiResponse::<serde_json::Value> {
            success: false,
            message: "QR verification failed".into(),
            data: Some(json!({"details": e.to_string()})),
        })),
    }
}
