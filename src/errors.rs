use std::fmt;

use actix_web::{
    HttpRequest, HttpResponse, ResponseError,
    body::BoxBody,
    error::{InternalError, JsonPayloadError},
    http::StatusCode,
};

use crate::models::responses::{ApiResponse, ErrorData, ErrorInfo};

#[derive(Debug, Clone, Copy)]
pub struct ApiError {
    code: ErrorCode,
}

impl ApiError {
    pub const fn new(code: ErrorCode) -> Self {
        Self { code }
    }

    pub const fn internal() -> Self {
        Self::new(ErrorCode::InternalServerError)
    }

    pub fn from_json_payload(error: &JsonPayloadError) -> Self {
        match error {
            JsonPayloadError::ContentType => Self::new(ErrorCode::UnsupportedContentType),
            JsonPayloadError::OverflowKnownLength { .. } | JsonPayloadError::Overflow { .. } => {
                Self::new(ErrorCode::RequestBodyTooLarge)
            }
            JsonPayloadError::Deserialize(_) => Self::new(ErrorCode::InvalidJson),
            JsonPayloadError::Payload(_) => Self::new(ErrorCode::RequestBodyReadError),
            _ => Self::new(ErrorCode::InvalidRequestBody),
        }
    }

    pub fn from_token_generation(error: &anyhow::Error) -> Self {
        let error_text = error_chain_text(error);

        if error_text.contains("not found in taxpayer registry") {
            Self::new(ErrorCode::CompanyIdNotRegistered)
        } else {
            Self::internal()
        }
    }

    pub fn from_csr_parse(error: &str) -> Self {
        if error.to_ascii_lowercase().contains("decode") {
            Self::new(ErrorCode::InvalidCsrEncoding)
        } else {
            Self::new(ErrorCode::InvalidCsr)
        }
    }

    pub fn from_enrollment(error: &anyhow::Error) -> Self {
        if has_database_constraint(error, "devices_pkey") {
            return Self::new(ErrorCode::DeviceAlreadyEnrolled);
        }

        if sqlx_error(error).is_some() {
            return Self::internal();
        }

        let error_text = error_chain_text(error);
        if contains_any(&error_text, &["not found or expired", "token hash mismatch"]) {
            Self::new(ErrorCode::InvalidOrExpiredToken)
        } else if error_text.contains("missing the serial number") {
            Self::new(ErrorCode::CsrDeviceIdMissing)
        } else if error_text.contains("failed to parse device id as uuid") {
            Self::new(ErrorCode::InvalidCsrDeviceId)
        } else if error_text.contains("missing the organizationname") {
            Self::new(ErrorCode::CsrSupplierTinMissing)
        } else if error_text.contains("invalid supplier tin") {
            Self::new(ErrorCode::SupplierTinNotRegistered)
        } else if error_text.contains("error checking for the supplier tin") {
            Self::internal()
        } else if error_text.contains("utf-8") {
            Self::new(ErrorCode::InvalidCsrSubject)
        } else {
            Self::new(ErrorCode::EnrollmentFailed)
        }
    }

    pub fn from_invoice_parse(error: &anyhow::Error) -> Self {
        if matches!(sqlx_error(error), Some(error) if matches!(error, sqlx::Error::RowNotFound)) {
            return Self::new(ErrorCode::DeviceNotFound);
        }

        if sqlx_error(error).is_some() {
            return Self::internal();
        }

        let error_text = error_chain_text(error);
        if error_text.contains("failed to decode the the invoice") {
            Self::new(ErrorCode::InvalidInvoiceEncoding)
        } else if error_text.contains("failed to decode the invoice hash") {
            Self::new(ErrorCode::InvalidInvoiceHashEncoding)
        } else if contains_any(
            &error_text,
            &[
                "failed to optain a valid uuid",
                "failed to obtain a valid uuid",
            ],
        ) {
            Self::new(ErrorCode::InvalidInvoiceUuid)
        } else if contains_any(
            &error_text,
            &[
                "failed to extract the certificate",
                "failed to decode the the certificate",
                "failed to create a certificate",
            ],
        ) {
            Self::new(ErrorCode::InvalidInvoiceCertificate)
        } else if error_text.contains("failed to extract the company id") {
            Self::new(ErrorCode::InvalidSupplierTin)
        } else if contains_any(
            &error_text,
            &[
                "invalid xml",
                "xml error",
                "failed to canonicalize the invoice",
            ],
        ) {
            Self::new(ErrorCode::InvalidInvoiceXml)
        } else {
            Self::new(ErrorCode::InvalidInvoiceData)
        }
    }

