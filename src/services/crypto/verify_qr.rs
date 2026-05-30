use anyhow::{Context, bail};
use base64::{Engine, engine::general_purpose};
use openssl::memcmp;
use tracing::instrument;

use crate::{
    config::crypto_config::Crypto,
    services::{crypto::pki_service::verify_signature_with_cert, xml::edit_tlv::extract_records},
};

#[instrument(skip(qr_b64, crypto), fields(qr_length = qr_b64.len()))]
pub fn verify_qr_signature(qr_b64: &str, crypto: &Crypto) -> anyhow::Result<()> {
    let tlv_bytes = general_purpose::STANDARD.decode(qr_b64)?;
    let records = extract_records(&tlv_bytes)?;
    let mut signature: Option<Vec<u8>> = None;
    let mut hash: Option<Vec<u8>> = None;
    let mut certificate: Option<Vec<u8>> = None;
    for (tag, value) in records {
        match tag {
            6 => hash = Some(value),
            7 => signature = Some(value),
            8 => certificate = Some(value),
            _ => {}
        }
    }
    let hash = hash.context("QR is missing invoice hash tag")?;
    let signature = signature.context("QR is missing signature tag")?;
    let certificate = certificate.context("QR is missing certificate tag")?;
    let server_certificate = crypto.certificate.to_der()?;
    if !memcmp::eq(&certificate, &server_certificate) {
        bail!("QR certificate does not match server certificate");
    }
    if !verify_signature_with_cert(&hash, &signature, &crypto.certificate)? {
        bail!("invalid QR signature");
    }
    Ok(())
}
