use serde::{Deserialize, Serialize};

#[derive(Debug,Deserialize)]
pub struct QrVerificationDto{
   pub qr_b64 : String,
}
#[derive(Debug,Serialize)]
pub struct QrVerificationRsponse{
    pub code : u32,
    pub status : String,
}