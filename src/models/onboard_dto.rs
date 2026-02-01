use serde::{Deserialize, Serialize};

#[derive(Debug,Deserialize,Serialize)]
pub struct onboardDto{
    pub name : String,
    pub email : String,
    pub company : String,
}
#[derive(Debug,Serialize)]
pub struct onBoardResponseDto{
    pub message : String,
    pub token : String,
}