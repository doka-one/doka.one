use crate::api_error::ApiError;
use http::StatusCode;
use once_cell::sync::Lazy;

// General / URL
pub static URL_PARSING_ERROR: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Impossible to parse the Url"));
pub static SUCCESS: Lazy<ApiError<'static>> = Lazy::new(|| ApiError::borrowed(StatusCode::OK.as_u16(), "Success"));

// Auth / Tokens
pub static INVALID_CEK: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::UNAUTHORIZED.as_u16(), "Invalid CEK"));
pub static INVALID_TOKEN: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::UNAUTHORIZED.as_u16(), "Invalid token"));
pub static INVALID_SID: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::UNAUTHORIZED.as_u16(), "Invalid Sid"));
pub static INVALID_REQUEST: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Invalid request"));
pub static INVALID_PASSWORD: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::UNAUTHORIZED.as_u16(), "Invalid password"));

// Internals
pub static INTERNAL_TECHNICAL_ERROR: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::INTERNAL_SERVER_ERROR.as_u16(), "Internal technical error"));
pub static STILL_IN_USE: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::LOCKED.as_u16(), "Trying to delete an item still in use"));
pub static INTERNAL_DATABASE_ERROR: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::SERVICE_UNAVAILABLE.as_u16(), "Internal database error"));

// Customer keys
pub static CUSTOMER_KEY_ALREADY_EXISTS: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::CONFLICT.as_u16(), "Customer key already exists"));
pub static CUSTOMER_KEY_DOES_NOT_EXIT: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::NOT_FOUND.as_u16(), "Customer key not found"));

// Sessions
pub static SESSION_TIMED_OUT: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::UNAUTHORIZED.as_u16(), "Session timeout"));
pub static SESSION_NOT_FOUND: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::NOT_FOUND.as_u16(), "Session not found"));
pub static SESSION_CANNOT_BE_CREATED: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Session cannot be created"));
pub static SESSION_CANNOT_BE_RENEWED: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Session cannot be renewed"));
pub static SESSION_INVALID_USER_NAME: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Session invalid user name"));
pub static SESSION_INVALID_CUSTOMER_CODE: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Session invalid customer code"));
pub static SESSION_CANNOT_BE_CLOSED: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Cannot close the session"));
pub static SESSION_LOGIN_DENIED: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::FORBIDDEN.as_u16(), "Login denied"));

// Tags
pub static INCORRECT_DEFAULT_STRING_LENGTH: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Incorrect string length"));
pub static INCORRECT_DEFAULT_LINK_LENGTH: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Incorrect link length"));
pub static INCORRECT_DEFAULT_BOOLEAN_VALUE: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Incorrect default boolean value"));
pub static INCORRECT_DEFAULT_DOUBLE_VALUE: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Incorrect default double value"));
pub static INCORRECT_DEFAULT_INTEGER_VALUE: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Incorrect default integer value"));
pub static INCORRECT_DEFAULT_DATE_VALUE: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Incorrect default date value"));
pub static INCORRECT_DEFAULT_DATETIME_VALUE: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Incorrect default datetime value"));
pub static INCORRECT_TAG_TYPE: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Incorrect tag type"));
pub static INCORRECT_CHAR_TAG_NAME: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Wrong char in tag name"));
pub static INCORRECT_LENGTH_TAG_NAME: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Tag name too long"));

// Items
pub static MISSING_ITEM: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Missing item"));
pub static BAD_TAG_FOR_ITEM: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Bad tag definition"));
pub static MISSING_TAG_FOR_ITEM: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Missing or Incorrect tag definition"));

// Customer
pub static CUSTOMER_NAME_ALREADY_TAKEN: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::CONFLICT.as_u16(), "Customer name already taken"));
pub static CUSTOMER_CODE_ALREADY_TAKEN: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::CONFLICT.as_u16(), "Customer code already taken"));
pub static USER_NAME_ALREADY_TAKEN: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::CONFLICT.as_u16(), "User name already taken"));
pub static CUSTOMER_NOT_REMOVABLE: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::FORBIDDEN.as_u16(), "Customer not removable"));

// Upload
pub static UPLOAD_WRONG_ITEM_INFO: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Item info is not a correct string"));
pub static FILE_INFO_NOT_FOUND: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::NOT_FOUND.as_u16(), "Information about the file is not found"));

pub static HTTP_CLIENT_ERROR: Lazy<ApiError<'static>> =
    Lazy::new(|| ApiError::borrowed(StatusCode::BAD_REQUEST.as_u16(), "Http Client Error"));
