use utoipa::OpenApi;

use crate::{
    models::{
        enrollment::{EnrollDTO, EnrollmentCertificateDto},
        responses::{ApiResponse, EmptyApiResponse, ErrorData, ErrorInfo},
        submit_invoice::{ClearedInvoiceDto, SubmitInvoiceDto},
    },
    routes::{enroll, health_check, invoice_controller},
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "STC Server Public API",
        version = "0.1.0",
        description = "Swagger documentation for the public STC server integration endpoints."
    ),
    servers((url = "https://stc-server.onrender.com", description = "Render production server"), (url = "http://localhost:8080", description = "Local development server")),
    paths(
        health_check::health_check,
        enroll::enroll,
        invoice_controller::clearance_prod,
        invoice_controller::clearance_sandbox,
        invoice_controller::reporting_prod,
        invoice_controller::reporting_sandbox
    ),
    components(schemas(
        EnrollDTO,
        EnrollmentCertificateDto,
        SubmitInvoiceDto,
        ClearedInvoiceDto,
        ApiResponse<EnrollmentCertificateDto>,
        ApiResponse<ClearedInvoiceDto>,
        EmptyApiResponse,
        ApiResponse<ErrorData>,
        ErrorData,
        ErrorInfo
    )),
    tags((name = "Public API", description = "Public integration endpoints for enrollment, invoice processing, and health checks."))
)]
pub struct ApiDoc;
