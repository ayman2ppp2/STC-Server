use actix_web::{HttpResponse, web};
 
use crate::{
    config::crypto_config::Crypto,
    models::enrollment_dto::{EnrollDTO, EnrollResponse},
    services::{extractors::extract_company_id, pki_service::handle_enrollment},
};

pub async fn enroll(
    dto: web::Json<EnrollDTO>,
    crypto: web::Data<Crypto>,
) -> Result<HttpResponse, actix_web::Error> {
    // extract the company id from the csr and comapre it with the company id associated with the token the user provides
    let intermediate_dto=&dto.into_inner().parse().map_err(|e|actix_web::error::ErrorBadRequest(e))?;
    let extracted_company_id = intermediate_dto.get_company_id().map_err(|e|actix_web::error::ErrorBadRequest(e))?;
    let token_company_id = todo!();
    match handle_enrollment(&intermediate_dto, crypto.get_ref()).await {
        Ok(crt) => {
            let response = EnrollResponse {
                certificate: crt,
                status: "enrolled".to_string(),
            };
            Ok(HttpResponse::Ok().json(response))
        }
        Err(e) => Ok(HttpResponse::BadRequest().json(serde_json::json!({
            "error": "Enrollment failed",
            "details": e
        }))),
    }
}
