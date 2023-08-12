use rocket::http::Status;

use crate::ErrorSet;

pub const SUCCESS : ErrorSet = ErrorSet { err_message : "Success", http_error_code : Status::Ok.code};
pub const INVALID_CEK : ErrorSet = ErrorSet {  err_message : "Invalid CEK", http_error_code : Status::Unauthorized.code};
pub const INVALID_TOKEN : ErrorSet = ErrorSet { err_message : "Invalid token", http_error_code : Status::Unauthorized.code};
pub const INVALID_SID : ErrorSet = ErrorSet { err_message : "Invalid Sid", http_error_code : Status::Unauthorized.code};
pub const INVALID_REQUEST : ErrorSet = ErrorSet { err_message : "Invalid request", http_error_code : Status::BadRequest.code};
pub const INVALID_PASSWORD : ErrorSet = ErrorSet { err_message : "Invalid password", http_error_code : Status::Unauthorized.code};
pub const INTERNAL_TECHNICAL_ERROR : ErrorSet = ErrorSet { err_message : "Internal technical error", http_error_code : Status::InternalServerError.code };
pub const STILL_IN_USE : ErrorSet = ErrorSet { err_message : "Trying to delete an item still in use", http_error_code : Status::Locked.code};
pub const INTERNAL_DATABASE_ERROR : ErrorSet = ErrorSet { err_message : "Internal database error", http_error_code : Status::ServiceUnavailable.code};

pub const CUSTOMER_KEY_ALREADY_EXISTS: ErrorSet = ErrorSet { err_message: "Customer key already exists", http_error_code: Status::Conflict.code };
pub const CUSTOMER_KEY_DOES_NOT_EXIT: ErrorSet = ErrorSet { err_message: "Customer key not found", http_error_code: Status::NotFound.code };

pub const SESSION_TIMED_OUT: ErrorSet = ErrorSet { err_message: "Session timeout", http_error_code: Status::Unauthorized.code };
pub const SESSION_NOT_FOUND: ErrorSet = ErrorSet { err_message: "Session not found", http_error_code: Status::NotFound.code };
pub const SESSION_CANNOT_BE_CREATED: ErrorSet = ErrorSet { err_message: "Session cannot be created", http_error_code: Status::BadRequest.code };
pub const SESSION_CANNOT_BE_RENEWED: ErrorSet = ErrorSet { err_message: "Session cannot be renewed", http_error_code: Status::BadRequest.code };
pub const SESSION_INVALID_USER_NAME: ErrorSet = ErrorSet { err_message: "Session invalid user name", http_error_code: Status::BadRequest.code };
pub const SESSION_INVALID_CUSTOMER_CODE: ErrorSet = ErrorSet { err_message: "Session invalid customer code", http_error_code: Status::BadRequest.code };
pub const SESSION_CANNOT_BE_CLOSED: ErrorSet = ErrorSet { err_message: "Cannot close the session", http_error_code: Status::BadRequest.code };
pub const SESSION_LOGIN_DENIED: ErrorSet = ErrorSet { err_message: "Login denied", http_error_code: Status::Forbidden.code };

/// Tags
pub const INCORRECT_DEFAULT_STRING_LENGTH: ErrorSet = ErrorSet { err_message: "Incorrect string length", http_error_code: Status::BadRequest.code };
pub const INCORRECT_DEFAULT_LINK_LENGTH: ErrorSet = ErrorSet { err_message: "Incorrect link length", http_error_code: Status::BadRequest.code };
pub const INCORRECT_DEFAULT_BOOLEAN_VALUE: ErrorSet = ErrorSet { err_message: "Incorrect default boolean value", http_error_code: Status::BadRequest.code };
pub const INCORRECT_DEFAULT_DOUBLE_VALUE: ErrorSet = ErrorSet { err_message: "Incorrect default double value", http_error_code: Status::BadRequest.code };
pub const INCORRECT_DEFAULT_INTEGER_VALUE: ErrorSet = ErrorSet { err_message: "Incorrect default integer value", http_error_code: Status::BadRequest.code };
pub const INCORRECT_DEFAULT_DATE_VALUE: ErrorSet = ErrorSet { err_message: "Incorrect default date value", http_error_code: Status::BadRequest.code };
pub const INCORRECT_DEFAULT_DATETIME_VALUE: ErrorSet = ErrorSet { err_message: "Incorrect default datetime value", http_error_code: Status::BadRequest.code };
pub const INCORRECT_TAG_TYPE: ErrorSet = ErrorSet { err_message: "Incorrect tag type", http_error_code: Status::BadRequest.code };
pub const INCORRECT_CHAR_TAG_NAME: ErrorSet = ErrorSet { err_message: "Wrong char in tag name", http_error_code: Status::BadRequest.code };
pub const INCORRECT_LENGTH_TAG_NAME: ErrorSet = ErrorSet { err_message: "Tag name too long", http_error_code: Status::BadRequest.code };

/// Items
pub const MISSING_ITEM: ErrorSet = ErrorSet { err_message: "Missing item", http_error_code: Status::BadRequest.code };
pub const BAD_TAG_FOR_ITEM: ErrorSet = ErrorSet { err_message: "Bad tag definition", http_error_code: Status::BadRequest.code };
pub const MISSING_TAG_FOR_ITEM: ErrorSet = ErrorSet { err_message: "Missing or Incorrect tag definition", http_error_code: Status::BadRequest.code };

/// Customer
pub const CUSTOMER_NAME_ALREADY_TAKEN: ErrorSet = ErrorSet { err_message: "Customer name already taken", http_error_code: Status::Conflict.code };
pub const USER_NAME_ALREADY_TAKEN: ErrorSet = ErrorSet { err_message: "User name already taken", http_error_code: Status::Conflict.code };
pub const CUSTOMER_NOT_REMOVABLE: ErrorSet = ErrorSet { err_message: "Customer not removable", http_error_code: Status::Forbidden.code };

/// Upload
pub const UPLOAD_WRONG_ITEM_INFO: ErrorSet = ErrorSet { err_message: "Item info is not a correct string", http_error_code: Status::BadRequest.code };
pub const FILE_INFO_NOT_FOUND: ErrorSet = ErrorSet { err_message: "Information about the file is not found", http_error_code: Status::NotFound.code };

pub const HTTP_CLIENT_ERROR: ErrorSet = ErrorSet { err_message: "Http Client Error", http_error_code: Status::BadRequest.code };

