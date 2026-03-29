use crate::config::crypto_config::Crypto;
use crate::models::enrollment_dto::IntermediateEnrollDto;
use crate::services::device_service::create_new_device;
use crate::services::signer::sign_csr;
use crate::services::tin_service::verify_supplier_tin;
use anyhow::{Context, anyhow};

use openssl::hash::hash;
use openssl::nid::Nid;
use openssl::{asn1::Asn1Time, hash::MessageDigest, sign::Verifier, x509::X509};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn handle_enrollment(
    intermediate_dto: &IntermediateEnrollDto,
    crypto: &Crypto,
) -> anyhow::Result<String> {
    let pubkey = &intermediate_dto
        .csr
        .public_key()
        .map_err(|e| anyhow!("error exracting the public key :{}", e))?;
    if !intermediate_dto
        .csr
        .verify(pubkey)
        .map_err(|e| anyhow!("invalid CSR : {}", e))?
    {
        return Err(anyhow!("CSR verficiation failed".to_string()));
    }
    let certificate = sign_csr(&intermediate_dto.csr, crypto).await.map_err(|e| {
        anyhow!(
            "an error with the creation and signing of the certificate :{}",
            e
        )
    })?;
    let certificate = certificate.to_pem().map_err(|e| {
        anyhow!(
            "failed to convert the X509 certificate to a pem certificate : {}",
            e
        )
    })?;
    String::from_utf8(certificate)
        .map_err(|e| anyhow!("failed to convert the certificate to a String : {}", e))
}

pub async fn enroll_device(
    intermediate_dto: &IntermediateEnrollDto,
    crypto: &Crypto,
    pool: &PgPool,
) -> anyhow::Result<String> {
    let certificate = handle_enrollment(intermediate_dto, crypto).await?;

    let device_id_str = intermediate_dto.get_device_id()?;
    let device_uuid = Uuid::parse_str(&device_id_str)
        .context("Failed to parse device ID as UUID")?;
    let tin = intermediate_dto.get_tin()?;
    
    verify_supplier_tin(tin.as_bytes(), pool).await?;
    create_new_device(&device_uuid, &tin, pool).await?;

    Ok(certificate)
}

// wite a function that gets submit_invoice certificate and then verifies it returning a bool

pub async fn verify_cert_with_ca(ca_crt: &X509, client_crt: &X509) -> anyhow::Result<bool> {
    let now = Asn1Time::days_from_now(0).context("failed to generate the time in the server")?;
    if client_crt.not_before() > now || client_crt.not_after() < now {
        return Err(anyhow!("certficate is expired or yet to be used"));
    }
    let ca_pub_key = ca_crt
        .public_key()
        .context("failed to extract the public key from the cerificate")?;
    client_crt
        .verify(&ca_pub_key)
        .context("failed to verifiy the certificate with the servers CA")
}

pub fn verify_signature_with_cert(
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

pub fn extract_device_id(crt: &X509) -> anyhow::Result<Uuid> {
    let entry = crt
        .subject_name()
        .entries_by_nid(Nid::SERIALNUMBER)
        .next()
        .ok_or_else(|| anyhow!("CSR is missing the Serial Number (device ID)"))?;
    let device_id_str = entry
        .data()
        .as_utf8()
        .context("Failed to parse SERIALNUMBER as valid UTF-8")?
        .to_string();
    let device_id = Uuid::parse_str(&device_id_str).context("Failed to parse device ID as UUID")?;
    Ok(device_id)
}
pub fn verfiy_supplier_tin_with_ca(invoice_tin: &String, crt: &X509) -> anyhow::Result<()> {
    let entry = crt
        .subject_name()
        .entries_by_nid(Nid::ORGANIZATIONNAME)
        .next()
        .ok_or_else(|| anyhow!("Certificate is missing the ORGANIZATIONNAME (supplier ID)"))?;
    let crt_tin = entry
        .data()
        .as_utf8()
        .context("Failed to parse ORGANIZATIONNAME as valid UTF-8")?
        .to_string();
    if *invoice_tin == crt_tin { Ok(())} else { Err(anyhow!("Supplier TIN mismatch expected : {}, found :{}",crt_tin,invoice_tin)) }
}