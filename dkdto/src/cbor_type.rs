use crate::error_codes::INTERNAL_TECHNICAL_ERROR;
use crate::{ErrorSet, SimpleMessage, WebTypeBuilder};
use ciborium::into_writer;
use http::header::CONTENT_TYPE;
use http::{HeaderName, StatusCode};
use serde::de;
use serde::Serialize;

pub struct CborType<T> {
    http_code: StatusCode,
    result: Result<T, SimpleMessage>,
}

/// A response with a potential error related to a http code
/// (500, CONTENT_TYPE, message)
pub type CborBytes = (StatusCode, [(HeaderName, &'static str); 1], bytes::Bytes);

///
/// * Allow the conversion of a CborType<T> to a CborBytes
///
impl<T> Into<CborBytes> for CborType<T>
where
    T: Serialize,
{
    fn into(self) -> CborBytes {
        let binary = match self.result {
            Ok(value) => serialize_to_bytes(&value),
            Err(error) => serialize_to_bytes(&SimpleMessage {
                message: error.message.to_string(),
            }),
        };

        (self.http_code, [(CONTENT_TYPE, "application/cbor")], binary)
    }
}

fn serialize_to_bytes<T: Serialize>(value: &T) -> bytes::Bytes {
    let mut cbor_data = Vec::new();
    match into_writer(value, &mut cbor_data) {
        Ok(_) => bytes::Bytes::from(cbor_data),
        Err(_) => bytes::Bytes::from(INTERNAL_TECHNICAL_ERROR.err_message.as_bytes()),
    }
}

impl<T> WebTypeBuilder<T> for CborType<T>
where
    T: de::DeserializeOwned + Serialize,
{
    fn from_simple(code: u16, simple: SimpleMessage) -> Self {
        Self {
            http_code: StatusCode::from_u16(code).unwrap(),
            result: Err(simple),
        }
    }

    /// Convert an item to a CborType
    fn from_item(code: u16, item: T) -> Self {
        Self {
            http_code: StatusCode::from_u16(code).unwrap(),
            result: Ok(item),
        }
    }

    /// Convert an ErrorSet to a CborType
    fn from_errorset(error: &ErrorSet<'static>) -> Self {
        Self {
            http_code: StatusCode::from_u16(error.http_error_code).unwrap(),
            result: Err(SimpleMessage {
                message: error.err_message.to_string(),
            }),
        }
    }
}
