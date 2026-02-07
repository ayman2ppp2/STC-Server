use anyhow::{Context, anyhow};
use openssl::{nid::Nid, x509::X509Req};
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
    pub fn parse(&self) -> Result<IntermediateEnrollDto, String> {
        // let certificate_bytes = general_purpose::STANDARD.decode(& self.csr).map_err(|_|
        //   "the certificate request is not valid base64"
        // )?;

        let csr = X509Req::from_pem(self.csr.as_bytes())
            .map_err(|e| format!("Failed to parse the certificate request : {}", e))?;
        Ok(IntermediateEnrollDto { csr })
    }
}
impl IntermediateEnrollDto {
    pub fn get_company_id(&self) -> anyhow::Result<String> {
        // 1. Use entries_by_nid to find the SPECIFIC field (e.g., Common Name)
        // If your Company ID is in the "Serial Number" field, change Nid::COMMON_NAME to Nid::SERIAL_NUMBER
        let entry = self.csr
            .subject_name()
            .entries_by_nid(Nid::SERIALNUMBER) 
            .next()
            .ok_or_else(|| anyhow!("CSR is missing the Serial Number (Company ID)"))?;

        // 2. Safely convert the ASN1 string to a Rust String
        let company_id = entry
            .data()
            .as_utf8()
            .context("Failed to parse Company ID as valid UTF-8")?
            .to_string();

        Ok(company_id)
    }
}
