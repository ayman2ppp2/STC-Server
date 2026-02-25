use actix_web::{HttpResponse, web};


use crate::{config::crypto_config::Crypto, models::qr_verification_model::{QrVerificationDto, QrVerificationRsponse}, services::verify_qr::verify_qr_signature};


pub async fn verify_qr(qr_dto : web::Json<QrVerificationDto>,crypto : web::Data<Crypto>)-> Result<HttpResponse, actix_web::Error> {
    verify_qr_signature(&qr_dto.into_inner().qr_b64,crypto.get_ref()).map_err(|e|actix_web::error::ErrorBadRequest(format!("{}", e)))?;
    let response = QrVerificationRsponse{
        code : 200,
        status: "verfied".to_string(),
    };
    Ok(HttpResponse::Ok().json(response))
}