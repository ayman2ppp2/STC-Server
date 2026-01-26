/*
get canonical invoice hash then build the signed poperties
then canonicalize signed properties to get its hash
then build signed info with both hashes
canonicalize signed info to get its hash
sign the signed info hash to get signature value
build QR using invoice hash + signature
*/

use crate::{models::submit_invoice_dto::{self, IntermediateInvoiceDto, SubmitInvoiceDto}, services::{extractors::extract_signed_properties, pki_service::compute_hash}};

pub fn clear_invoice(intermediate_dto:IntermediateInvoiceDto) ->anyhow::Result<Vec<u8>>{
    let invoice_hash = compute_hash(intermediate_dto.canonicalized_invoice_bytes);
    let singed_properties =  extract_signed_properties(&intermediate_dto.invoice_bytes);
}