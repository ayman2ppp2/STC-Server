use base64::Engine;
use base64::engine::general_purpose;
use openssl::hash::{MessageDigest, hash};
use openssl::pkey::{PKey, Public};
use openssl::x509::X509;
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

use crate::models::invoice_model::Invoice;
use crate::services::c14n11::canonicalize_c14n11;
use crate::services::extractors::{ extract_invoice, extract_sig_crt};
#[derive(Debug, Deserialize, Serialize, FromRow)]
pub struct SubmitInvoiceDto {
    uuid: String,
    invoice_hash: String,
    invoice: String,
}

pub struct IntermediateInvoiceDto {
    pub invoice_bytes: Vec<u8>,
    pub canonicalized_invoice_bytes: Vec<u8>,
    pub invoice_hash: Vec<u8>,
    pub invoice_signature: Vec<u8>,
    pub certificate: X509,
    pub public_key: PKey<Public>,
}

impl SubmitInvoiceDto {
    pub fn parse(self) -> Result<IntermediateInvoiceDto, String> {
        let invoice_bytes = general_purpose::STANDARD
            .decode(self.invoice)
            .map_err(|_| "invalid Base64 invoice")?;
        let (signature, certificate) = extract_sig_crt(
            &String::from_utf8(invoice_bytes.clone())
                .map_err(|_| "failed to parse the invoice bytes")?,
        );
        let canonicalized_invoice_bytes = canonicalize_c14n11(extract_invoice(&invoice_bytes)?)?;

        let invoice_hash = general_purpose::STANDARD
            .decode(self.invoice_hash)
            .map_err(|_| "invalid invoice hash")?;
        let invoice_signature = general_purpose::STANDARD
            .decode(signature)
            .map_err(|_| "invalid base64 signature")?;

        let certificate = general_purpose::STANDARD
            .decode(certificate)
            .map_err(|_| "invalid Base64 invoice")?;

        let certificate = X509::from_pem(&certificate).map_err(|_| "invalid x509 certificate")?;
        let public_key = certificate
            .public_key()
            .map_err(|_| "certificate has no public key")?;
        Ok(IntermediateInvoiceDto {
            invoice_bytes,
            canonicalized_invoice_bytes,
            invoice_hash,
            invoice_signature,
            certificate,
            public_key,
        })
    }
}

impl IntermediateInvoiceDto {
    pub fn parse_invoice(&self) -> Result<Invoice, String> {
        let xml_string = std::str::from_utf8(&self.invoice_bytes)
            .map_err(|_| "invoice xml is not valid utf8")?;
        let invoice: Invoice =
            from_str(xml_string).map_err(|e| format!("invalid invoice XML : {}", e))?;
        Ok(invoice)
    }
}
