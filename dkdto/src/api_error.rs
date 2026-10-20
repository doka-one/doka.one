use http::StatusCode;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fmt::{Display, Formatter};

/// Replacement for ErrorSet and ErrorMessage

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ApiError<'a> {
    pub http_error_code: u16,
    #[serde(borrow)]
    pub message: Cow<'a, str>,
    /// Optional structured context attached by the producer (e.g. character
    /// position for filter-parsing errors). Empty for legacy / simple errors.
    #[serde(default)]
    pub context: Vec<String>,
}

impl<'a> Display for ApiError<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "http:{} message:{}", self.http_error_code, self.message)
    }
}

impl<'a> ApiError<'a> {
    pub fn borrowed(code: u16, msg: &'a str) -> Self {
        Self { http_error_code: code, message: Cow::Borrowed(msg), context: Vec::new() }
    }
    pub fn owned<S: Into<String>>(code: u16, msg: S) -> Self {
        Self { http_error_code: code, message: Cow::Owned(msg.into()), context: Vec::new() }
    }
    pub fn owned_with_context<S: Into<String>>(code: u16, msg: S, context: Vec<String>) -> Self {
        Self { http_error_code: code, message: Cow::Owned(msg.into()), context }
    }
    pub fn into_owned(self) -> ApiError<'static> {
        ApiError {
            http_error_code: self.http_error_code,
            message: Cow::Owned(self.message.into_owned()),
            context: self.context,
        }
    }
}

impl From<anyhow::Error> for ApiError<'static> {
    fn from(e: anyhow::Error) -> Self {
        ApiError::owned(StatusCode::INTERNAL_SERVER_ERROR.as_u16(), e.to_string())
    }
}
