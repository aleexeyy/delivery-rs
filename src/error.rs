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
