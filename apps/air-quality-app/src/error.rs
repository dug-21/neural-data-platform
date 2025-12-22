use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::fmt;

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

#[derive(Debug)]
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    InternalError(String),
    ServiceUnavailable(String),
    Timeout(String),
}

impl ApiError {
    pub fn code(&self) -> &str {
        match self {
            ApiError::NotFound(_) => "NOT_FOUND",
            ApiError::BadRequest(_) => "BAD_REQUEST",
            ApiError::InternalError(_) => "INTERNAL_ERROR",
            ApiError::ServiceUnavailable(_) => "SERVICE_UNAVAILABLE",
            ApiError::Timeout(_) => "TIMEOUT",
        }
    }

    pub fn message(&self) -> &str {
        match self {
            ApiError::NotFound(msg) => msg,
            ApiError::BadRequest(msg) => msg,
            ApiError::InternalError(msg) => msg,
            ApiError::ServiceUnavailable(msg) => msg,
            ApiError::Timeout(msg) => msg,
        }
    }

    pub fn status_code(&self) -> StatusCode {
        match self {
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::InternalError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Timeout(_) => StatusCode::REQUEST_TIMEOUT,
        }
    }

    pub fn to_detail(&self) -> ErrorDetail {
        ErrorDetail {
            code: self.code().to_string(),
            message: self.message().to_string(),
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

impl std::error::Error for ApiError {}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status_code();
        let body = Json(serde_json::json!({
            "status": "error",
            "error": self.to_detail(),
            "meta": {
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "request_id": uuid::Uuid::new_v4().to_string(),
            }
        }));

        (status, body).into_response()
    }
}

impl From<neural_core::CoreError> for ApiError {
    fn from(err: neural_core::CoreError) -> Self {
        ApiError::InternalError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(ApiError::NotFound("test".to_string()).code(), "NOT_FOUND");
        assert_eq!(
            ApiError::BadRequest("test".to_string()).code(),
            "BAD_REQUEST"
        );
        assert_eq!(
            ApiError::InternalError("test".to_string()).code(),
            "INTERNAL_ERROR"
        );
        assert_eq!(
            ApiError::ServiceUnavailable("test".to_string()).code(),
            "SERVICE_UNAVAILABLE"
        );
        assert_eq!(ApiError::Timeout("test".to_string()).code(), "TIMEOUT");
    }

    #[test]
    fn test_status_codes() {
        assert_eq!(
            ApiError::NotFound("test".to_string()).status_code(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ApiError::BadRequest("test".to_string()).status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            ApiError::InternalError("test".to_string()).status_code(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_error_detail() {
        let error = ApiError::NotFound("Resource not found".to_string());
        let detail = error.to_detail();
        assert_eq!(detail.code, "NOT_FOUND");
        assert_eq!(detail.message, "Resource not found");
    }
}
