
use std::fmt::format;

use base64::Engine;
use base64::engine::general_purpose;
use crate::services::signer::sign;
use openssl::x509::X509;

use crate::{config::crypto_config::Crypto, models::enrollment_DTO::EnrollDTO};

pub async fn handle_enrollment(dto : &EnrollDTO, crypto: &Crypto)-> Result<String,String>{
	let intermediate = dto.parse().await?;
	let pubkey = &intermediate.csr.public_key().map_err(|e| format!("error exracting the public key :{}",e))?;
	if !intermediate.csr.verify(pubkey).map_err(|e| format!("invalid CSR : {}",e))?{
      return Err("CSR verficiation failed".to_string());
    }
	let certificate = sign(intermediate.csr, crypto).await.map_err(|e|format!("an error with the creation and signing of the certificate :{}",e))?;
    let certificate = certificate.to_pem().map_err(|e|format!("failed to convert the X509 certificate to a pem certificate : {}",e))?;
    String::from_utf8(certificate).map_err(|e|format!("failed to convert the certificate to a String : {}",e))
}

pub fn x509_to_base64(cert: &X509) -> Result<String, String> {
    let der_bytes = cert.to_pem().map_err(|e| format!("Failed to convert certificate to DER: {}", e))?;
    Ok(general_purpose::STANDARD.encode(&der_bytes))
}
