use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub message: String,
    pub data: Option<T>,
}

#[derive(Debug, Serialize)]
pub struct ErrorData {
    pub error: ErrorInfo,
}

#[derive(Debug, Serialize)]
pub struct ErrorInfo {
    pub code: &'static str,
}
