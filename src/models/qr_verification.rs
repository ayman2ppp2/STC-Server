use serde::Deserialize;

#[derive(Debug,Deserialize)]
pub struct QrVerificationDto{
   pub qr_b64 : String,
}