use actix_web::{HttpResponse, web};
use base64::{Engine, engine::general_purpose};
use fastxml::schema::CompiledSchema;
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    config::crypto_config::Crypto,
    errors::{ApiError, ErrorCode},
    models::{
        responses::ApiResponse,
        submit_invoice::{InvoiceType, SubmitInvoiceDto},
    },
    services::{
        db::rejected_invoice_service::{RejectedInvoiceRecord, save_rejected_invoice},
        pipeline::clearance_service::process_clearance,
        pipeline::reporting_service::process_reporting,
        xml::extractors::extract_supplier_id,
    },
};

pub async fn clearance_prod(
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<CompiledSchema>,
) -> Result<HttpResponse, ApiError> {
    handle_clearance(db_pool, invoice_dto, crypto, schema_validator, false).await
}

pub async fn clearance_sandbox(
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<CompiledSchema>,
) -> Result<HttpResponse, ApiError> {
    handle_clearance(db_pool, invoice_dto, crypto, schema_validator, true).await
}

async fn handle_clearance(
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<CompiledSchema>,
    sandbox: bool,
) -> Result<HttpResponse, ApiError> {
    let dto = invoice_dto.into_inner();
    let submitted = dto.clone();
    let raw_uuid = dto.uuid.clone();

    let intermediate_dto = match dto.parse(&db_pool).await {
        Ok(intermediate_dto) => intermediate_dto,
        Err(e) => {
            tracing::error!(uuid = %raw_uuid, error = %e, "Failed to parse clearance invoice");
            let api_error = ApiError::from_invoice_parse(&e);
            if !sandbox {
                let supplier_tin = best_effort_supplier_tin(&submitted);
                return Err(persist_rejection_or_internal(
                    db_pool.get_ref(),
                    &submitted,
                    "clear",
                    InvoiceType::Clearance.as_str(),
                    api_error,
                    supplier_tin.as_deref(),
                    None,
                )
                .await);
            }
            return Err(api_error);
        }
    };

    if !intermediate_dto.device.is_active {
        tracing::warn!(
            uuid = %intermediate_dto.uuid,
            device_uuid = %intermediate_dto.device.device_uuid,
            "Invoice rejected because device is inactive"
        );
        let api_error = ApiError::new(ErrorCode::DeviceInactive);
        if !sandbox {
            return Err(persist_rejection_or_internal(
                db_pool.get_ref(),
                &submitted,
                "clear",
                InvoiceType::Clearance.as_str(),
                api_error,
                Some(&intermediate_dto.supplier),
                Some(intermediate_dto.device.device_uuid),
            )
            .await);
        }
        return Err(api_error);
    }

    let uuid = intermediate_dto.uuid;
    let device_uuid = intermediate_dto.device.device_uuid;
    let supplier_tin = intermediate_dto.supplier.clone();

    let cleared_invoice = match process_clearance(
        intermediate_dto,
        &db_pool,
        &crypto,
        sandbox,
        schema_validator,
        InvoiceType::Clearance,
    )
    .await
    {
        Ok(cleared_invoice) => cleared_invoice,
        Err(e) => {
            tracing::error!(uuid = %uuid, device_uuid = %device_uuid, error = %e, "Clearance pipeline failed");
            let api_error = ApiError::from_invoice_pipeline(&e);
            if !sandbox {
                return Err(persist_rejection_or_internal(
                    db_pool.get_ref(),
                    &submitted,
                    "clear",
                    InvoiceType::Clearance.as_str(),
                    api_error,
                    Some(&supplier_tin),
                    Some(device_uuid),
                )
                .await);
            }
            return Err(api_error);
        }
    };

    Ok(HttpResponse::Ok().json(ApiResponse {
        success: true,
        message: "Invoice cleared".into(),
        data: Some(json!({"cleared_invoice": cleared_invoice})),
    }))
}

pub async fn reporting_prod(
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<CompiledSchema>,
) -> Result<HttpResponse, ApiError> {
    handle_reporting(db_pool, invoice_dto, crypto, schema_validator, false).await
}

