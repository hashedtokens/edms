#![allow(dead_code)]
use thiserror::Error;
use std::io;
use axum :: {
    http::StatusCode,
    response::{IntoResponse,Response},
    Json,
};

use serde::Serialize;

#[derive(Error,Debug)]
pub enum EdmsError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Folder structure broken: {0}")]
    StructureBroken(String),

    #[error("Zip error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub enum ApiError {
    BadRequest(String), //400
    NotFound(String), //404
    Internal(String), // 500
}

#[derive(Serialize)]
pub struct ErrorBody {
    pub error: &'static str,
    pub message: String,
}
// Implementation 

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, kind, message) = match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            ApiError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_server_error",
                msg,
            ),
        };

        tracing::error!(
            status = %status,
            kind = kind,
            message = %message,
            "Request failed"
        );

        (status, Json(ErrorBody { error: kind, message })).into_response()
    }
}



impl From<Box<dyn std::error::Error + Send + Sync>> for ApiError {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for ApiError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        ApiError::Internal(e.to_string())
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::Internal(e.to_string())
    }
}
