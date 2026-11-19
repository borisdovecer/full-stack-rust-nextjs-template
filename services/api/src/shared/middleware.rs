//! Middleware that every request passes through.

use axum::extract::Request;
use axum::http::{HeaderValue, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use app_core::error::IsErrorBody;
use app_core::{AppError, ErrorCode, request_id};

/// Accepts or makes the request id, makes it current for the request, and returns it.
pub async fn request_id(mut req: Request, next: Next) -> Response {
    let incoming = req
        .headers()
        .get(request_id::HEADER)
        .and_then(|v| v.to_str().ok());
    let id =
        request_id::accept(incoming).unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
    if let Ok(value) = HeaderValue::from_str(&id) {
        req.headers_mut().insert(request_id::HEADER, value.clone());
        let mut res = request_id::scope(id, next.run(req)).await;
        res.headers_mut().insert(request_id::HEADER, value);
        res
    } else {
        next.run(req).await
    }
}

/// Turns an error response made outside a handler (unknown route, wrong method, timeout,
/// body limit, panic) into the standard JSON error, so a client only ever parses one shape.
/// A handler's own JSON answer with an error status (health's 503) is left as it is.
pub async fn normalize_errors(req: Request, next: Next) -> Response {
    let res = next.run(req).await;
    let status = res.status();
    let is_json = res
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if (status.is_client_error() || status.is_server_error())
        && res.extensions().get::<IsErrorBody>().is_none()
        && !is_json
    {
        let code = ErrorCode::from_status(status);
        let mut out = AppError::new(code).into_response();
        // Keep a status the code table does not have (for example 418) as it was.
        *out.status_mut() = status;
        return out;
    }
    res
}

/// Refuses a write that is not JSON. A cross-site form can only send form encodings or plain
/// text, so together with `SameSite=Lax` cookies this blocks cross-site request forgery.
pub async fn require_json_writes(req: Request, next: Next) -> Response {
    use axum::http::Method;
    let safe = matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS);
    let json = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    if safe || json {
        next.run(req).await
    } else {
        AppError::new(ErrorCode::UnsupportedMediaType).into_response()
    }
}
