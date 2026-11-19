//! The one error type every route returns, and the JSON it becomes.
//!
//! Clients decide on `code`, never on `message`: the message is for logs and developers and may
//! change at any time. Adding a code is a deliberate change: the web keeps words for every code
//! and its test fails on a code it does not know.

use std::borrow::Cow;

use axum::Json;
use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::request_id;

/// Stable error codes. Serialised in `snake_case`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The body or a parameter is malformed (not JSON, wrong type).
    InvalidRequest,
    /// Well-formed input that breaks a rule. `details.fields` maps a field to its problems.
    ValidationFailed,
    /// A write without `Content-Type: application/json`.
    UnsupportedMediaType,
    PayloadTooLarge,
    /// No session, or the session ended.
    Unauthenticated,
    /// Wrong email or password. The same for both, on purpose.
    InvalidCredentials,
    /// The mailed code is wrong, expired, spent, or was never sent. The same for all four.
    InvalidCode,
    /// The wallet's signed message failed a check (domain, nonce, time, signature).
    InvalidSignature,
    /// Google or GitHub has not verified the account's email.
    EmailNotVerified,
    /// The Google or GitHub account's email is not the account's email.
    ProviderEmailMismatch,
    /// The sign-in at Google or GitHub failed, was cancelled, or its answer could not be used.
    ProviderFailed,
    /// The current password given to change a password is wrong.
    WrongPassword,
    /// Signed in, but missing the permission.
    Forbidden,
    AccountPending,
    AccountSuspended,
    RegistrationClosed,
    NotFound,
    MethodNotAllowed,
    EmailTaken,
    /// The wallet address belongs to another account.
    WalletTaken,
    /// The Google or GitHub account belongs to another account.
    IdentityTaken,
    /// The change would leave no active admin.
    LastAdmin,
    /// Nobody may change their own role, status or permissions.
    CannotChangeSelf,
    /// The sign-in method is not turned on, or cannot be used here.
    LoginMethodUnavailable,
    /// The account already signs in with this method.
    LoginMethodUnchanged,
    /// The change needs a recent sign-in; sign in again and retry.
    ReauthRequired,
    /// Throttled. `details.retry_at` is when to try again (RFC 3339).
    TooManyAttempts,
    RequestTimeout,
    Internal,
    Unavailable,
}

