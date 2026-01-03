use crate::services::pki_service::x509_to_base64;
use actix_web::{HttpResponse, web};


use crate::{
    config::crypto_config::Crypto, 
    models::enrollment_DTO::{EnrollDTO, EnrollResponse},
    services::pki_service::handle_enrollment,
};

pub async fn enroll(
    dto: web::Json<EnrollDTO>,
    crypto: web::Data<Crypto>,
) -> Result<HttpResponse, actix_web::Error> {
    match handle_enrollment(&dto.into_inner(), crypto.get_ref()).await {
        Ok(crt) => {
            match x509_to_base64(&crt) {
                Ok(cert_b64) => {
                    let response = EnrollResponse {
                        certificate_base64: cert_b64,
                        status: "enrolled".to_string(),
                    };
                    Ok(HttpResponse::Ok().json(response))
                },
                Err(e) => {
                    Ok(HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": "Failed to encode certificate",
                        "details": e
                    })))
                }
            }
        },
        Err(e) => {
            Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Enrollment failed",
                "details": e
            })))
        }
    }
}
