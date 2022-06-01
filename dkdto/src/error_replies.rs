use crate::error_codes::{INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR, INVALID_CEK, INVALID_TOKEN};
use crate::{ErrorSet};

///
/// Define the standard error code status for any type of reply
///
pub trait ErrorReply {
    type T;
    fn from_error(error_set: ErrorSet) -> Self::T;

    fn invalid_token_error_reply() -> Self::T {
        Self::from_error(INVALID_TOKEN)
    }

    fn invalid_common_edible_key() -> Self::T {
        Self::from_error(INVALID_CEK)
    }

    fn internal_database_error_reply() -> Self::T {
        Self::from_error(INTERNAL_DATABASE_ERROR)
    }

    fn internal_technical_error_reply() -> Self::T {
        Self::from_error(INTERNAL_TECHNICAL_ERROR)
    }
}
