use std::collections::HashMap;
use std::fmt::format;
use std::str::{FromStr, ParseBoolError};
use chrono::{Date, DateTime, Local, LocalResult, NaiveDate, ParseResult, TimeZone, Utc};

use rocket_okapi::JsonSchema;
use serde_derive::{Deserialize, Serialize};
use crate::error_replies::ErrorReply;

pub mod error_codes;
pub mod error_replies;

///
/// Commons DTO
///

#[derive(Debug)]
pub struct ErrorSet<'a> {
    pub error_code : u32,
    pub err_message : &'a str,
    pub http_error_code : u32,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct JsonErrorSet {
    pub error_code : u32,
    pub err_message : String,
}

impl JsonErrorSet {
    pub fn from(error : ErrorSet<'_>) -> Self {
        let err_message = String::from(error.err_message);
        JsonErrorSet { error_code : error.error_code, err_message }
    }
}

///
/// Key DTO
///


#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct AddKeyRequest {
    pub customer_code: String,
}

// { customer_name, [<key-info>] }
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct CustomerKeyReply {
    pub keys: HashMap<String, EntryReply>,
    pub status: JsonErrorSet,
}

impl ErrorReply for CustomerKeyReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        CustomerKeyReply {
            keys: Default::default(),
            status: JsonErrorSet::from(error_set),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct EntryReply {
    pub key_id: i64,
    pub customer_code: String,
    pub ciphered_key: String,
    pub active: bool,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct AddKeyReply {
    pub success: bool,
    pub status: JsonErrorSet,
}

impl ErrorReply for AddKeyReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        AddKeyReply {
            success: false,
            status: JsonErrorSet::from(error_set),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct ClearTextReply {
    pub clear_text: String,
}

///
/// Session DTO
///

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct SessionReply {
    pub sessions : Vec<EntrySession>,
    pub status: JsonErrorSet,
}

impl ErrorReply for SessionReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        SessionReply {
            sessions: vec![],
            status: JsonErrorSet::from(error_set),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, JsonSchema)]
pub struct EntrySession {
    pub id: i64,
    pub customer_code : String,
    pub user_name : String,
    pub customer_id : i64,
    pub user_id : i64,
    pub session_id : String,
    pub start_time_gmt : String,
    pub renew_time_gmt : Option<String>,
    pub termination_time_gmt : Option<String>,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct OpenSessionRequest {
    pub customer_code : String,
    pub user_name : String,
    pub customer_id : i64,
    pub user_id : i64,
    pub session_id : String,
}

// { customer_name, [<key-info>] }
#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct OpenSessionReply {
    pub session_id : String,
    pub status : JsonErrorSet,
}

impl ErrorReply for OpenSessionReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        OpenSessionReply {
            session_id: "".to_string(),
            status: JsonErrorSet::from(error_set),
        }
    }
}

