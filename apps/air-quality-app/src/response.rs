use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub timestamp: String,
    pub request_id: String,
}

impl Meta {
    pub fn new() -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: uuid::Uuid::new_v4().to_string(),
        }
    }
}

impl Default for Meta {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub status: String,
    pub data: T,
    pub meta: Meta,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            status: "success".to_string(),
            data,
            meta: Meta::new(),
        }
    }
}

impl<T: Serialize> IntoResponse for ApiResponse<T> {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_meta_creation() {
        let meta = Meta::new();
        assert!(!meta.timestamp.is_empty());
        assert!(!meta.request_id.is_empty());
    }

    #[test]
    fn test_api_response_success() {
        let response = ApiResponse::success("test data");
        assert_eq!(response.status, "success");
        assert_eq!(response.data, "test data");
    }
}
