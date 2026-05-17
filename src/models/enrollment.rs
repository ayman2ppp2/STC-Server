use anyhow::{anyhow, Context};
use base64::{engine::general_purpose, Engine};
use openssl::{nid::Nid, x509::X509Req};

#[derive(serde::Deserialize)]
pub struct EnrollDTO {
    pub token: String,
    pub csr: String,
}

// #[derive(serde::Serialize)]
// pub struct EnrollResponse {
//     pub certificate: String,
//     pub status: String,
// }

pub struct IntermediateEnrollDto {
    pub token: String,
    pub csr: X509Req,
}

impl EnrollDTO {
    pub fn parse(&self) -> Result<IntermediateEnrollDto, String> {
        let der = general_purpose::STANDARD
            .decode(&self.csr)
            .map_err(|e| format!("Failed to decode the der bytes : {}", e))?;
        let csr = X509Req::from_der(&der)
            .map_err(|e| format!("Failed to parse the certificate request : {}", e))?;
        Ok(IntermediateEnrollDto {
            token: self.token.clone(),
            csr,
        })
    }
}

impl IntermediateEnrollDto {
    pub fn get_device_id(&self) -> anyhow::Result<String> {
        let entry = self
            .csr
            .subject_name()
            .entries_by_nid(Nid::SERIALNUMBER)
            .next()
            .ok_or_else(|| anyhow!("CSR is missing the Serial Number (device ID)"))?;

        let device_id = entry
            .data()
            .as_utf8()
            .context("Failed to parse device ID as valid UTF-8")?
            .to_string();

        Ok(device_id)
    }

    pub fn get_tin(&self) -> anyhow::Result<String> {
        let entry = self
            .csr
            .subject_name()
            .entries_by_nid(Nid::ORGANIZATIONNAME)
            .next()
            .ok_or_else(|| anyhow!("CSR is missing the ORGANIZATIONNAME (TIN)"))?;

        let tin = entry
            .data()
            .as_utf8()
            .context("Failed to parse TIN as valid UTF-8")?
            .to_string();

        Ok(tin)
    }
}
