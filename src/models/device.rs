use serde::{Deserialize, Serialize};
use sqlx::{types::time::OffsetDateTime, FromRow};
use uuid::Uuid;

#[derive(Debug, FromRow, Deserialize, Serialize)]
pub struct Device {
    pub device_uuid: Uuid,
    pub tin: String,
    pub current_icv: i32,
    pub last_pih: Vec<u8>,
    pub is_active: bool,
    pub onboarded_at: OffsetDateTime,
}
