use serde::Serialize;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct EmptyApiResponse {
    pub success: bool,
    pub message: String,
    #[schema(nullable = true, example = json!(null))]
    pub data: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorData {
    pub error: ErrorInfo,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorInfo {
    #[schema(value_type = String, example = "invalid_invoice_data")]
    pub code: &'static str,
}
