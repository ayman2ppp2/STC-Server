use crate::config::crypto_config::Crypto;
use crate::models::enrollment::IntermediateEnrollDto;


use anyhow::{Context, anyhow};
use openssl::bn::BigNum;
use openssl::hash::hash;
use openssl::nid::Nid;
use openssl::{asn1::Asn1Time, hash::MessageDigest, sign::{Signer, Verifier}, x509::{X509, X509Builder, X509Req}};
use tracing::instrument;
use uuid::Uuid;

#[instrument(skip(crypto, intermediate_dto))]
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
pub fn check_cert_serial(crt: &X509, extracted_serial: BigNum) -> anyhow::Result<bool> {
    let serial = crt.serial_number();
    let bn_serial = serial.to_bn()?;
    
    Ok(bn_serial == extracted_serial)
}

pub async fn sign_csr(req: &X509Req, crypto: &Crypto) -> Result<X509, openssl::error::ErrorStack> {
    let mut builder = X509Builder::new()?;
    builder.set_version(2)?;
    let mut serial = BigNum::new()?;
    serial.rand(128, openssl::bn::MsbOption::MAYBE_ZERO, false)?;
    let serial = serial.to_asn1_integer()?;
    builder.set_serial_number(&serial)?;
    builder.set_subject_name(req.subject_name())?;
    builder.set_issuer_name(crypto.certificate.issuer_name())?;
    let pubkey = req.public_key()?;
    builder.set_pubkey(&pubkey)?;
    let validty_in = Asn1Time::days_from_now(0)?;
    let validty_expr = Asn1Time::days_from_now(356)?;
    builder.set_not_before(&validty_in)?;
    builder.set_not_after(&validty_expr)?;

    builder.sign(&crypto.private_key, MessageDigest::sha256())?;

    Ok(builder.build())
}

pub fn sign(hash: Vec<u8>, crypto: &Crypto) -> anyhow::Result<Vec<u8>> {
    let mut signer = Signer::new(MessageDigest::sha256(), &crypto.private_key)?;
    signer.update(&hash)?;
    let signature = signer.sign_to_vec()?;
    Ok(signature)
}