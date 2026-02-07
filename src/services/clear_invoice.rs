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
    models::submit_invoice_dto::IntermediateInvoiceDto,
    services::{
        c14n11::canonicalize_c14n11,
        editors::{edit_signature, edit_signed_info, edit_signing_time},
        extractors::extract_signed_properties,
        pki_service::compute_hash,
        signer::sign,
    },
};

pub fn clear_invoice(
    intermediate_dto: &IntermediateInvoiceDto,
    crypto: &Crypto,
) -> anyhow::Result<(Vec<u8>, String)> {
    let invoice_hash = compute_hash(&intermediate_dto.canonicalized_invoice_bytes)?;

    let edited_signed_properties_invoice_bytes =
        edit_signing_time(&intermediate_dto.invoice_bytes)?;
    let signed_properties = extract_signed_properties(&edited_signed_properties_invoice_bytes)?;
    let signed_properties_hash = compute_hash(&canonicalize_c14n11(signed_properties)?)?;
    let edited_signed_info_invoice_bytes = edit_signed_info(
        &intermediate_dto.invoice_bytes,
        &invoice_hash,
        &signed_properties_hash,
    )?;
    let signed_info_hash = compute_hash(&canonicalize_c14n11(extract_signed_properties(
        &edited_signed_info_invoice_bytes,
    )?)?)?;
    let signature = sign(signed_info_hash, crypto)?;
    let signature_b64 = general_purpose::STANDARD.encode(signature);
    let final_invoice = edit_signature(&edited_signed_info_invoice_bytes, signature_b64)?;
    let final_invoice_b64: String = general_purpose::STANDARD.encode(final_invoice);
    
    Ok((invoice_hash, final_invoice_b64))
}