    pub fn from_invoice_pipeline(error: &anyhow::Error) -> Self {
        let error_text = error_chain_text(error);

        if error_text.contains("invoice uuid already exists")
            || has_database_constraint(error, "invoices_pkey")
            || has_database_constraint(error, "invoices_uuid_unique")
        {
            return Self::new(ErrorCode::DuplicateInvoiceUuid);
        }

        if has_database_constraint(error, "idx_invoices_hash") {
            return Self::new(ErrorCode::DuplicateInvoiceHash);
        }

        if matches!(sqlx_error(error), Some(error) if matches!(error, sqlx::Error::RowNotFound)) {
            return Self::new(ErrorCode::DeviceNotFound);
        }

        if sqlx_error(error).is_some() {
            return Self::internal();
        }

        if error_text.contains("invoice hash mismatch") {
            Self::new(ErrorCode::InvoiceHashMismatch)
        } else if error_text.contains("invoice type mismatch") {
            Self::new(ErrorCode::InvoiceTypeMismatch)
        } else if error_text.contains("schema") {
            Self::new(ErrorCode::InvoiceSchemaInvalid)
        } else if error_text.contains("icv mismatch") {
            Self::new(ErrorCode::InvoiceSequenceMismatch)
        } else if error_text.contains("pih") {
            Self::new(ErrorCode::InvoiceChainMismatch)
        } else if error_text.contains("customer tin equals supplier tin") {
            Self::new(ErrorCode::CustomerSupplierTinMatch)
        } else if error_text.contains("invalid customer tin") {
            Self::new(ErrorCode::CustomerTinNotRegistered)
        } else if error_text.contains("invalid supplier tin") {
            Self::new(ErrorCode::SupplierTinNotRegistered)
        } else if error_text.contains("supplier tin mismatch") {
            Self::new(ErrorCode::SupplierTinMismatch)
        } else if contains_any(&error_text, &["certificate", "signature", "xades"]) {
            Self::new(ErrorCode::InvoiceSignatureInvalid)
        } else if contains_any(
            &error_text,
            &["not valid utf-8", "invalid xml", "xml error"],
        ) {
            Self::new(ErrorCode::InvalidInvoiceXml)
        } else {
            Self::new(ErrorCode::InvoiceValidationFailed)
        }
    }

    pub fn from_qr(error: &anyhow::Error) -> Self {
        let error_text = error_chain_text(error);

        if contains_any(
            &error_text,
            &["invalid byte", "invalid padding", "encoded text"],
        ) {
            Self::new(ErrorCode::InvalidQrEncoding)
        } else if error_text.contains("missing invoice hash tag") {
            Self::new(ErrorCode::QrHashMissing)
        } else if error_text.contains("missing signature tag") {
            Self::new(ErrorCode::QrSignatureMissing)
        } else if error_text.contains("missing certificate tag") {
            Self::new(ErrorCode::QrCertificateMissing)
        } else if contains_any(
            &error_text,
            &[
                "truncated tlv",
                "unsupported tlv",
                "tlv length overflow",
            ],
        ) {
            Self::new(ErrorCode::InvalidQrTlv)
        } else if error_text.contains("certificate does not match") {
            Self::new(ErrorCode::QrCertificateMismatch)
        } else if error_text.contains("invalid qr signature") {
            Self::new(ErrorCode::QrSignatureInvalid)
        } else {
            Self::new(ErrorCode::QrVerificationFailed)
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code.message())
    }
}

impl ResponseError for ApiError {
    fn status_code(&self) -> StatusCode {
        self.code.status()
    }

