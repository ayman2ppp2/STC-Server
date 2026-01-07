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
            let response = EnrollResponse {
                        certificate: crt,
                        status: "enrolled".to_string(),
                    };
                    Ok(HttpResponse::Ok().json(response))
        },
        Err(e) => {
            Ok(HttpResponse::BadRequest().json(serde_json::json!({
                "error": "Enrollment failed",
                "details": e
            })))
        }
    }
}
