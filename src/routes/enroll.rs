use actix_web::{HttpResponse, Responder, web};

use crate::models::enrollment_DTO::EnrollDTO;

pub async fn enroll(dto : web::Json<EnrollDTO>)-> Result<HttpResponse,actix_web::Error>{
  let intermediate_dto = dto.into_inner().parse().await.map_err(actix_web::error::ErrorExpectationFailed)?;


  Ok(HttpResponse::Ok().finish())
}

async fn sign_certificate(){}