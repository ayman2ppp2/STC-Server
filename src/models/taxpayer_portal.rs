use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct TaxpayerCredentialsDto {
    pub tin: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct TaxpayerDto {
    pub tin: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct EnrollmentTokenDto {
    pub tin: String,
    pub name: String,
    pub token: String,
    pub expires_in_seconds: u16,
}

#[derive(Debug, Deserialize)]
pub struct InvoicePayloadDto {
    pub invoice_xml: String,
}

#[derive(Debug, Serialize)]
pub struct PreparedInvoicePayloadDto {
    pub invoice: String,
    pub invoice_hash: String,
}
