use actix_web::{HttpResponse, web};
use uuid::Uuid;

use crate::models::onboard_dto::{onBoardResponseDto, onboardDto};

pub async fn token_generator(
    data: web::Json<onboardDto>,
) -> Result<HttpResponse, actix_web::error::Error> {
    let rand = Uuid::new_v4();
    let token = data.company.to_owned() + ":" + &rand.to_string()[..];
    let message = "here you droped it 🧠".to_owned();
    dbg!(&token);

    Ok(HttpResponse::Ok().json(onBoardResponseDto { message, token }))
}