impl ErrorCode {
    pub const ALL: &'static [ErrorCode] = &[
        ErrorCode::InvalidRequest,
        ErrorCode::ValidationFailed,
        ErrorCode::UnsupportedMediaType,
        ErrorCode::PayloadTooLarge,
        ErrorCode::Unauthenticated,
        ErrorCode::InvalidCredentials,
        ErrorCode::InvalidCode,
        ErrorCode::InvalidSignature,
        ErrorCode::EmailNotVerified,
        ErrorCode::ProviderEmailMismatch,
        ErrorCode::ProviderFailed,
        ErrorCode::WrongPassword,
        ErrorCode::Forbidden,
        ErrorCode::AccountPending,
        ErrorCode::AccountSuspended,
        ErrorCode::RegistrationClosed,
        ErrorCode::NotFound,
        ErrorCode::MethodNotAllowed,
        ErrorCode::EmailTaken,
        ErrorCode::WalletTaken,
        ErrorCode::IdentityTaken,
        ErrorCode::LastAdmin,
        ErrorCode::CannotChangeSelf,
        ErrorCode::LoginMethodUnavailable,
        ErrorCode::LoginMethodUnchanged,
        ErrorCode::ReauthRequired,
        ErrorCode::TooManyAttempts,
        ErrorCode::RequestTimeout,
        ErrorCode::Internal,
        ErrorCode::Unavailable,
    ];

    pub fn status(self) -> StatusCode {
        use ErrorCode::*;
        match self {
            InvalidRequest => StatusCode::BAD_REQUEST,
            ValidationFailed | WrongPassword | LoginMethodUnavailable => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            Unauthenticated | InvalidCredentials | InvalidCode | InvalidSignature
            | EmailNotVerified | ProviderFailed => StatusCode::UNAUTHORIZED,
            Forbidden | AccountPending | AccountSuspended | RegistrationClosed | ReauthRequired => {
                StatusCode::FORBIDDEN
            }
            NotFound => StatusCode::NOT_FOUND,
            MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            EmailTaken
            | WalletTaken
            | IdentityTaken
            | LastAdmin
            | CannotChangeSelf
            | LoginMethodUnchanged
            | ProviderEmailMismatch => StatusCode::CONFLICT,
            TooManyAttempts => StatusCode::TOO_MANY_REQUESTS,
            RequestTimeout => StatusCode::REQUEST_TIMEOUT,
            Internal => StatusCode::INTERNAL_SERVER_ERROR,
            Unavailable => StatusCode::SERVICE_UNAVAILABLE,
        }
    }

    /// The code for a status produced outside a handler (router, middleware).
    pub fn from_status(status: StatusCode) -> Self {
        match status {
            StatusCode::BAD_REQUEST => ErrorCode::InvalidRequest,
            StatusCode::UNAUTHORIZED => ErrorCode::Unauthenticated,
            StatusCode::FORBIDDEN => ErrorCode::Forbidden,
            StatusCode::NOT_FOUND => ErrorCode::NotFound,
            StatusCode::METHOD_NOT_ALLOWED => ErrorCode::MethodNotAllowed,
            StatusCode::REQUEST_TIMEOUT => ErrorCode::RequestTimeout,
            StatusCode::PAYLOAD_TOO_LARGE => ErrorCode::PayloadTooLarge,
            StatusCode::UNSUPPORTED_MEDIA_TYPE => ErrorCode::UnsupportedMediaType,
            StatusCode::UNPROCESSABLE_ENTITY => ErrorCode::ValidationFailed,
            StatusCode::TOO_MANY_REQUESTS => ErrorCode::TooManyAttempts,
            StatusCode::SERVICE_UNAVAILABLE => ErrorCode::Unavailable,
            s if s.is_client_error() => ErrorCode::InvalidRequest,
            _ => ErrorCode::Internal,
        }
    }

    fn default_message(self) -> &'static str {
        use ErrorCode::*;
        match self {
            InvalidRequest => "the request is malformed",
            ValidationFailed => "some fields are invalid",
            UnsupportedMediaType => "writes must be sent as application/json",
            PayloadTooLarge => "the request body is too large",
            Unauthenticated => "sign in to continue",
            InvalidCredentials => "wrong email or password",
            InvalidCode => "the code is wrong or no longer works",
            InvalidSignature => "the wallet signature could not be verified",
            EmailNotVerified => "the provider has not verified this email",
            ProviderEmailMismatch => "the provider account has another email",
            ProviderFailed => "the sign-in at the provider did not complete",
            WrongPassword => "the current password is wrong",
            Forbidden => "you do not have permission for this",
            AccountPending => "the account is waiting for approval",
            AccountSuspended => "the account is suspended",
            RegistrationClosed => "registration is closed",
            NotFound => "not found",
            MethodNotAllowed => "method not allowed",
            EmailTaken => "an account with this email already exists",
            WalletTaken => "another account signs in with this wallet",
            IdentityTaken => "another account signs in with this provider account",
            LastAdmin => "this would leave no active admin",
            CannotChangeSelf => "you cannot change your own role, status or permissions",
            LoginMethodUnavailable => "this sign-in method is not available",
            LoginMethodUnchanged => "the account already signs in this way",
            ReauthRequired => "sign in again to make this change",
            TooManyAttempts => "too many attempts",
            RequestTimeout => "the request took too long",
            Internal => "something went wrong on our side",
            Unavailable => "the service is unavailable",
        }
    }
}

/// The JSON body of every error response.
#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ErrorBody {
    pub error: ErrorDetail,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ErrorDetail {
    pub code: ErrorCode,
    /// For developers and logs. Never shown to a user, never matched on.
    pub message: String,
    pub request_id: Option<String>,
    /// Facts the client may need, such as `fields` or `retry_at`.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub details: Option<Value>,
}

