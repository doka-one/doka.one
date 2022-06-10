use crate::ErrorSet;

pub const SUCCESS : ErrorSet = ErrorSet { error_code : 0, err_message : "Success", http_error_code : 200};
pub const INVALID_CEK : ErrorSet = ErrorSet { error_code : 10, err_message : "Invalid CEK", http_error_code : 200};
pub const INVALID_TOKEN : ErrorSet = ErrorSet { error_code : 20, err_message : "Invalid token", http_error_code : 200};
pub const INVALID_SID : ErrorSet = ErrorSet { error_code : 25, err_message : "Invalid Sid", http_error_code : 200};
pub const INVALID_REQUEST : ErrorSet = ErrorSet { error_code : 30, err_message : "Invalid request", http_error_code : 200};
pub const INVALID_PASSWORD : ErrorSet = ErrorSet { error_code : 40, err_message : "Invalid password", http_error_code : 200};
pub const INTERNAL_TECHNICAL_ERROR : ErrorSet = ErrorSet { error_code : 80, err_message : "Internal technical error", http_error_code : 200};
pub const STILL_IN_USE : ErrorSet = ErrorSet { error_code : 85, err_message : "Trying to delete an item still in use", http_error_code : 200};
pub const INTERNAL_DATABASE_ERROR : ErrorSet = ErrorSet { error_code : 90, err_message : "Internal database error", http_error_code : 200};

pub const CUSTOMER_KEY_ALREADY_EXISTS : ErrorSet = ErrorSet { error_code : 100, err_message : "Customer key already exists", http_error_code : 200};
pub const CUSTOMER_KEY_DOES_NOT_EXIT : ErrorSet = ErrorSet { error_code : 110, err_message : "Customer key not found", http_error_code : 200};

pub const SESSION_TIMED_OUT : ErrorSet = ErrorSet { error_code : 200, err_message : "Session timeout", http_error_code : 200};
pub const SESSION_NOT_FOUND : ErrorSet = ErrorSet { error_code : 210, err_message : "Session not found", http_error_code : 200};
pub const SESSION_CANNOT_BE_CREATED : ErrorSet = ErrorSet { error_code : 220, err_message : "Session cannot be created", http_error_code : 200};
pub const SESSION_CANNOT_BE_RENEWED : ErrorSet = ErrorSet { error_code : 220, err_message : "Session cannot be renewed", http_error_code : 200};
pub const SESSION_INVALID_USER_NAME : ErrorSet = ErrorSet { error_code : 230, err_message : "Session invalid user name", http_error_code : 200};
pub const SESSION_INVALID_CUSTOMER_CODE : ErrorSet = ErrorSet { error_code : 240, err_message : "Session invalid customer code", http_error_code : 200};
pub const SESSION_CANNOT_BE_CLOSED : ErrorSet = ErrorSet { error_code : 250, err_message : "Cannot close the session", http_error_code : 200};
pub const SESSION_LOGIN_DENIED: ErrorSet = ErrorSet { error_code : 300, err_message : "Login denied", http_error_code : 200};

/// Tags
pub const INCORRECT_STRING_LENGTH : ErrorSet = ErrorSet { error_code : 400, err_message : "Incorrect string length", http_error_code : 200};
pub const INCORRECT_DEFAULT_STRING_LENGTH : ErrorSet = ErrorSet { error_code : 400, err_message : "Incorrect string length", http_error_code : 200};
pub const INCORRECT_DEFAULT_BOOLEAN_VALUE : ErrorSet = ErrorSet { error_code : 410, err_message : "Incorrect default boolean value", http_error_code : 200};
pub const INCORRECT_DEFAULT_DOUBLE_VALUE : ErrorSet = ErrorSet { error_code : 420, err_message : "Incorrect default double value", http_error_code : 200};
pub const INCORRECT_DEFAULT_INTEGER_VALUE : ErrorSet = ErrorSet { error_code : 420, err_message : "Incorrect default integer value", http_error_code : 200};
pub const INCORRECT_DEFAULT_DATE_VALUE : ErrorSet = ErrorSet { error_code : 420, err_message : "Incorrect default date value", http_error_code : 200};
pub const INCORRECT_DEFAULT_DATETIME_VALUE : ErrorSet = ErrorSet { error_code : 420, err_message : "Incorrect default datetime value", http_error_code : 200};
pub const INCORRECT_TAG_TYPE : ErrorSet = ErrorSet { error_code : 430, err_message : "Incorrect tag type", http_error_code : 200};
pub const INCORRECT_CHAR_TAG_NAME : ErrorSet = ErrorSet { error_code : 440, err_message : "Wrong char in tag name", http_error_code : 200};
pub const INCORRECT_LENGTH_TAG_NAME : ErrorSet = ErrorSet { error_code : 440, err_message : "Tag name too long", http_error_code : 200};

/// Items
pub const BAD_TAG_FOR_ITEM : ErrorSet = ErrorSet { error_code : 510, err_message : "Bad tag definition", http_error_code : 200};
pub const MISSING_TAG_FOR_ITEM : ErrorSet = ErrorSet { error_code : 520, err_message : "Missing or Incorrect tag definition", http_error_code : 200};

/// Customer
pub const CUSTOMER_NAME_ALREADY_TAKEN: ErrorSet = ErrorSet { error_code : 600, err_message : "Customer name already taken", http_error_code : 200};
pub const USER_NAME_ALREADY_TAKEN: ErrorSet = ErrorSet { error_code : 610, err_message : "User name already taken", http_error_code : 200};
pub const CUSTOMER_NOT_REMOVABLE: ErrorSet = ErrorSet { error_code : 610, err_message : "Customer not removable", http_error_code : 200};


pub const HTTP_CLIENT_ERROR : ErrorSet = ErrorSet { error_code : 999, err_message : "Http Client Error", http_error_code : 200};