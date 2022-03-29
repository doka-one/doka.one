#![feature(let_else)]

use std::collections::HashMap;
use guard::guard;
use commons_services::token_lib::SessionToken;
use rocket::{get,post, delete};
use rocket_contrib::json::Json;
use commons_error::*;
use log::error;
use log::info;
use commons_pg::{CellValue, iso_to_date, iso_to_datetime, SQLChange, SQLConnection, SQLDataSet, SQLQueryBlock, SQLTransaction};
use commons_services::database_lib::open_transaction;
use commons_services::session_lib::fetch_entry_session;
use dkdto::error_codes::{INCORRECT_DEFAULT_BOOLEAN_VALUE, INCORRECT_DEFAULT_DATE_VALUE, INCORRECT_DEFAULT_DATETIME_VALUE, INCORRECT_DEFAULT_DOUBLE_VALUE, INCORRECT_DEFAULT_INTEGER_VALUE, INCORRECT_DEFAULT_STRING_LENGTH, INCORRECT_STRING_LENGTH, INCORRECT_TAG_TYPE, INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR, INVALID_TOKEN, STILL_IN_USE, SUCCESS};
use dkdto::{AddTagReply, AddTagRequest, GetTagReply, JsonErrorSet, TagElement};

///
/// Find a session from its sid
///
#[get("/tag?<start_page>&<page_size>")]
pub (crate) fn get_all_tag(start_page : Option<u32>, page_size : Option<u32>, session_token: SessionToken) -> Json<GetTagReply> {

    // Check if the token is valid
    if !session_token.is_valid() {
        log_error!("Invalid session token {:?}", &session_token);
        return Json(GetTagReply { tags : vec![], status: JsonErrorSet::from(INVALID_TOKEN) } )
    }

    let sid = session_token.take_value();

    log_info!("🚀 Start get_all_tag api, sid={}", &sid);

    // Read the session information
    let entry_session = match fetch_entry_session(&sid).map_err(err_fwd!("Session Manager failed")) {
        Ok(x) => x,
        Err(_) => {
            return Json(GetTagReply {
                tags : vec![],
                status: JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR),
            });
        }
    };

    // Query the items
    let internal_database_error_reply = Json(GetTagReply{ tags: vec![], status : JsonErrorSet::from(INTERNAL_DATABASE_ERROR) });

    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    let tags = match search_tag_by_sid(&mut trans, None,
                                       start_page, page_size,
                                       &entry_session.customer_code ) {
        Ok(x) => {x}
        Err(_) => {
            return internal_database_error_reply;
        }
    };

    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    Json(GetTagReply{
        tags,
        status: JsonErrorSet::from(SUCCESS),
    })
}

