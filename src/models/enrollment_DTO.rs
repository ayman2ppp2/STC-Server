use base64::{Engine, engine::general_purpose};
use openssl::x509::X509Req;
#[derive(serde::Deserialize)]
pub struct EnrollDTO {
  pub csr_base64 : String,
}

#[derive(serde::Serialize)]
pub struct EnrollResponse {
  pub certificate_base64: String,
  pub status: String,
}

pub struct IntermediateEnrollDto{
  pub csr : X509Req,
}

impl EnrollDTO{
  pub async fn parse(&self) -> Result<IntermediateEnrollDto,String>{
    let certificate_bytes = general_purpose::STANDARD.decode(& self.csr_base64).map_err(|_|
      "the certificate is not valid base64"
    )?;
    let csr = X509Req::from_der(&certificate_bytes).map_err(|_| "Failed to parse the certificate")?;
    Ok(IntermediateEnrollDto { csr })
  }

}

