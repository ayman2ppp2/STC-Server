use anyhow::{Context, anyhow};
use base64::{Engine, engine::general_purpose};
use openssl::{nid::Nid, x509::X509Req};
use serde::Serialize;
use utoipa::ToSchema;

#[derive(serde::Deserialize, ToSchema)]
pub struct EnrollDTO {
    #[schema(example = "100011:550e8400-e29b-41d4-a716-446655440000")]
    pub token: String,
    #[schema(example = "BASE64_DER_CSR")]
    pub csr: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EnrollmentCertificateDto {
    #[schema(example = "-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----\n")]
    pub certificate: String,
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