#[derive(Debug)]
pub struct AppError {
    pub code: ErrorCode,
    message: Cow<'static, str>,
    details: Option<Value>,
    source: Option<anyhow::Error>,
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code,
            message: Cow::Borrowed(code.default_message()),
            details: None,
            source: None,
        }
    }

    pub fn with_message(mut self, message: impl Into<Cow<'static, str>>) -> Self {
        self.message = message.into();
        self
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// An unexpected failure. The cause is logged, the client sees only `internal`.
    pub fn internal(source: impl Into<anyhow::Error>) -> Self {
        Self {
            source: Some(source.into()),
            ..Self::new(ErrorCode::Internal)
        }
    }

    /// `validation_failed` with one problem on one field.
    pub fn field(field: &str, problem: &str) -> Self {
        Self::new(ErrorCode::ValidationFailed)
            .with_details(serde_json::json!({ "fields": { field: [problem] } }))
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn details(&self) -> Option<&Value> {
        self.details.as_ref()
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {}", self.code, self.message)
    }
}

impl From<ErrorCode> for AppError {
    fn from(code: ErrorCode) -> Self {
        Self::new(code)
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        match e {
            sqlx::Error::RowNotFound => Self::new(ErrorCode::NotFound),
            sqlx::Error::PoolTimedOut => {
                Self::new(ErrorCode::Unavailable).with_message("database busy")
            }
            other => Self::internal(other),
        }
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        Self::internal(e)
    }
}

impl From<JsonRejection> for AppError {
    fn from(r: JsonRejection) -> Self {
        let code = match &r {
            JsonRejection::MissingJsonContentType(_) => ErrorCode::UnsupportedMediaType,
            JsonRejection::JsonDataError(_) => ErrorCode::ValidationFailed,
            _ if r.status() == StatusCode::PAYLOAD_TOO_LARGE => ErrorCode::PayloadTooLarge,
            _ => ErrorCode::InvalidRequest,
        };
        Self::new(code).with_message(r.body_text())
    }
}

impl From<PathRejection> for AppError {
    fn from(r: PathRejection) -> Self {
        Self::new(ErrorCode::NotFound).with_message(r.body_text())
    }
}

impl From<QueryRejection> for AppError {
    fn from(r: QueryRejection) -> Self {
        Self::new(ErrorCode::InvalidRequest).with_message(r.body_text())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.code.status();
        let request_id = request_id::current();
        if status.is_server_error() {
            match &self.source {
                Some(source) => tracing::error!(
                    code = ?self.code, request_id = request_id.as_deref(), error = ?source,
                    "request failed"
                ),
                None => tracing::error!(
                    code = ?self.code, request_id = request_id.as_deref(), message = %self.message,
                    "request failed"
                ),
            }
        }
        let body = ErrorBody {
            error: ErrorDetail {
                code: self.code,
                message: self.message.into_owned(),
                request_id,
                details: self.details,
            },
        };
        let mut res = (status, Json(body)).into_response();
        res.extensions_mut().insert(IsErrorBody);
        res
    }
}

/// Marks a response whose body is already an [`ErrorBody`], so middleware leaves it alone.
#[derive(Debug, Clone, Copy)]
pub struct IsErrorBody;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_serialise_in_snake_case() {
        let json = serde_json::to_string(&ErrorCode::TooManyAttempts).unwrap();
        assert_eq!(json, "\"too_many_attempts\"");
    }

    #[test]
    fn all_lists_every_code_once() {
        let unique: std::collections::HashSet<_> = ErrorCode::ALL.iter().collect();
        assert_eq!(unique.len(), ErrorCode::ALL.len());
        assert_eq!(
            ErrorCode::ALL.len(),
            30,
            "a new code must be added to ALL and to the web"
        );
    }

    /// The API page's error table is the reference for clients, so it must name every code.
    #[test]
    fn the_api_docs_list_every_code() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/system/api.md");
        let docs = std::fs::read_to_string(path).unwrap();
        for code in ErrorCode::ALL {
            let name = serde_json::to_value(code).unwrap();
            let name = name.as_str().unwrap();
            assert!(
                docs.contains(&format!("| `{name}` |")),
                "docs/system/api.md has no row for `{name}`"
            );
        }
    }

    #[test]
    fn internal_hides_its_cause() {
        let res = AppError::internal(anyhow::anyhow!("password=hunter2")).into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn status_and_code_round_trip_for_middleware_statuses() {
        for code in [
            ErrorCode::NotFound,
            ErrorCode::MethodNotAllowed,
            ErrorCode::RequestTimeout,
            ErrorCode::PayloadTooLarge,
            ErrorCode::UnsupportedMediaType,
        ] {
            assert_eq!(ErrorCode::from_status(code.status()), code);
        }
    }
}