    fn error_response(&self) -> HttpResponse<BoxBody> {
        HttpResponse::build(self.status_code()).json(ApiResponse {
            success: false,
            message: self.code.message().to_string(),
            data: Some(ErrorData {
                error: ErrorInfo {
                    code: self.code.as_str(),
                },
            }),
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ErrorCode {
    UnsupportedContentType,
    RequestBodyTooLarge,
    InvalidJson,
    RequestBodyReadError,
    InvalidRequestBody,
    InternalServerError,
    CompanyIdNotRegistered,
    InvalidCsrEncoding,
    InvalidCsr,
    InvalidOrExpiredToken,
    DeviceAlreadyEnrolled,
    CsrDeviceIdMissing,
    InvalidCsrDeviceId,
    CsrSupplierTinMissing,
    SupplierTinNotRegistered,
    InvalidCsrSubject,
    EnrollmentFailed,
    DeviceNotFound,
    DeviceInactive,
    InvalidInvoiceEncoding,
    InvalidInvoiceHashEncoding,
    InvalidInvoiceUuid,
    InvalidInvoiceCertificate,
    InvalidSupplierTin,
    InvalidInvoiceXml,
    InvalidInvoiceData,
    DuplicateInvoiceUuid,
    DuplicateInvoiceHash,
    InvoiceHashMismatch,
    InvoiceTypeMismatch,
    InvoiceSchemaInvalid,
    InvoiceSequenceMismatch,
    InvoiceChainMismatch,
    CustomerSupplierTinMatch,
    CustomerTinNotRegistered,
    SupplierTinMismatch,
    InvoiceSignatureInvalid,
    InvoiceValidationFailed,
    InvalidQrEncoding,
    QrHashMissing,
    QrSignatureMissing,
    QrCertificateMissing,
    InvalidQrTlv,
    QrCertificateMismatch,
    QrSignatureInvalid,
    QrVerificationFailed,
}

impl ErrorCode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedContentType => "unsupported_content_type",
            Self::RequestBodyTooLarge => "request_body_too_large",
            Self::InvalidJson => "invalid_json",
            Self::RequestBodyReadError => "request_body_read_error",
            Self::InvalidRequestBody => "invalid_request_body",
            Self::InternalServerError => "internal_server_error",
            Self::CompanyIdNotRegistered => "company_id_not_registered",
            Self::InvalidCsrEncoding => "invalid_csr_encoding",
            Self::InvalidCsr => "invalid_csr",
            Self::InvalidOrExpiredToken => "invalid_or_expired_token",
            Self::DeviceAlreadyEnrolled => "device_already_enrolled",
            Self::CsrDeviceIdMissing => "csr_device_id_missing",
            Self::InvalidCsrDeviceId => "invalid_csr_device_id",
            Self::CsrSupplierTinMissing => "csr_supplier_tin_missing",
            Self::SupplierTinNotRegistered => "supplier_tin_not_registered",
            Self::InvalidCsrSubject => "invalid_csr_subject",
            Self::EnrollmentFailed => "enrollment_failed",
            Self::DeviceNotFound => "device_not_found",
            Self::DeviceInactive => "device_inactive",
            Self::InvalidInvoiceEncoding => "invalid_invoice_encoding",
            Self::InvalidInvoiceHashEncoding => "invalid_invoice_hash_encoding",
            Self::InvalidInvoiceUuid => "invalid_invoice_uuid",
            Self::InvalidInvoiceCertificate => "invalid_invoice_certificate",
            Self::InvalidSupplierTin => "invalid_supplier_tin",
            Self::InvalidInvoiceXml => "invalid_invoice_xml",
            Self::InvalidInvoiceData => "invalid_invoice_data",
            Self::DuplicateInvoiceUuid => "duplicate_invoice_uuid",
            Self::DuplicateInvoiceHash => "duplicate_invoice_hash",
            Self::InvoiceHashMismatch => "invoice_hash_mismatch",
            Self::InvoiceTypeMismatch => "invoice_type_mismatch",
            Self::InvoiceSchemaInvalid => "invoice_schema_invalid",
            Self::InvoiceSequenceMismatch => "invoice_sequence_mismatch",
            Self::InvoiceChainMismatch => "invoice_chain_mismatch",
            Self::CustomerSupplierTinMatch => "customer_supplier_tin_match",
            Self::CustomerTinNotRegistered => "customer_tin_not_registered",
            Self::SupplierTinMismatch => "supplier_tin_mismatch",
            Self::InvoiceSignatureInvalid => "invoice_signature_invalid",
            Self::InvoiceValidationFailed => "invoice_validation_failed",
            Self::InvalidQrEncoding => "invalid_qr_encoding",
            Self::QrHashMissing => "qr_hash_missing",
            Self::QrSignatureMissing => "qr_signature_missing",
            Self::QrCertificateMissing => "qr_certificate_missing",
            Self::InvalidQrTlv => "invalid_qr_tlv",
            Self::QrCertificateMismatch => "qr_certificate_mismatch",
            Self::QrSignatureInvalid => "qr_signature_invalid",
            Self::QrVerificationFailed => "qr_verification_failed",
        }
    }

    const fn message(self) -> &'static str {
        match self {
            Self::UnsupportedContentType => "Content-Type must be application/json",
            Self::RequestBodyTooLarge => "Request body is too large",
            Self::InvalidJson => "Request body must be valid JSON",
            Self::RequestBodyReadError => "Request body could not be read",
            Self::InvalidRequestBody => "Request body is invalid",
            Self::InternalServerError => "Internal server error",
            Self::CompanyIdNotRegistered => "Company ID is not registered",
            Self::InvalidCsrEncoding => "CSR must be valid base64",
            Self::InvalidCsr => "CSR is invalid",
            Self::InvalidOrExpiredToken => "Invalid or expired token",
            Self::DeviceAlreadyEnrolled => "Device is already enrolled",
            Self::CsrDeviceIdMissing => "CSR is missing the device ID",
            Self::InvalidCsrDeviceId => "CSR device ID must be a valid UUID",
            Self::CsrSupplierTinMissing => "CSR is missing the supplier TIN",
            Self::SupplierTinNotRegistered => "Supplier TIN not registered",
            Self::InvalidCsrSubject => "CSR contains invalid text fields",
            Self::EnrollmentFailed => "Enrollment failed",
            Self::DeviceNotFound => "Device is not enrolled",
            Self::DeviceInactive => "Device is not enabled",
            Self::InvalidInvoiceEncoding => "Invoice must be valid base64",
            Self::InvalidInvoiceHashEncoding => "Invoice hash must be valid base64",
            Self::InvalidInvoiceUuid => "Invoice UUID is invalid",
            Self::InvalidInvoiceCertificate => "Invoice certificate is invalid",
            Self::InvalidSupplierTin => "Invoice supplier TIN is missing or invalid",
            Self::InvalidInvoiceXml => "Invoice XML is invalid",
            Self::InvalidInvoiceData => "Invalid invoice data",
            Self::DuplicateInvoiceUuid => "Invoice UUID already exists",
            Self::DuplicateInvoiceHash => "Invoice was already submitted",
            Self::InvoiceHashMismatch => "Invoice hash does not match invoice content",
            Self::InvoiceTypeMismatch => "Invoice type does not match endpoint",
            Self::InvoiceSchemaInvalid => "Invoice XML does not match required schema",
            Self::InvoiceSequenceMismatch => "Invoice sequence is out of order",
            Self::InvoiceChainMismatch => "Invoice chain validation failed",
            Self::CustomerSupplierTinMatch => "Customer TIN cannot match supplier TIN",
            Self::CustomerTinNotRegistered => "Customer TIN not registered",
            Self::SupplierTinMismatch => "Supplier TIN does not match invoice certificate or device",
            Self::InvoiceSignatureInvalid => "Invoice signature or certificate is invalid",
            Self::InvoiceValidationFailed => "Invoice failed validation",
            Self::InvalidQrEncoding => "QR payload must be valid base64",
            Self::QrHashMissing => "QR payload is missing the invoice hash",
            Self::QrSignatureMissing => "QR payload is missing the signature",
            Self::QrCertificateMissing => "QR payload is missing the certificate",
            Self::InvalidQrTlv => "QR payload is malformed",
            Self::QrCertificateMismatch => "QR certificate does not match this server",
            Self::QrSignatureInvalid => "QR signature is invalid",
            Self::QrVerificationFailed => "QR verification failed",
        }
    }

