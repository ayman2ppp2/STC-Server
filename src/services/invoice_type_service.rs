use anyhow::bail;

use crate::{models::submit_invoice_dto::InvoiceType, services::extractors::extract_profile_id};

pub fn verify_invoice_type(
    invoice_bytes: &[u8],
    invoice_type: &InvoiceType,
) -> anyhow::Result<bool> {
    let ex_invoice_type = match extract_profile_id(invoice_bytes) {
        Ok(invoice_id) => {
            if invoice_id.contains("reporing") {
                InvoiceType::Reporting
            } else if invoice_id.contains("clearance") {
                InvoiceType::Clearance
            } else {
                bail!(
                    "Invoice type verfication error unknown type : {}",
                    invoice_id
                )
            }
        }
        Err(e) => bail!(
            "Invoice type verfication error unable to extract the invoice type : {}",
            e
        ),
    };
    if !(&ex_invoice_type == invoice_type) {
        bail!("Invoice type verfication error invoice type mismatch , Provided : {} != Extracted :{}",invoice_type.as_str(),ex_invoice_type.as_str())
    }
    Ok(true)
}