///
/// Admin Server
///

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct CreateCustomerRequest {
    pub customer_name : String,
    pub email : String,
    pub admin_password : String,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct CreateCustomerReply {
    pub customer_id: i64,
    pub customer_code : String, // ex : 2fa6a8d8
    pub admin_user_id : i64,
    pub status : JsonErrorSet,
}

impl ErrorReply for CreateCustomerReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        CreateCustomerReply {
            customer_id: 0,
            customer_code: "".to_string(),
            admin_user_id: 0,
            status: JsonErrorSet::from(error_set),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct DeleteCustomerRequest {
    pub customer_code : String,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct DeleteCustomerReply {
    pub status : JsonErrorSet,
}

impl ErrorReply for DeleteCustomerReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        DeleteCustomerReply {
            status: JsonErrorSet::from(error_set),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct LoginRequest {
    pub login : String,
    pub password : String,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct LoginReply {
    pub session_id: String,
    pub customer_code : String,
    pub status : JsonErrorSet,
}

impl ErrorReply for LoginReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        LoginReply {
            session_id: "".to_string(),
            customer_code: "".to_string(),
            status: JsonErrorSet::from(error_set),
        }
    }
}

///
/// Document Server
///

#[derive(Clone, Serialize, Deserialize, Debug, JsonSchema)]
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
            EnumTagValue::String(v) => {
                v.clone().unwrap_or("".to_string())
            }
            EnumTagValue::Boolean(v) => {
                v.clone().unwrap_or(false).to_string()
            }
            EnumTagValue::Integer(v) => {
                v.clone().unwrap_or(0_i64).to_string()
            }
            EnumTagValue::Double(v) => {
                v.clone().unwrap_or(0.0_f64).to_string()
            }
            EnumTagValue::SimpleDate(v) => {
                v.clone().unwrap_or("".to_string()).to_string()
            }
            EnumTagValue::DateTime(v) => {
                v.clone().unwrap_or("".to_string()).to_string()
            }
            EnumTagValue::Link(v) => {
                v.clone().unwrap_or("".to_string()).to_string()
            }
        }
    }


    pub fn from_string(tag_value: &str, tag_type: &str) -> Result<Self, String> {
        match tag_type.to_lowercase().as_str() {
            TAG_TYPE_STRING => {
                Ok(Self::String(Some(tag_value.to_owned())))
            }
            TAG_TYPE_BOOL => {
                match bool::from_str(tag_value) {
                    Ok(b) => {
                        Ok(Self::Boolean(Some(b)))
                    }
                    Err(e) => {
                        Err(format!("Bad boolean value: {}", e.to_string()))
                    }
                }
            }
            TAG_TYPE_INT => {
                match tag_value.parse::<i64>() {
                    Ok(i) => {
                        Ok(Self::Integer(Some(i)))
                    }
                    Err(e) => {
                        Err(format!("Bad integer value: {}", e.to_string()))
                    }
                }
            }
            TAG_TYPE_DOUBLE => {
                match tag_value.parse::<f64>() {
                    Ok(i) => {
                        Ok(Self::Double(Some(i)))
                    }
                    Err(e) => {
                        Err(format!("Bad double value: {}", e.to_string()))
                    }
                }
            }
            TAG_TYPE_DATE => {
                match NaiveDate::parse_from_str(tag_value, "%Y-%m-%d") {
                    Ok(nd) => {
                        Ok(Self::SimpleDate(Some(tag_value.to_owned())))
                    }
                    Err(e) => {
                        Err(format!("Bad date value: {}", e.to_string()))
                    }
                }
            }
            TAG_TYPE_DATETIME => {
                match DateTime::parse_from_rfc3339(tag_value) {
                    Ok(_) => {
                        Ok(Self::DateTime(Some(tag_value.to_owned())))
                    }
                    Err(e) => {
                        Err(format!("Bad datetime value: {}", e.to_string()))
                    }
                }

            }
            TAG_TYPE_LINK => {
                Ok(Self::Link(Some(tag_value.to_owned())))
            }
            _ => {
                Err(format!("Bad type: {}", tag_type))
            }
        }
    }

}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct AddItemRequest {
    pub name : String,
    pub file_ref : Option<String>, // file reference to be associated to the item
    pub properties: Option<Vec<AddTagValue>>,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct AddItemTagRequest {
    pub item_id : i64,
    pub properties: Vec<AddTagValue>,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct AddTagValue {
    pub tag_id : Option<i64>,
    pub tag_name: Option<String>,
    pub value : EnumTagValue,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct AddItemReply {
    pub item_id : i64,
    pub name : String,
    pub created : String,
    pub last_modified : Option<String>,
    pub status : JsonErrorSet,
}

impl ErrorReply for AddItemReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        Self {
            item_id: 0,
            name: "".to_string(),
            created: "".to_string(),
            status: JsonErrorSet::from(error_set),
            last_modified: None
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct AddItemTagReply {
    pub status : JsonErrorSet,
}

impl ErrorReply for AddItemTagReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        Self {
            status: JsonErrorSet::from(error_set),
        }
    }
}

impl AddItemReply {
    pub fn from_error_with_text(error_set : ErrorSet, text : &str) -> Self {
        let message = format!("{} - {}", &error_set.err_message, text);
        let extended_error_set = ErrorSet {
            error_code: error_set.error_code,
            err_message: message.as_str(),
            http_error_code: error_set.http_error_code,
        };

        Self {
            item_id: 0,
            name: "".to_string(),
            created: "".to_string(),
            last_modified: None,
            status: JsonErrorSet::from(extended_error_set),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct GetItemReply {
    pub items :  Vec<ItemElement>,
    pub status : JsonErrorSet,
}

impl ErrorReply for GetItemReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        GetItemReply {
            items: vec![],
            status: JsonErrorSet::from(error_set),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct ItemElement {
    pub item_id : i64,
    pub name : String,
    pub file_ref: Option<String>,
    pub created : String,
    pub last_modified : Option<String>,
    pub properties: Option<Vec<TagValueElement>>,
}


#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct TagValueElement {
    pub tag_value_id : i64,
    pub item_id :  i64,
    pub tag_id : i64,
    pub tag_name: String,
    pub value : EnumTagValue,
}

// Tag

pub const TAG_TYPE_STRING : &str = "text";
pub const TAG_TYPE_BOOL : &str = "bool";
pub const TAG_TYPE_INT : &str = "int";
pub const TAG_TYPE_DOUBLE : &str = "decimal";
pub const TAG_TYPE_DATE : &str = "date";
pub const TAG_TYPE_DATETIME : &str = "datetime";
pub const TAG_TYPE_LINK : &str = "link";

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct AddTagRequest {
    pub name : String,
    pub tag_type : String, // string, bool, integer, double, date, datetime

    pub default_value : Option<String>,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct AddTagReply {
    pub tag_id : i64,
    pub status : JsonErrorSet,
}

impl ErrorReply for AddTagReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        AddTagReply {
            tag_id: 0,
            status: JsonErrorSet::from(error_set),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct GetTagReply {
    pub tags :  Vec<TagElement>,
    pub status : JsonErrorSet,
}

impl ErrorReply for GetTagReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        GetTagReply {
            tags: vec![],
            status: JsonErrorSet::from(error_set),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct TagElement {
    pub tag_id : i64,
    pub name : String,
    pub tag_type : String, // string, bool, integer, double, date, datetime

    pub default_value : Option<String>,
}

// Full text

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct FullTextRequest {
    pub file_name : String,
    pub file_ref : String,
    pub raw_text : String,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct FullTextReply {
    //pub item_id : i64,
    pub part_count : u32,
    pub status : JsonErrorSet,
}

impl ErrorReply for FullTextReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        FullTextReply {
            // item_id: 0,
            part_count: 0,
            status: JsonErrorSet::from(error_set),
        }
    }
}

// File Server
// #[derive(Serialize, Deserialize, Debug, JsonSchema)]
// pub struct UploadRequest {
//     file_name : String,
// }

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct UploadReply {
    pub file_ref : String,
    pub size : usize,
    pub block_count : u32,
    pub status : JsonErrorSet,
}

impl ErrorReply for UploadReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        UploadReply {
            file_ref: "".to_string(),
            size : 0,
            block_count: 0,
            status: JsonErrorSet::from(error_set),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct GetFileInfoReply {
    pub file_ref : String,
    pub block_count : u32,
    pub block_status : Vec<Option<BlockStatus>>,
    pub status : JsonErrorSet,
}

impl ErrorReply for GetFileInfoReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        GetFileInfoReply {
            file_ref : "".to_string(),
            block_count: 0u32,
            block_status: vec![],
            status: JsonErrorSet::from(error_set),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct BlockStatus {
    pub original_size : usize,
    pub block_number : u32,
    pub is_encrypted : bool,
    pub is_fulltext_indexed : bool,
    pub is_preview_generated : bool,
}

#[derive(Serialize, Deserialize, Debug, JsonSchema)]
pub struct GetFileInfoShortReply {
    pub file_ref : String,
    pub original_file_size: u64,
    pub block_count : u32,
    pub encrypted_count : i64,
    pub fulltext_indexed_count : i64,
    pub preview_generated_count : i64,
    pub status : JsonErrorSet,
}

impl ErrorReply for GetFileInfoShortReply {
    type T = Self;
    fn from_error(error_set: ErrorSet) -> Self::T {
        GetFileInfoShortReply {
            file_ref: "".to_string(),
            original_file_size: 0,
            block_count: 0,
            encrypted_count: 0,
            fulltext_indexed_count: 0,
            preview_generated_count: 0,
            status: JsonErrorSet::from(error_set),
        }
    }
}

/// Tika
#[derive(Serialize, Deserialize, Debug)]
pub struct TikaParsing {
    #[serde(rename(deserialize  = "Content-Type"))]
    pub content_type: String,
    #[serde(rename(deserialize  = "X-TIKA:content"))]
    pub x_tika_content: String,
    #[serde(rename(deserialize  = "pdf:PDFVersion"))]
    pub pdf_version: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TikaMeta {
    pub language: String,
    #[serde(rename(deserialize  = "Content-Type"))]
    pub content_type: String,
}
