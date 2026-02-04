use std::str::FromStr;

use base64::Engine;
use base64::engine::general_purpose;

use openssl::x509::X509;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

use crate::services::c14n11::canonicalize_c14n11;
use crate::services::extractors::{ extract_company_id, extract_invoice, extract_sig_crt};
#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct SubmitInvoiceDto {
    uuid: String,
    invoice_hash: String,
    invoice: String,
}

pub struct IntermediateInvoiceDto {
    pub uuid:Uuid,
    pub invoice_bytes: Vec<u8>,
    pub canonicalized_invoice_bytes: Vec<u8>,
    pub invoice_hash: Vec<u8>,
    pub invoice_signature: Vec<u8>,
    pub certificate: X509,
    pub company : String,
}

impl SubmitInvoiceDto {
    pub fn parse(self) -> anyhow::Result<IntermediateInvoiceDto> {
        let invoice_bytes = general_purpose::STANDARD
            .decode(self.invoice)
            ?;
        let (signature, certificate) = extract_sig_crt(
            &invoice_bytes
        )?;
        let canonicalized_invoice_bytes = canonicalize_c14n11(extract_invoice(&invoice_bytes)?)?;

        let invoice_hash = general_purpose::STANDARD
            .decode(self.invoice_hash)
            ?;
        let invoice_signature = general_purpose::STANDARD
            .decode(signature)
            ?;

        let certificate = general_purpose::STANDARD
            .decode(certificate)
            ?;

        let certificate = X509::from_pem(&certificate)?;
        
        let uuid = Uuid::from_str(&self.uuid)?;
        let company = extract_company_id(&invoice_bytes)?;
        Ok(IntermediateInvoiceDto {
            uuid,
            invoice_bytes,
            canonicalized_invoice_bytes,
            invoice_hash,
            invoice_signature,
            certificate,
            company
        })
    }
}


