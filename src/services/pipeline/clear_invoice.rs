/*
get canonical invoice hash then build the signed poperties
then canonicalize signed properties to get its hash
then build signed info with both hashes
canonicalize signed info to get its hash
sign the signed info hash to get signature value
build QR using invoice hash + signature
*/

use base64::{Engine, engine::general_purpose};

use crate::{
    config::crypto_config::Crypto,
    models::submit_invoice::IntermediateInvoiceDto,
    services::{
        crypto::pki_service::compute_hash,
        crypto::pki_service::sign,
        xml::c14n11::canonicalize_c14n11,
        xml::editors::{
            edit_certificate, edit_qr, edit_signature, edit_signed_info, edit_signing_time,
        },
        xml::extractors::{extract_signed_info, extract_signed_properties},
    },
};

pub fn clear_invoice(
    intermediate_dto: &IntermediateInvoiceDto,
    crypto: &Crypto,
) -> anyhow::Result<(Vec<u8>, String)> {
    let invoice_hash = compute_hash(&intermediate_dto.canonicalized_invoice_bytes)?;
    // edit signing time
    let edited_signed_properties_invoice_bytes =
        edit_signing_time(&intermediate_dto.invoice_bytes)?;
    // extract the edited signed properties
    let signed_properties =
        extract_signed_properties(&edited_signed_properties_invoice_bytes, None)?;
    // hash the extracted signed properties
    let signed_properties_hash = compute_hash(&canonicalize_c14n11(signed_properties)?)?;
    // edit the signed info to add the new invoice hash and SP hash
    let edited_signed_info_invoice_bytes = edit_signed_info(
        &edited_signed_properties_invoice_bytes,
        &invoice_hash,
        &signed_properties_hash,
    )?;
    // compute hash for the edited signed info
    let signed_info_canonical = &canonicalize_c14n11(extract_signed_info(
        &edited_signed_info_invoice_bytes,
        None,
    )?)?;
    // let edited_qr_invoice_bytes = edit_qr(invoice_hash,signature);
    // sign the signed info hash
    let signature = sign(signed_info_canonical, crypto)?;
    let qr_signature = sign(&invoice_hash, crypto)?;
    let certificate = crypto.certificate.to_der()?;
    // base64 encoding
    let signature_b64 = general_purpose::STANDARD.encode(&signature);
    let certificate_b64 = general_purpose::STANDARD.encode(&certificate);
    // injecting the signature
    let signed_invoice = edit_signature(&edited_signed_info_invoice_bytes, signature_b64)?;
    let signed_invoice = edit_certificate(&signed_invoice, certificate_b64)?;
    let final_invoice = edit_qr(&signed_invoice, &invoice_hash, &qr_signature, &certificate)?;
    let final_invoice_b64: String = general_purpose::STANDARD.encode(final_invoice);
    Ok((invoice_hash, final_invoice_b64))
}