///
/// Search items by id
/// If no item id provided, return all existing items
///
fn search_tag_by_sid(mut trans : &mut SQLTransaction, tag_id: Option<i64>,
                     start_page : Option<u32>, page_size : Option<u32>,
                     customer_code : &str) -> anyhow::Result<Vec<TagElement>> {

    let p_tag_id = CellValue::Int(tag_id);

    let mut params = HashMap::new();
    params.insert("p_tag_id".to_owned(), p_tag_id);

    let sql_query = format!(r"SELECT id, name, type, string_tag_length, default_value
                                    FROM cs_{}.tag_definition
                                    WHERE ( id = :p_tag_id OR :p_tag_id IS NULL )
                                    ORDER BY name ", customer_code );

    let query = SQLQueryBlock {
        sql_query,
        start : start_page.unwrap_or(0) * page_size.unwrap_or(0),
        length : page_size,
        params,
    };

    let mut sql_result : SQLDataSet =  query.execute(&mut trans).map_err(err_fwd!("Query failed, [{}]", &query.sql_query))?;

    let mut tags = vec![];
    while sql_result.next() {
        let id : i64 = sql_result.get_int("id").unwrap_or(0i64);
        let name : String = sql_result.get_string("name").unwrap_or("".to_owned());
        let tag_type= sql_result.get_string("type").unwrap_or("".to_owned());
        let string_tag_length = sql_result.get_int_32("string_tag_length");
        let default_value= sql_result.get_string("default_value");

        let item = TagElement {
            tag_id: id,
            name,
            tag_type,
            string_tag_length,
            default_value,
        };

        let _ = &tags.push(item);

    }

    Ok(tags)
}


///
/// Return a None if all inputs are correct
///
fn check_input_values(add_tag_request: &AddTagRequest)-> Option<AddTagReply> {

    // Check the input values ( ie tag_type, length limit, default_value type, etc )
    match add_tag_request.tag_type.to_lowercase().as_str() {
        "string" => {
            // The string_length between 0 and 10_000_000
            if let Some(length ) = add_tag_request.string_tag_length {
                if length > 10_000_000 || length < 0 {
                    return Some(AddTagReply {
                        tag_id: 0,
                        status:  JsonErrorSet::from(INCORRECT_STRING_LENGTH),
                    })
                }
                if let Some(default_string) = &add_tag_request.default_value {
                    if default_string.len() > length as usize {
                        return Some(AddTagReply {
                            tag_id: 0,
                            status:  JsonErrorSet::from(INCORRECT_DEFAULT_STRING_LENGTH),
                        })
                    }
                }
            }
        },
        "bool" => {
            if let Some(v) = &add_tag_request.default_value {
                if v != "true" && v != "false" {
                    return Some(AddTagReply {
                        tag_id: 0,
                        status:  JsonErrorSet::from(INCORRECT_DEFAULT_BOOLEAN_VALUE),
                    })
                }
            }
        },
        "integer" => {
            if let Some(v) = &add_tag_request.default_value {
                if v.parse::<i64>().is_err() {
                    return Some(AddTagReply {
                        tag_id: 0,
                        status:  JsonErrorSet::from(INCORRECT_DEFAULT_INTEGER_VALUE),
                    })
                }
            }
        },
        "double" => {
            if let Some(d) = &add_tag_request.default_value {
                if d.parse::<f64>().is_err() {
                    return Some(AddTagReply {
                        tag_id: 0,
                        status:  JsonErrorSet::from(INCORRECT_DEFAULT_DOUBLE_VALUE),
                    })
                }
            }
        },
        "date" => {
            if let Some(d_str) = &add_tag_request.default_value {
                // Check if the default is a valid date  ISO8601 1977-04-22
                if iso_to_date(d_str).is_err() {
                    return Some(AddTagReply {
                        tag_id: 0,
                        status:  JsonErrorSet::from(INCORRECT_DEFAULT_DATE_VALUE),
                    })
                }
            }
        },
        "datetime" => {
            if let Some(dt_str) = &add_tag_request.default_value {
                // Check if the default is a valid datetime ISO8601 "1977-04-22T06:00:00Z"
                if iso_to_datetime(dt_str).is_err() {
                    return Some(AddTagReply {
                        tag_id: 0,
                        status:  JsonErrorSet::from(INCORRECT_DEFAULT_DATETIME_VALUE),
                    })
                }
            }
        },
        _ => {
            return Some(AddTagReply {
                tag_id: 0,
                status:  JsonErrorSet::from(INCORRECT_TAG_TYPE),
            })
        },
    };

    None
}


fn check_tag_usage(trans : &mut SQLTransaction, tag_id: i64, customer_code : &str) -> anyhow::Result<()> {

    let sql_query = format!( r"SELECT 1 FROM cs_{}.tag_value
	                                WHERE tag_id = :p_tag_id", customer_code );

    let mut params = HashMap::new();
    params.insert("p_tag_id".to_owned(), CellValue::from_raw_int(tag_id));

    let sql = SQLQueryBlock {
        sql_query,
        start: 0,
        length: Some(1),
        params,
    };

    let dataset = sql.execute( trans)?;

    if dataset.len() > 0 {
        return Err(anyhow::anyhow!("Tag still in use"));
    }

    Ok(())
}


///
/// Create a new tag
///
#[delete("/tag/<tag_id>")]
pub (crate) fn delete_tag(tag_id: i64, session_token: SessionToken) -> Json<JsonErrorSet> {

    // Check if the token is valid
    if !session_token.is_valid() {
        return Json(
            JsonErrorSet::from(INVALID_TOKEN),
        );
    }
    let sid = session_token.take_value();

    log_info!("🚀 Start delete_tag api, sid={}", &sid);

    // Read the session information
    let entry_session = match fetch_entry_session(&sid).map_err(err_fwd!("Session Manager failed")) {
        Ok(x) => x,
        Err(_) => {
            return Json(
                JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR),
            );
        }
    };

    let customer_code = entry_session.customer_code.as_str();

    // Open the transaction

    let internal_database_error_reply = Json(
        JsonErrorSet::from(INTERNAL_DATABASE_ERROR),
    );

    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    // Check if the tag definition is used somewhere

    if check_tag_usage(&mut trans, tag_id, customer_code).is_err() {
        return Json(
            JsonErrorSet::from(STILL_IN_USE),
        );
    }

    // Delete the tag definition

    let sql_query = format!( r"DELETE FROM cs_{}.tag_definition
	                                WHERE id = :p_tag_id", customer_code );

    let mut params = HashMap::new();
    params.insert("p_tag_id".to_string(), CellValue::from_raw_int(tag_id));

    dbg!(&params);

    let sql_delete = SQLChange {
        sql_query,
        params,
        sequence_name: "".to_string()
    };

    let _tag_id = match sql_delete.delete(&mut trans).map_err(err_fwd!("Tag delete failed, tag_id=[{}]", tag_id)) {
        Ok(x) => x,
        Err(_) => {
            return internal_database_error_reply;
        }
    };

    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    Json(
        JsonErrorSet::from(SUCCESS),
    )

}

///
/// Create a new tag
///
#[post("/tag", format = "application/json", data = "<add_tag_request>")]
pub (crate) fn add_tag(add_tag_request: Json<AddTagRequest>, session_token: SessionToken) -> Json<AddTagReply> {
    dbg!(&add_tag_request);
    // Check if the token is valid
    if !session_token.is_valid() {
        return Json(AddTagReply {
            tag_id: 0,
            status: JsonErrorSet::from(INVALID_TOKEN),
        });
    }
    let sid = session_token.take_value();

    log_info!("🚀 Start add_tag api, sid={}", &sid);

    let internal_database_error_reply = Json(AddTagReply {
        tag_id: 0,
        status: JsonErrorSet::from(INTERNAL_DATABASE_ERROR),
    });

    let _internal_technical_error = Json(AddTagReply {
        tag_id: 0,
        status: JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR),
    });

    // Read the session information
    let entry_session = match fetch_entry_session(&sid).map_err(err_fwd!("Session Manager failed")) {
        Ok(x) => x,
        Err(_) => {
            return Json(AddTagReply {
                tag_id: 0,
                status: JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR),
            });
        }
    };

    let customer_code = entry_session.customer_code.as_str();

    if let Some(err) = check_input_values(&add_tag_request) {
        return Json(err);
    }

    // Open the transaction
    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    let sql_query = format!( r"INSERT INTO cs_{}.tag_definition(name, string_tag_length, default_value, type)
	VALUES (:p_name, :p_string_tag_length , :p_default_value, :p_type)", customer_code );

    let sequence_name = format!( "cs_{}.tag_definition_id_seq", customer_code );

    let length = CellValue::Int32(add_tag_request.string_tag_length);
    let default_value = CellValue::from_opt_str(add_tag_request.default_value.as_deref());
    let mut params = HashMap::new();
    params.insert("p_name".to_string(), CellValue::from_raw_string(add_tag_request.name.clone()));
    params.insert("p_type".to_string(), CellValue::from_raw_string(add_tag_request.tag_type.clone()));
    params.insert("p_string_tag_length".to_string(), length);
    params.insert("p_default_value".to_string(), default_value);

    dbg!(&params);

    let sql_insert = SQLChange {
        sql_query,
        params,
        sequence_name,
    };


    let r_tag_id = sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion of a new item failed"));
    guard!(let Ok(tag_id) = r_tag_id else {
        return internal_database_error_reply;
    });

    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    dbg!(tag_id);

    Json(AddTagReply {
        tag_id,
        status: JsonErrorSet::from(SUCCESS),
    })
}


#[cfg(test)]
mod test {

    use chrono::{Datelike, DateTime, NaiveDateTime, Timelike, Utc};
    use commons_pg::{iso_to_date, iso_to_datetime};

    #[test]
    fn is_valid_datetime_test() {

        assert!(iso_to_datetime("1977-04-22T06:12:04Z").is_ok());
        assert!(iso_to_datetime("1977-04-22T23:12:04Z").is_ok());
        assert!(iso_to_datetime("1977-04-22T23:12:04+01:00").is_ok());
        assert!(iso_to_datetime("0000-12-04T22:12:04Z").is_ok());

        assert!(iso_to_datetime("1977-04-22T26:12:04Z").is_err());
        assert!(iso_to_datetime("1977-0422T22:12:04Z").is_err());
        assert!(iso_to_datetime("1977-04T22:12:04Z").is_err());
        assert!(iso_to_datetime("1977-04-22T22:12:04+01").is_err());
        assert!(iso_to_datetime("1977-13-04T22:12:04Z").is_err());
    }

    #[test]
    fn is_valid_date_test() {

        assert!(iso_to_date("1977-04-22").is_ok());
        assert!(iso_to_date("2000-02-29").is_ok());

        assert!(iso_to_date("1977-13-26").is_err());
        assert!(iso_to_date("1977-02-32").is_err());
        assert!(iso_to_date("1977-02-29").is_err());
        assert!(iso_to_date("1977-02").is_err());
    }

    #[test]
    fn convert_iso8601_str_to_datetime() {
        let r_dt = DateTime::parse_from_rfc3339("1977-04-22T06:12:04Z");

        match r_dt {
            Ok(dt) => {
                assert_eq!(1977, dt.year());
                assert_eq!(4, dt.month());
                assert_eq!(22, dt.day());
                assert_eq!(6, dt.hour());
                assert_eq!(12, dt.minute());
                assert_eq!(4, dt.second());
                assert_eq!("+00:00", dt.timezone().to_string());
            }
            Err(_) => {
                assert!(false);
            }
        }
    }

    #[test]
    fn convert_iso8601_str_to_date() {
        let r_dt = DateTime::parse_from_rfc3339("1977-04-22T00:00:00Z");

        match r_dt {
            Ok(dt) => {
                let d = dt.date();
                assert_eq!(1977,  d.year());
                assert_eq!(4, dt.month());
                assert_eq!(22, dt.day());
            }
            Err(_) => {
                assert!(false);
            }
        }
    }

    #[test]
    fn convert_datetime_to_iso8601_string() {

        let dt = Utc::now();

        let s = dt.to_rfc3339();

        dbg!(s);


    }

}