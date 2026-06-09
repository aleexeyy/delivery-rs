use crate::dto::error::ErrorResponse;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    InternalServerError,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            ApiError::BadRequest(message) => {
                let body = Json(ErrorResponse { error: message });

                (StatusCode::BAD_REQUEST, body).into_response()
            }

            ApiError::InternalServerError => {
                let body = Json(ErrorResponse {
                    error: "internal server error".to_string(),
                });

                (StatusCode::INTERNAL_SERVER_ERROR, body).into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    #[tokio::test]
    async fn bad_request_returns_400_with_message() {
        let response = ApiError::BadRequest("invalid payload".to_string()).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "invalid payload");
    }

    #[tokio::test]
    async fn internal_server_error_returns_500_with_generic_message() {
        let response = ApiError::InternalServerError.into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"], "internal server error");
    }

    #[tokio::test]
    async fn bad_request_body_is_json_object_with_error_field() {
        let response = ApiError::BadRequest("oops".to_string()).into_response();
        let body = to_bytes(response.into_body(), 1024).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.is_object(), "Response body must be a JSON object");
        assert!(json.get("error").is_some(), "JSON object must have 'error' field");
    }
}
