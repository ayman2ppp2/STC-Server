use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
pub struct TaxpayerCredentialsDto {
    #[serde(default)]
    pub tin: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaxpayerDto {
    pub tin: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct TaxpayerProfileDto {
    pub tin: String,
    pub name: String,
    pub address: Option<String>,
    pub created_at: String,
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

#[derive(Debug, Default, Deserialize)]
pub struct InvoiceReportRequestDto {
    #[serde(default)]
    pub tin: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub invoice_type: Option<String>,
    #[serde(default)]
    pub search: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PreparedInvoicePayloadDto {
    pub invoice: String,
    pub invoice_hash: String,
}

#[derive(Debug, Serialize)]
pub struct InvoiceReportDto {
    pub summary: InvoiceReportSummaryDto,
    pub invoices: Vec<InvoiceReportRowDto>,
    pub filtered_total: usize,
    pub limit: usize,
    pub offset: usize,
    pub has_next: bool,
    pub has_previous: bool,
}

#[derive(Debug, Serialize)]
pub struct InvoiceReportSummaryDto {
    pub total: usize,
    pub successful: usize,
    pub failed: usize,
    pub clearance_successful: usize,
    pub clearance_failed: usize,
    pub reporting_successful: usize,
    pub reporting_failed: usize,
    pub devices: usize,
    pub latest_invoice_at: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct InvoiceReportRowDto {
    pub uuid: String,
    pub invoice_type: String,
    pub device_id: Option<String>,
    pub hash_value: String,
    pub created_at: String,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}
