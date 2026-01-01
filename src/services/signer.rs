use openssl::{asn1::Asn1Time, bn::BigNum, hash::MessageDigest, x509::{X509, X509Builder, X509Req}};

use crate::config::crypto_config::Crypto;

pub async fn sign(req : X509Req,crypto : Crypto) -> Result<X509,openssl::error::ErrorStack>{
    let pubkey = req.public_key()?;
    if !req.verify(&pubkey)?{
      return Err(openssl::error::ErrorStack::get());
    }
    let mut builder = X509Builder::new()?;
    builder.set_version(2)?;
    let mut serial = BigNum::new()?;
    serial.rand(128, openssl::bn::MsbOption::MAYBE_ZERO, false)?;
    let serial = serial.to_asn1_integer()?;
    builder.set_serial_number(&serial)?;
    builder.set_subject_name(&req.subject_name())?;
    builder.set_issuer_name(&crypto.certificate.issuer_name())?;
    let pubkey = req.public_key()?;
    builder.set_pubkey(&pubkey)?;
    let validty_in = Asn1Time::days_from_now(0)?;
    let validty_expr = Asn1Time::days_from_now(356)?;
    builder.set_not_before(&validty_in)?;
    builder.set_not_after(&validty_expr)?;

    builder.sign(&crypto.private_key, MessageDigest::sha256())?;

    Ok(builder.build())

}