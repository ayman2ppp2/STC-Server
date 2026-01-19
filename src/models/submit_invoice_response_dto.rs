use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitInvoiceResponse {
    pub clearence_status: ClearenceStatus,
    pub cleared_invoice: String,
    pub validation_results: ValidationResults,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClearenceStatus {
    Cleared,
    NotCleared,
    Rejected,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResults {
    pub info_messages: Vec<ValidationMessage>,
    pub warning_messages: Vec<ValidationMessage>,
    pub error_messages: Vec<ValidationMessage>,
    pub validation_status: ValidationStatus,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationMessage {
    #[serde(rename = "type")]
    pub message_type: MessageType,

    pub code: String,
    pub category: String,
    pub message: String,
    pub status: ValidationStatus,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidationStatus {
    Pass,
    Fail,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MessageType {
    Info,
    Warnining,
    Error,
}
