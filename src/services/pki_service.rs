use crate::services::signer::sign_csr;
use crate::{config::crypto_config::Crypto, models::enrollment_dto::EnrollDTO};
use anyhow::{Context, anyhow};
use base64::Engine;
use base64::engine::general_purpose;
use openssl::hash::hash;
use openssl::{asn1::Asn1Time, hash::MessageDigest, sign::Verifier, x509::X509};

pub async fn handle_enrollment(dto: &EnrollDTO, crypto: &Crypto) -> Result<String, String> {
    let intermediate = dto.parse().await?;
    let pubkey = &intermediate
        .csr
        .public_key()
        .map_err(|e| format!("error exracting the public key :{}", e))?;
    if !intermediate
        .csr
        .verify(pubkey)
        .map_err(|e| format!("invalid CSR : {}", e))?
    {
        return Err("CSR verficiation failed".to_string());
    }
    let certificate = sign_csr(intermediate.csr, crypto).await.map_err(|e| {
        format!(
            "an error with the creation and signing of the certificate :{}",
            e
        )
    })?;
    let certificate = certificate.to_pem().map_err(|e| {
        format!(
            "failed to convert the X509 certificate to a pem certificate : {}",
            e
        )
    })?;
    String::from_utf8(certificate)
        .map_err(|e| format!("failed to convert the certificate to a String : {}", e))
}

pub fn x509_to_base64(cert: &X509) -> anyhow::Result<String> {
    let der_bytes = cert
        .to_pem()
        .context("Failed to convert certificate to DER")?;
    Ok(general_purpose::STANDARD.encode(&der_bytes))
}

// wite a function that gets submit_invoice certificate and then verifies it returning a bool

pub async fn verify_cert_with_ca(ca_crt: &X509, client_crt: &X509) -> anyhow::Result<bool> {
    let now = Asn1Time::days_from_now(0)?;
    if client_crt.not_before() > now || client_crt.not_after() < now {
        return Err(anyhow!("dates not valid"));
    }
    let ca_pub_key = ca_crt.public_key()?;
    Ok(client_crt.verify(&ca_pub_key)?)
}

pub async fn verify_signature_with_cert(
    recv_hash: &Vec<u8>,
    sig: &Vec<u8>,
    crt: &X509,
) -> anyhow::Result<bool> {
    let pkey = crt
        .public_key()
        .context("failed to extract the public key from the provided cerificate ")?;
    let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey)
        .context("failed to instantiate the verifier : {}")?;

    verifier
        .update(recv_hash)
        .map_err(|e| format!("failed to feed the hash to the verifier : {}", e))?;

    let result = verifier
        .verify(sig)
        .map_err(|e| format!("failed to verify the signature : {}", e))?;
    Ok(result)
}

pub fn compute_hash(bytes: &Vec<u8>) -> anyhow::Result<Vec<u8>> {
    let digest = hash(MessageDigest::sha256(), &bytes)?;
    Ok(digest.to_vec())
}
