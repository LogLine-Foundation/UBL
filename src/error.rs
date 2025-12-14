use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UblError {
    #[error("Program Not Found: {0}")]
    ProgramNotFound(String), // UBL-0x10
    #[error("Chip Not Found: {0}")]
    ChipNotFound(String), // UBL-0x11
    #[error("Validation Error: {0}")]
    Validation(String), // UBL-0x20
    #[error("Logic Denied: {0}")]
    LogicDenied(String), // UBL-0x01
    #[error("Unauthorized")]
    Unauthorized, // UBL-0x40
    #[error("Ledger IO Error: {0}")]
    LedgerIo(String), // UBL-0x30
    #[error("Serialization Error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("State Corruption: {0}")]
    State(String),
    #[error("External Service Error: {0}")]
    External(String),
}

impl IntoResponse for UblError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            UblError::ProgramNotFound(_) => (StatusCode::NOT_FOUND, "UBL-0x10"),
            UblError::ChipNotFound(_) => (StatusCode::NOT_FOUND, "UBL-0x11"),
            UblError::Validation(_) => (StatusCode::BAD_REQUEST, "UBL-0x20"),
            UblError::LogicDenied(_) => (StatusCode::UNPROCESSABLE_ENTITY, "UBL-0x01"),
            UblError::Unauthorized => (StatusCode::UNAUTHORIZED, "UBL-0x40"),
            UblError::LedgerIo(_) => (StatusCode::INTERNAL_SERVER_ERROR, "UBL-0x30"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "UBL-0x99"),
        };
        let body = Json(json!({ "error": self.to_string(), "code": code }));
        (status, body).into_response()
    }
}