    const fn status(self) -> StatusCode {
        match self {
            Self::UnsupportedContentType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Self::RequestBodyTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::CompanyIdNotRegistered
            | Self::DeviceNotFound
            | Self::SupplierTinNotRegistered
            | Self::CustomerTinNotRegistered => StatusCode::NOT_FOUND,
            Self::DeviceAlreadyEnrolled
            | Self::DuplicateInvoiceUuid
            | Self::DuplicateInvoiceHash
            | Self::InvoiceSequenceMismatch
            | Self::InvoiceChainMismatch => StatusCode::CONFLICT,
            Self::DeviceInactive => StatusCode::FORBIDDEN,
            _ => StatusCode::BAD_REQUEST,
        }
    }
}

pub fn json_error_handler(error: JsonPayloadError, _req: &HttpRequest) -> actix_web::Error {
    let api_error = ApiError::from_json_payload(&error);
    InternalError::from_response(error, api_error.error_response()).into()
}

fn error_chain_text(error: &anyhow::Error) -> String {
    error
        .chain()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" | ")
        .to_ascii_lowercase()
}

fn sqlx_error(error: &anyhow::Error) -> Option<&sqlx::Error> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<sqlx::Error>())
}

fn has_database_constraint(error: &anyhow::Error, constraint: &str) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<sqlx::Error>()
            .and_then(|error| match error {
                sqlx::Error::Database(database_error) => database_error.constraint(),
                _ => None,
            })
            == Some(constraint)
    })
}

fn contains_any(text: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| text.contains(pattern))
}
