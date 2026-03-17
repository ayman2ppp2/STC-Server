use actix_web::{HttpRequest, HttpResponse, web};
use serde_json::json;
use sqlx::PgPool;

use crate::{
    config::{crypto_config::Crypto, xsd_config::SchemaValidator},
    models::{responses::ApiResponse, submit_invoice_dto::SubmitInvoiceDto},
    services::submit_invoice_service::process_invoice,
};

pub async fn clearance(
    req: HttpRequest,
    db_pool: web::Data<PgPool>,
    invoice_dto: web::Json<SubmitInvoiceDto>,
    crypto: web::Data<Crypto>,
    schema_validator : web::Data<SchemaValidator>
) -> Result<HttpResponse, actix_web::Error> {
    let sandbox = req.headers().contains_key("X-Sandbox-Mode");
    let intermediate_dto = invoice_dto
        .into_inner()
        .parse()
        .map_err(actix_web::error::ErrorBadRequest)?;
    let api_response = match process_invoice(intermediate_dto, &db_pool, &crypto, sandbox, &schema_validator).await {
        Ok(cleared_invoice) => ApiResponse {
            success: true,
            message: "Invoice cleared".to_owned(),
            data: Some(json!({"cleared invoice": cleared_invoice})),
        },
        Err(e) => ApiResponse {
            success: false,
            message: "bad request ".to_owned(),
            data: Some(json!({"details": e.to_string()})),
        },
    };
    Ok(HttpResponse::Ok().json(api_response))
    // Ok(HttpResponse::Ok().json(SubmitInvoiceResponse {
    //     clearence_status: ClearenceStatus::Cleared,
    //     cleared_invoice,
    //     validation_results: ValidationResults {
    //         info_messages: vec![ValidationMessage {
    //             message_type: MessageType::Info,
    //             code: "200".to_string(),
    //             category: "XSD validation".to_string(),
    //             message: "Complied with UBL 2.1 standards in line with STC specifications"
    //                 .to_string(),
    //             status: ValidationStatus::Pass,
    //         }],
    //         warning_messages: vec![],
    //         error_messages: vec![],
    //         validation_status: ValidationStatus::Pass,
    //     },
    // }))
}
