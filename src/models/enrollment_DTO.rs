use base64::{Engine, engine::general_purpose};
use openssl::x509::X509Req;
#[derive(serde::Deserialize)]
pub struct EnrollDTO {
    pub csr: String,
}

#[derive(serde::Serialize)]
pub struct EnrollResponse {
    pub certificate: String,
    pub status: String,
}

pub struct IntermediateEnrollDto {
    pub csr: X509Req,
}

impl EnrollDTO {
    pub async fn parse(&self) -> Result<IntermediateEnrollDto, String> {
        // let certificate_bytes = general_purpose::STANDARD.decode(& self.csr).map_err(|_|
        //   "the certificate request is not valid base64"
        // )?;

        let csr = X509Req::from_pem(self.csr.as_bytes())
            .map_err(|e| format!("Failed to parse the certificate request : {}", e))?;
        Ok(IntermediateEnrollDto { csr })
    }
}
