use std::str::FromStr;

use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose;

use openssl::x509::X509;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tracing::instrument;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::models::device::Device;
use crate::services::db::device_service::get_device;
use crate::services::xml::c14n11::canonicalize_c14n11;
use crate::services::xml::extractors::{extract_crt, extract_invoice, extract_supplier_id};
#[derive(Debug, Clone, Deserialize, Serialize, FromRow, ToSchema)]
pub struct SubmitInvoiceDto {
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub uuid: String,
    #[schema(example = "BASE64_SHA256_HASH_OF_CANONICAL_INVOICE")]
    pub invoice_hash: String,
    #[schema(example = "BASE64_UBL_INVOICE_XML")]
    pub invoice: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ClearedInvoiceDto {
    #[schema(example = "BASE64_CLEARED_INVOICE_XML")]
    pub cleared_invoice: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InvoiceType {
    Reporting,
    Clearance,
}

impl InvoiceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            InvoiceType::Reporting => "reporting",
            InvoiceType::Clearance => "clearance",
        }
    }
}

pub struct IntermediateInvoiceDto {
    pub uuid: Uuid,
    pub invoice_bytes: Vec<u8>,
    pub canonicalized_invoice_bytes: Vec<u8>,
    pub invoice_hash: Vec<u8>,
    pub certificate: X509,
    pub supplier: String,
    pub device: Device,
}

impl SubmitInvoiceDto {
    #[instrument(skip(self, pool), fields(uuid = %self.uuid, invoice_b64_len = self.invoice.len()))]
    pub async fn parse(self, pool: &PgPool) -> anyhow::Result<IntermediateInvoiceDto> {
        let invoice_bytes = general_purpose::STANDARD
            .decode(self.invoice)
            .context("failed to decode the the invoice")?;
        let certificate =
            extract_crt(&invoice_bytes).context("failed to extract the certificate")?;
        let canonicalized_invoice_bytes = canonicalize_c14n11(extract_invoice(&invoice_bytes)?)
            .context("failed to canonicalize the invoice")?;

        let invoice_hash = general_purpose::STANDARD
            .decode(self.invoice_hash)
            .context("failed to decode the invoice hash")?;

        let certificate = general_purpose::STANDARD
            .decode(certificate)
            .context("failed to decode the the certificate")?;

        let certificate = X509::from_der(&certificate)
            .context("failed to create a certificate from the der file")?;

        let uuid = Uuid::from_str(&self.uuid)
            .context("failed to optain a valid uuid from the provided uuid")?;
        let supplier = extract_supplier_id(&invoice_bytes)
            .context("failed to extract the company id from the invoice")?;
        let device = get_device(&certificate, pool).await?;
        Ok(IntermediateInvoiceDto {
            uuid,
            invoice_bytes,
            canonicalized_invoice_bytes,
            invoice_hash,
            certificate,
            supplier,
            device,
        })
    }
}
