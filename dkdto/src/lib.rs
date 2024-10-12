use axum::body::Body;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

use axum::Json;
use chrono::{DateTime, NaiveDate, Utc};
use http::{HeaderMap, StatusCode};
use serde::de;
use serde_derive::{Deserialize, Serialize};

pub mod error_codes;

///
/// Commons DTO
///

#[derive(Debug)]
pub struct ErrorSet<'a> {
    pub http_error_code: u16,
    pub err_message: &'a str,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ErrorMessage {
    pub http_error_code: u16,
    pub message: String,
}

impl From<anyhow::Error> for ErrorMessage {
    fn from(error: anyhow::Error) -> Self {
        ErrorMessage {
            http_error_code: 500,
            message: error.to_string(),
        }
    }
}

impl Display for ErrorMessage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let str = format!("http:{} message:{}", self.http_error_code, &self.message);
        write!(f, "{}", &str)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SimpleMessage {
    pub message: String,
}

pub type DType = (String, u64); // For test only

/// TODO It should be possible to get rid of the Axum dependency
///     we only need it because of Json()
///     so, find a pattern to make all the code below smart enough
pub type WebType<T> = (StatusCode, Result<Json<T>, Json<SimpleMessage>>);

pub trait WebTypeBuilder<T> {
    fn from_simple(code: u16, simple: SimpleMessage) -> Self;
    fn from_item(code: u16, item: T) -> Self;
    fn from_errorset(error: &ErrorSet<'static>) -> Self;
}

impl<T> WebTypeBuilder<T> for WebType<T>
where
    T: de::DeserializeOwned,
{
    fn from_simple(code: u16, simple: SimpleMessage) -> Self {
        let status = StatusCode::from_u16(code).unwrap();
        (status, Err(Json(simple)))
    }

    fn from_item(code: u16, item: T) -> Self {
        (StatusCode::from_u16(code).unwrap(), Ok(Json(item)))
    }

    fn from_errorset(error: &ErrorSet<'static>) -> Self {
        let s = StatusCode::from_u16(error.http_error_code).unwrap();
        (
            s,
            Err(Json(SimpleMessage {
                message: error.err_message.to_string(),
            })),
        )
    }
}

// Need for the ? operator
impl<T> From<ErrorMessage> for WebType<T> {
    fn from(error: ErrorMessage) -> Self {
        let s = StatusCode::from_u16(error.http_error_code).unwrap();
        (
            s,
            Err(Json(SimpleMessage {
                message: error.message,
            })),
        )
    }
}

/// A response with a potential error related to a http code
/// ```
/// use dkdto::{ErrorMessage, WebResponse};
/// let wr: WebResponse<String> = Err( ErrorMessage { http_error_code: 401, message : "Cannot read the document".to_string()} );
/// ```
pub type WebResponse<T> = Result<T, ErrorMessage>;

impl<T> WebTypeBuilder<T> for WebResponse<T> {
    fn from_simple(code: u16, simple: SimpleMessage) -> Self {
        Err(ErrorMessage {
            http_error_code: code,
            message: simple.message.to_owned(),
        })
    }
    fn from_item(_code: u16, item: T) -> Self {
        Ok(item)
    }
    fn from_errorset(error: &ErrorSet<'static>) -> Self {
        Err(ErrorMessage {
            http_error_code: error.http_error_code,
            message: error.err_message.to_owned(),
        })
    }
}

#[derive(Debug)]
pub struct MediaBytes {
    pub media_type: String,
    pub data: bytes::Bytes,
}

pub type MyResult<T> = Result<T, ErrorMessage>;

///
/// Key DTO
///

#[derive(Serialize, Deserialize, Debug)]
pub struct AddKeyRequest {
    pub customer_code: String,
}

// { customer_name, [<key-info>] }
#[derive(Serialize, Deserialize, Debug)]
pub struct CustomerKeyReply {
    pub keys: HashMap<String, EntryReply>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EntryReply {
    pub key_id: i64,
    pub customer_code: String,
    pub ciphered_key: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddKeyReply {
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ClearTextReply {
    pub clear_text: String,
}

///
/// Session DTO
///

#[derive(Serialize, Deserialize, Debug)]
pub struct SessionReply {
    pub sessions: Vec<EntrySession>,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct EntrySession {
    pub id: i64,
    pub customer_code: String,
    pub user_name: String,
    pub customer_id: i64,
    pub user_id: i64,
    pub session_id: String,
    pub start_time_gmt: String,
    pub renew_time_gmt: Option<String>,
    pub termination_time_gmt: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OpenSessionRequest {
    pub customer_code: String,
    pub user_name: String,
    pub customer_id: i64,
    pub user_id: i64,
    pub session_id: String,
}

// { customer_name, [<key-info>] }
#[derive(Serialize, Deserialize, Debug)]
pub struct OpenSessionReply {
    pub session_id: String,
}

///
/// Admin Server
///

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateCustomerRequest {
    pub customer_name: String,
    pub email: String,
    pub admin_password: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct CreateCustomerReply {
    pub customer_id: i64,
    pub customer_code: String, // ex : 2fa6a8d8
    pub admin_user_id: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteCustomerRequest {
    pub customer_code: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteCustomerReply {
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LoginRequest {
    pub login: String,
    pub password: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct LoginReply {
    pub session_id: String,
    pub customer_code: String,
}

///
/// Document Server
///

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum EnumTagValue {
    String(Option<String>),
    Boolean(Option<bool>),
    Integer(Option<i64>),
    Double(Option<f64>),
    SimpleDate(Option<String>),
    DateTime(Option<String>), // "1970-03-23T23:04:10.236Z"
    Link(Option<String>),
}

impl EnumTagValue {
    pub fn to_string(&self) -> String {
        match self {
            EnumTagValue::String(v) => v.clone().unwrap_or("".to_string()),
            EnumTagValue::Boolean(v) => v.clone().unwrap_or(false).to_string(),
            EnumTagValue::Integer(v) => v.clone().unwrap_or(0_i64).to_string(),
            EnumTagValue::Double(v) => v.clone().unwrap_or(0.0_f64).to_string(),
            EnumTagValue::SimpleDate(v) => v.clone().unwrap_or("".to_string()).to_string(),
            EnumTagValue::DateTime(v) => v.clone().unwrap_or("".to_string()).to_string(),
            EnumTagValue::Link(v) => v.clone().unwrap_or("".to_string()).to_string(),
        }
    }

    pub fn from_string(tag_value: &str, tag_type: &str) -> Result<Self, String> {
        match tag_type.to_lowercase().as_str() {
            TAG_TYPE_STRING => Ok(Self::String(Some(tag_value.to_owned()))),
            TAG_TYPE_BOOL => match bool::from_str(tag_value) {
                Ok(b) => Ok(Self::Boolean(Some(b))),
                Err(e) => Err(format!("Bad boolean value: {}", e.to_string())),
            },
            TAG_TYPE_INT => match tag_value.parse::<i64>() {
                Ok(i) => Ok(Self::Integer(Some(i))),
                Err(e) => Err(format!("Bad integer value: {}", e.to_string())),
            },
            TAG_TYPE_DOUBLE => match tag_value.parse::<f64>() {
                Ok(i) => Ok(Self::Double(Some(i))),
                Err(e) => Err(format!("Bad double value: {}", e.to_string())),
            },
            TAG_TYPE_DATE => match NaiveDate::parse_from_str(tag_value, "%Y-%m-%d") {
                Ok(_nd) => Ok(Self::SimpleDate(Some(tag_value.to_owned()))),
                Err(e) => Err(format!("Bad date value: {}", e.to_string())),
            },
            TAG_TYPE_DATETIME => match DateTime::parse_from_rfc3339(tag_value) {
                Ok(_) => Ok(Self::DateTime(Some(tag_value.to_owned()))),
                Err(e) => Err(format!("Bad datetime value: {}", e.to_string())),
            },
            TAG_TYPE_LINK => Ok(Self::Link(Some(tag_value.to_owned()))),
            _ => Err(format!("Bad type: {}", tag_type)),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddItemRequest {
    pub name: String,
    pub file_ref: Option<String>, // file reference to be associated to the item
    pub properties: Option<Vec<AddTagValue>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddItemTagRequest {
    //pub item_id : i64,
    pub properties: Vec<AddTagValue>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddTagValue {
    pub tag_id: Option<i64>, // TODO, not used for now, check if it's usefull or not
    pub tag_name: Option<String>,
    pub value: EnumTagValue,
}

#[derive(Debug)]
pub struct DeleteTagsRequest(pub Vec<String>);

// TODO This code below must be adapted for Axum
//      It simply parse the tag ids when passed as query parameters
// Mise en œuvre de FromFormValue pour traiter la liste de chaînes
// impl<'v> FromFormValue<'v> for DeleteTagsRequest {
//     type Error = &'v RawStr;
//
//     fn from_form_value(form_value: &'v RawStr) -> Result<Self, Self::Error> {
//         let tags: Vec<String> = form_value
//             .split(',')
//             .map(|tag| tag.to_string())
//             .collect();
//
//         Ok(DeleteTagsRequest(tags))
//     }
// }

#[derive(Serialize, Deserialize, Debug)]
pub struct FilterCondition {
    pub tag: String,
    pub op: String,
    pub value: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct QueryFilters(pub String);

// TODO This code below must be adapted for Axum
//      It simply unescape the query filter parameters
// impl<'v> FromFormValue<'v> for QueryFilters {
//     type Error = &'v RawStr;
//
//     fn from_form_value(form_value: &'v RawStr) -> Result<Self, Self::Error> {
//         // TODO : We could do a base64url decoding instead ....
//         let s=  form_value.percent_decode().unwrap().to_string();
//         dbg!(&s);
//         Ok(QueryFilters(s))
//     }
// }

#[derive(Serialize, Deserialize, Debug)]
pub struct AddItemReply {
    pub item_id: i64,
    pub name: String,
    pub created: String,
    pub last_modified: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddItemTagReply {
    pub status: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetItemReply {
    pub items: Vec<ItemElement>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ItemElement {
    pub item_id: i64,
    pub name: String,
    pub file_ref: Option<String>,
    pub created: String,
    pub last_modified: Option<String>,
    pub properties: Option<Vec<TagValueElement>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TagValueElement {
    pub tag_value_id: i64,
    pub item_id: i64,
    pub tag_id: i64,
    pub tag_name: String,
    pub value: EnumTagValue,
}

// Tag

pub const TAG_TYPE_STRING: &str = "text";
pub const TAG_TYPE_BOOL: &str = "bool";
pub const TAG_TYPE_INT: &str = "int";
pub const TAG_TYPE_DOUBLE: &str = "decimal";
pub const TAG_TYPE_DATE: &str = "date";
pub const TAG_TYPE_DATETIME: &str = "datetime";
pub const TAG_TYPE_LINK: &str = "link";

#[derive(Serialize, Deserialize, Debug)]
pub struct AddTagRequest {
    pub name: String,
    pub tag_type: String, // string, bool, integer, double, date, datetime

    pub default_value: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AddTagReply {
    pub tag_id: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetTagReply {
    pub tags: Vec<TagElement>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TagElement {
    pub tag_id: i64,
    pub name: String,
    pub tag_type: String, // string, bool, integer, double, date, datetime
    pub default_value: Option<String>,
}

// Full text

#[derive(Serialize, Deserialize, Debug)]
pub struct FullTextRequest {
    pub file_name: String,
    pub file_ref: String,
    pub raw_text: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct FullTextReply {
    pub part_count: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DeleteFullTextRequest {
    pub file_ref: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UploadReply {
    pub file_ref: String,
    pub size: usize,
    pub block_count: u32,
}

pub type DownloadReply = Result<(HeaderMap, Body), (axum::http::StatusCode, String)>;
// pub type DownloadReply = Custom<Content<Vec<u8>>>;
// pub type DownloadReply = Vec<u8>; // TODO
//
impl WebTypeBuilder<Vec<u8>> for DownloadReply {
    fn from_simple(code: u16, simple: SimpleMessage) -> Self {
        let status = StatusCode::from_u16(code).unwrap();
        Err((status, simple.message))
    }

    fn from_item(code: u16, item: Vec<u8>) -> Self {
        panic!("from_item is no implemented for DownloadReply")
        // Custom(
        //     Status::from_code(code).unwrap(),
        //     Content(ContentType::HTML, item),
        // )
    }

    fn from_errorset(error: &ErrorSet<'static>) -> Self {
        let status = StatusCode::from_u16(error.http_error_code).unwrap();
        Err((status, error.err_message.to_owned()))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetFileInfoReply {
    pub file_ref: String,
    pub media_type: Option<String>,
    pub checksum: Option<String>,
    pub original_file_size: Option<i64>,
    pub encrypted_file_size: Option<i64>,
    pub block_count: Option<i32>,
    pub is_encrypted: bool,
    pub is_fulltext_parsed: Option<bool>,
    pub is_preview_generated: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListOfFileInfoReply {
    pub list_of_files: Vec<GetFileInfoReply>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ListOfUploadInfoReply {
    pub list_of_upload_info: Vec<UploadInfoReply>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct UploadInfoReply {
    pub start_date_time: DateTime<Utc>,
    pub item_info: String, // Is a non unique string to make link with the item element during the initial phase of upload.
    pub file_reference: String,
    pub session_number: String, // Only the first letters of the session id
    pub encrypted_count: i64,   // Number of encrypted parts
    pub uploaded_count: i64,    // Number of block simply loaded
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GetFileInfoShortReply {
    pub file_ref: String,
    pub original_file_size: u64,
    pub block_count: u32,
    pub encrypted_count: i64,
    pub uploaded_count: i64,
    pub fulltext_indexed_count: i64,
    pub preview_generated_count: i64,
}

/// Tika
#[derive(Serialize, Deserialize, Debug)]
pub struct TikaParsing {
    #[serde(rename(deserialize = "Content-Type"))]
    pub content_type: String,
    #[serde(rename(deserialize = "X-TIKA:content"))]
    pub x_tika_content: String,
    #[serde(rename(deserialize = "pdf:PDFVersion"))]
    pub pdf_version: Option<String>,

    // TODO Add all the meta fields as Option is needed
    #[serde(rename(deserialize = "GPS:GPS Longitude"))]
    pub gps_longitude: Option<String>,

    // "ICC:Green Colorant": "(0,292, 0,6922, 0,0419)",
    #[serde(rename(deserialize = "ICC:Green Colorant"))]
    pub icc_green_colorant: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TikaMeta {
    pub language: String,
    #[serde(rename(deserialize = "Content-Type"))]
    pub content_type: String,
}
