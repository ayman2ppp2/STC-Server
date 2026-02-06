use serde::{Deserialize, Serialize};

#[derive(Debug,Deserialize,Serialize)]
pub struct OnboardDto{
    pub name : String,
    pub email : String,
    pub company_id : String,
}
#[derive(Debug,Serialize)]
pub struct OnBoardResponseDto{
    pub message : String,
    pub token : String,
}