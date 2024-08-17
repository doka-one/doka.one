use http::StatusCode;
use once_cell::sync::Lazy;

use crate::ErrorSet;

pub static SUCCESS: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Success",
    http_error_code: StatusCode::OK.as_u16(),
});

pub static INVALID_CEK: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Invalid CEK",
    http_error_code: StatusCode::UNAUTHORIZED.as_u16(),
});

pub static INVALID_TOKEN: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Invalid token",
    http_error_code: StatusCode::UNAUTHORIZED.as_u16(),
});

pub static INVALID_SID: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Invalid Sid",
    http_error_code: StatusCode::UNAUTHORIZED.as_u16(),
});

pub static INVALID_REQUEST: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Invalid request",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static INVALID_PASSWORD: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Invalid password",
    http_error_code: StatusCode::UNAUTHORIZED.as_u16(),
});
pub static INTERNAL_TECHNICAL_ERROR: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Internal technical error",
    http_error_code: StatusCode::INTERNAL_SERVER_ERROR.as_u16(),
});
pub static STILL_IN_USE: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Trying to delete an item still in use",
    http_error_code: StatusCode::LOCKED.as_u16(),
});
pub static INTERNAL_DATABASE_ERROR: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Internal database error",
    http_error_code: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
});

pub static CUSTOMER_KEY_ALREADY_EXISTS: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Customer key already exists",
    http_error_code: StatusCode::CONFLICT.as_u16(),
});
pub static CUSTOMER_KEY_DOES_NOT_EXIT: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Customer key not found",
    http_error_code: StatusCode::NOT_FOUND.as_u16(),
});

pub static SESSION_TIMED_OUT: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Session timeout",
    http_error_code: StatusCode::UNAUTHORIZED.as_u16(),
});
pub static SESSION_NOT_FOUND: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Session not found",
    http_error_code: StatusCode::NOT_FOUND.as_u16(),
});
pub static SESSION_CANNOT_BE_CREATED: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Session cannot be created",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static SESSION_CANNOT_BE_RENEWED: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Session cannot be renewed",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static SESSION_INVALID_USER_NAME: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Session invalid user name",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static SESSION_INVALID_CUSTOMER_CODE: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Session invalid customer code",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static SESSION_CANNOT_BE_CLOSED: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Cannot close the session",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static SESSION_LOGIN_DENIED: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Login denied",
    http_error_code: StatusCode::FORBIDDEN.as_u16(),
});

/// Tags
pub static INCORRECT_DEFAULT_STRING_LENGTH: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Incorrect string length",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static INCORRECT_DEFAULT_LINK_LENGTH: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Incorrect link length",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static INCORRECT_DEFAULT_BOOLEAN_VALUE: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Incorrect default boolean value",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static INCORRECT_DEFAULT_DOUBLE_VALUE: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Incorrect default double value",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static INCORRECT_DEFAULT_INTEGER_VALUE: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Incorrect default integer value",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static INCORRECT_DEFAULT_DATE_VALUE: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Incorrect default date value",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static INCORRECT_DEFAULT_DATETIME_VALUE: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Incorrect default datetime value",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static INCORRECT_TAG_TYPE: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Incorrect tag type",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static INCORRECT_CHAR_TAG_NAME: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Wrong char in tag name",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static INCORRECT_LENGTH_TAG_NAME: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Tag name too long",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});

/// Items
pub static MISSING_ITEM: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Missing item",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static BAD_TAG_FOR_ITEM: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Bad tag definition",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static MISSING_TAG_FOR_ITEM: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Missing or Incorrect tag definition",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});

/// Customer
pub static CUSTOMER_NAME_ALREADY_TAKEN: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Customer name already taken",
    http_error_code: StatusCode::CONFLICT.as_u16(),
});
pub static CUSTOMER_CODE_ALREADY_TAKEN: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Customer code already taken",
    http_error_code: StatusCode::CONFLICT.as_u16(),
});
pub static USER_NAME_ALREADY_TAKEN: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "User name already taken",
    http_error_code: StatusCode::CONFLICT.as_u16(),
});
pub static CUSTOMER_NOT_REMOVABLE: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Customer not removable",
    http_error_code: StatusCode::FORBIDDEN.as_u16(),
});

/// Upload
pub static UPLOAD_WRONG_ITEM_INFO: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Item info is not a correct string",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
pub static FILE_INFO_NOT_FOUND: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Information about the file is not found",
    http_error_code: StatusCode::NOT_FOUND.as_u16(),
});

pub static HTTP_CLIENT_ERROR: Lazy<ErrorSet> = Lazy::new(|| ErrorSet {
    err_message: "Http Client Error",
    http_error_code: StatusCode::BAD_REQUEST.as_u16(),
});
