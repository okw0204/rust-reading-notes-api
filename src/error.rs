//! 内部の失敗を、クライアントへ返す一貫した HTTP エラーへ変換します。

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("resource not found")]
    NotFound,
    #[error("reading state conflict")]
    Conflict,
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("invalid value stored in database: {0}")]
    InvalidStoredValue(String),
}

impl From<crate::domain::InvalidText> for AppError {
    fn from(error: crate::domain::InvalidText) -> Self {
        Self::Validation(error.to_string())
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

// ANCHOR: http_error_mapping
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            Self::Validation(message) => (StatusCode::BAD_REQUEST, "validation_error", message),
            Self::Conflict => (
                StatusCode::CONFLICT,
                "conflict",
                "reading state conflict".to_owned(),
            ),
            Self::NotFound => (
                StatusCode::NOT_FOUND,
                "not_found",
                "resource not found".to_owned(),
            ),
            Self::Database(error) => {
                tracing::error!(%error, "database operation failed");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal server error".to_owned(),
                )
            }
            Self::InvalidStoredValue(error) => {
                tracing::error!(%error, "database contained an invalid value");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "internal server error".to_owned(),
                )
            }
        };

        (
            status,
            Json(ErrorBody {
                error: ErrorDetail { code, message },
            }),
        )
            .into_response()
    }
}
// ANCHOR_END: http_error_mapping
