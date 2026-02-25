use base64::{Engine, engine::general_purpose};

use crate::{config::crypto_config::{ Crypto}, services::{edit_tlv::extract_records, pki_service::verify_signature_with_cert}};

pub fn verify_qr_signature(qr_b64 : &str ,crypto :&Crypto) -> anyhow::Result<()>{
    let tlv_bytes = general_purpose::STANDARD.decode(qr_b64)?;
    let records = extract_records(&tlv_bytes);
    let mut signature : Vec<u8> = Vec::new();
    let mut hash : Vec<u8> = Vec::new();
    for (tag , value) in records {
        match tag {
            6 => hash = value,
            7 => signature = value,
            _ => {},
        }
    }
    verify_signature_with_cert(&hash, &signature, &crypto.certificate)?;
    Ok(())
}