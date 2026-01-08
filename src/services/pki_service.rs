
use std::fmt::format;

use base64::Engine;
use base64::engine::general_purpose;
use crate::services::signer::sign;
use openssl::{asn1::Asn1Time, hash::MessageDigest, pkey::PKey, sign::Verifier, x509::X509};

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

// wite a function that gets submit_invoice certificate and then verifies it returning a bool

pub async fn verify_cert_with_ca(ca_crt : &X509, client_crt : &X509)-> Result<bool,String>{
    let now = Asn1Time::days_from_now(0).map_err(|e|format!("failed to get the time now :{}",e))?;
    if client_crt.not_before() > now || client_crt.not_after() < now {
        return Err("dates not valid".to_string());
    }
    let ca_pub_key = ca_crt.public_key().map_err(|e|format!("failed to extract the CA public key : {}",e))?;
    client_crt.verify(&ca_pub_key).map_err(|e|format!("certificate verficiation failed : {}",e))


}

pub async fn verify_signature_with_cert(recv_hash : &Vec<u8>, sig :&Vec<u8>,crt : &X509) -> Result<bool, String>{
    let pkey = crt.public_key().map_err(|e|format!("failed to extract the public key from the provided cerificate : {}",e))?;
    let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey).map_err(|e|format!("failed to instantiate the verifier : {}",e))?;

    verifier.update(&recv_hash).map_err(|e|format!("failed to feed the hash to the verifier : {}",e))?;

    let result = verifier.verify(&sig).map_err(|e|format!("failed to verify the signature : {}",e))?;
    Ok(result)
}