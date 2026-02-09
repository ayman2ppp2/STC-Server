use actix_web::{HttpResponse, web};
use openssl::memcmp;
use sqlx::PgPool;

use crate::{
    config::crypto_config::Crypto,
    models::enrollment_dto::{EnrollDTO, EnrollResponse},
    services::{extractors::extract_company_id, pki_service::handle_enrollment},
};

pub async fn enroll(
    dto: web::Json<EnrollDTO>,
    crypto: web::Data<Crypto>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    // extract the company id from the csr and comapre it with the company id associated with the token the user provides
    let intermediate_dto = &dto
        .into_inner()
        .parse()
        .map_err(actix_web::error::ErrorBadRequest)?;

    let extracted_company_id = intermediate_dto
        .get_company_id()
        .map_err(actix_web::error::ErrorBadRequest)?;
    //fetch for token that have the same company id if any ,
    let fetched_token = sqlx::query!(
        r#"
    SELECT token_hash as "token_hash!"
    FROM csr_challenges
    WHERE company_id = $1
      AND used_at IS NULL
      AND expires_at > now()
    LIMIT 1
    "#,
        &extracted_company_id,
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(actix_web::error::ErrorInternalServerError)?;

    // compare the received token hash with the fetched token hash,
    match fetched_token {
        Some(token) => {
            if !memcmp::eq(intermediate_dto.token.as_bytes(), &token.token_hash) {
                return Err(actix_web::error::ErrorBadRequest("token hash mismatch"));
            }
            // if valid handle the enrollment otherwise , return an invalid token error,
            match handle_enrollment(intermediate_dto, crypto.get_ref()).await {
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
        None =>  Err(actix_web::error::ErrorBadRequest("no valid token found")),
    }
}