pub async fn reporting_sandbox(
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<CompiledSchema>,
) -> Result<HttpResponse, ApiError> {
    handle_reporting(db_pool, invoice_dto, crypto, schema_validator, true).await
}

async fn handle_reporting(
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator: web::Data<CompiledSchema>,
    sandbox: bool,
) -> Result<HttpResponse, ApiError> {
    let dto = invoice_dto.into_inner();
    let submitted = dto.clone();
    let raw_uuid = dto.uuid.clone();

    let intermediate_dto = match dto.parse(&db_pool).await {
        Ok(intermediate_dto) => intermediate_dto,
        Err(e) => {
            tracing::error!(uuid = %raw_uuid, error = %e, "Failed to parse reporting invoice");
            let api_error = ApiError::from_invoice_parse(&e);
            if !sandbox {
                let supplier_tin = best_effort_supplier_tin(&submitted);
                return Err(persist_rejection_or_internal(
                    db_pool.get_ref(),
                    &submitted,
                    "report",
                    InvoiceType::Reporting.as_str(),
                    api_error,
                    supplier_tin.as_deref(),
                    None,
                )
                .await);
            }
            return Err(api_error);
        }
    };

    if !intermediate_dto.device.is_active {
        tracing::warn!(
            uuid = %intermediate_dto.uuid,
            device_uuid = %intermediate_dto.device.device_uuid,
            "Invoice rejected because device is inactive"
        );
        let api_error = ApiError::new(ErrorCode::DeviceInactive);
        if !sandbox {
            return Err(persist_rejection_or_internal(
                db_pool.get_ref(),
                &submitted,
                "report",
                InvoiceType::Reporting.as_str(),
                api_error,
                Some(&intermediate_dto.supplier),
                Some(intermediate_dto.device.device_uuid),
            )
            .await);
        }
        return Err(api_error);
    }

    let uuid = intermediate_dto.uuid;
    let device_uuid = intermediate_dto.device.device_uuid;
    let supplier_tin = intermediate_dto.supplier.clone();

    if let Err(e) = process_reporting(
        intermediate_dto,
        &db_pool,
        &crypto,
        sandbox,
        schema_validator,
        InvoiceType::Reporting,
    )
    .await
    {
        tracing::error!(uuid = %uuid, device_uuid = %device_uuid, error = %e, "Reporting pipeline failed");
        let api_error = ApiError::from_invoice_pipeline(&e);
        if !sandbox {
            return Err(persist_rejection_or_internal(
                db_pool.get_ref(),
                &submitted,
                "report",
                InvoiceType::Reporting.as_str(),
                api_error,
                Some(&supplier_tin),
                Some(device_uuid),
            )
            .await);
        }
        return Err(api_error);
    }

    Ok(HttpResponse::Accepted().json(ApiResponse::<()> {
        success: true,
        message: "Invoice reported".into(),
        data: None,
    }))
}

async fn persist_rejection_or_internal(
    db_pool: &PgPool,
    submitted: &SubmitInvoiceDto,
    endpoint: &'static str,
    invoice_type: &'static str,
    api_error: ApiError,
    supplier_tin: Option<&str>,
    device_id: Option<Uuid>,
) -> ApiError {
    let result = save_rejected_invoice(
        db_pool,
        RejectedInvoiceRecord {
            submitted,
            endpoint,
            invoice_type,
            api_error,
            supplier_tin,
            device_id,
        },
    )
    .await;

    match result {
        Ok(()) => api_error,
        Err(error) => {
            tracing::error!(
                submitted_uuid = %submitted.uuid,
                endpoint,
                invoice_type,
                error = %error,
                "Failed to persist rejected production invoice"
            );
            ApiError::internal()
        }
    }
}

fn best_effort_supplier_tin(submitted: &SubmitInvoiceDto) -> Option<String> {
    let invoice_bytes = general_purpose::STANDARD.decode(&submitted.invoice).ok()?;
    extract_supplier_id(&invoice_bytes).ok()
}
