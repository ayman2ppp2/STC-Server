

use openssl::hash::{MessageDigest, hash};
use openssl::pkey::{PKey, Public};
use quick_xml::de::from_str;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use base64::engine::general_purpose;
use base64::Engine;
use openssl::x509::{self, X509};

use crate::get_invoices;
use crate::models::invoice_model::Invoice;
#[derive(Debug, Deserialize, Serialize,FromRow)]
pub struct SubmitInvoiceDto{
  invoice_base64 : String ,
  invoice_hash : String,
  signature_base64 : String,
  certificate_base64 : String,
}

pub struct IntermediateInvoiceDto{
  pub invoice_bytes : Vec<u8>,
  pub invoice_hash : Vec<u8>,
  pub invoice_signature : Vec<u8>,
  pub certificate :X509,
  pub public_key : PKey<Public>
}

impl SubmitInvoiceDto {
    pub fn parse(self) -> Result<IntermediateInvoiceDto,String>{
      let invoice_bytes = general_purpose::STANDARD.decode(self.invoice_base64).map_err(|_| "invalid Base64 invoice")?;
      let invoice_hash = hex::decode(self.invoice_hash).map_err(|_| "invalid invioce hash")?;
      let invoice_signature = general_purpose::STANDARD.decode(self.signature_base64).map_err(|_| "invalid base64 signature")?;
      let certificate_bytes = general_purpose::STANDARD.decode(self.certificate_base64).map_err(|_| "invalid base64 certificate")?;
      let certificate = X509::from_pem(&certificate_bytes).map_err(|_| "invalid x509 certificate")?;
      let public_key = certificate.public_key().map_err(|_| "certificate has no public key")?;
      Ok(IntermediateInvoiceDto{invoice_bytes,invoice_hash,invoice_signature,certificate,public_key})
    }
}

impl IntermediateInvoiceDto {
  pub fn parse_invoice(&self) -> Result<Invoice,String>{
    let xml_string = std::str::from_utf8(& self.invoice_bytes).map_err(|_| "invoice xml is not valid utf8")?;
    let invoice :Invoice = from_str(xml_string).map_err(|e| format!("invalid invoice XML : {}",e))?;
    Ok(invoice)
  }
  pub fn compute_hash(&self) ->Result<Vec<u8>,openssl::error::ErrorStack>{
    let digest =  hash(MessageDigest::sha256(), &self.invoice_bytes)?;
    Ok(digest.to_vec())
  }
}