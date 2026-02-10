use crate::models::enrollment_dto::IntermediateEnrollDto;
use crate::services::signer::sign_csr;
use crate::config::crypto_config::Crypto;
use anyhow::{Context, anyhow};

use openssl::hash::hash;
use openssl::{asn1::Asn1Time, hash::MessageDigest, sign::Verifier, x509::X509};

pub async fn handle_enrollment(intermediate_dto: &IntermediateEnrollDto, crypto: &Crypto) -> Result<String, String> {


    let pubkey = &intermediate_dto
        .csr
        .public_key()
        .map_err(|e| format!("error exracting the public key :{}", e))?;
    if !intermediate_dto
        .csr
        .verify(pubkey)
        .map_err(|e| format!("invalid CSR : {}", e))?
    {
        return Err("CSR verficiation failed".to_string());
    }
    let certificate = sign_csr(&intermediate_dto.csr, crypto).await.map_err(|e| {
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
    recv_hash: &[u8],
    sig: &[u8],
    crt: &X509,
) -> anyhow::Result<bool> {
    let pkey = crt
        .public_key()
        .context("failed to extract the public key from the provided cerificate ")?;
    let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey)
        .context("failed to instantiate the verifier")?;

    verifier
        .update(recv_hash)
        .context("failed to feed the hash to the verifier ")?;

    let result = verifier
        .verify(sig)
        .context("failed to verify the signature ")?;
    Ok(result)
}

pub fn compute_hash(bytes: &[u8]) -> anyhow::Result<Vec<u8>> {
    let digest = hash(MessageDigest::sha256(), bytes)?;
    Ok(digest.to_vec())
}
