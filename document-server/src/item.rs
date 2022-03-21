use std::collections::HashMap;
use std::time::SystemTime;
use chrono::{DateTime, Utc};
use rocket::{get,post};
use rocket_contrib::json::Json;

use commons_pg::{CellValue, date_time_to_iso, date_to_iso, iso_to_date, iso_to_datetime, SQLChange, SQLConnection, SQLDataSet, SQLQueryBlock, SQLTransaction};
use commons_error::*;
use log::error;
use log::info;
use commons_services::database_lib::open_transaction;

use commons_services::token_lib::{SessionToken};
use commons_services::session_lib::{fetch_entry_session};
use dkdto::error_codes::{INTERNAL_TECHNICAL_ERROR, SUCCESS};
use dkdto::{AddItemReply, AddItemRequest, EnumTagValue, GetItemReply, ItemElement, JsonErrorSet, AddTagValue, TagValueElement};
use dkdto::error_replies::ErrorReply;
use crate::item_query::create_item;

///
/// Find a session from its sid
///
#[get("/item?<start_page>&<page_size>")]
pub (crate) fn get_all_item(start_page : Option<u32>, page_size : Option<u32>, session_token: SessionToken) -> Json<GetItemReply> {

    // Check if the token is valid
    if !session_token.is_valid() {
        log_error!("Invalid session token {:?}", &session_token);
        return Json(GetItemReply::invalid_token_error_reply())
    }

    let sid = session_token.take_value();

    log_info!("🚀 Start get_all_item api, sid={}", &sid);

    // Read the session information
    let entry_session = match fetch_entry_session(&sid).map_err(err_fwd!("Session Manager failed")) {
        Ok(x) => x,
        Err(_) => {
            return Json(GetItemReply::internal_technical_error_reply());
        }
    };

    // Query the items
    let internal_database_error_reply: Json<GetItemReply> = Json(GetItemReply::internal_database_error_reply());

    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    let items = match search_item_by_sid(&mut trans, None,
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

    Json(GetItemReply{
        items,
        status: JsonErrorSet::from(SUCCESS),
    })
}

///
/// Search items by id
/// If no item id provided, return all existing items
///
fn search_item_by_sid(mut trans : &mut SQLTransaction, item_id: Option<i64>,
                      start_page : Option<u32>, page_size : Option<u32>,
                      customer_code : &str) -> anyhow::Result<Vec<ItemElement>> {

    let p_item_id = CellValue::Int(item_id);

    let mut params = HashMap::new();
    params.insert("p_item_id".to_owned(), p_item_id);

    let sql_query = format!( r"SELECT id, name, created_gmt, last_modified_gmt
                    FROM cs_{}.item
                    WHERE ( id = :p_item_id OR  :p_item_id IS NULL )
                    ORDER BY name ", customer_code );

    let query = SQLQueryBlock {
        sql_query,
        start : start_page.unwrap_or(0) *  page_size.unwrap_or(0),
        length : page_size,
        params,
    };

    let mut sql_result : SQLDataSet =  query.execute(&mut trans).map_err(err_fwd!("Query failed, [{}]", &query.sql_query))?;

    let mut items = vec![];
    while sql_result.next() {
        let id : i64 = sql_result.get_int("id").unwrap_or(0i64);
        let name : String = sql_result.get_string("name").unwrap_or("".to_owned());
        let created_gmt  = sql_result.get_timestamp_as_datetime("created_gmt")
            .ok_or(anyhow::anyhow!(""))
            .map_err(err_fwd!("Cannot read the creation datetime"))?;

        let last_modified_gmt = sql_result.get_timestamp_as_datetime("last_modified_gmt")
                                    .as_ref().map( |x| date_time_to_iso(x) );

        let props = find_item_properties(trans, id, customer_code);

        let item = ItemElement {
            item_id: id,
            name,
            created : date_time_to_iso(&created_gmt),
            last_modified : last_modified_gmt,
            properties: Some(props),
        };

        let _ = &items.push(item);

    }

    Ok(items)
}

///
///
///
fn find_item_properties(trans : &mut SQLTransaction, item_id : i64, customer_code : &str) -> Vec<TagValueElement> {
    let mut props = vec![];

    let p_item_id = CellValue::from_raw_int(item_id);

    let mut params = HashMap::new();
    params.insert("p_item_id".to_owned(), p_item_id);

    let sql_query = format!( r"SELECT td.name, td.type, tv.id, tv.tag_id, tv.item_id, tv.value_string, tv.value_integer, tv.value_double,
                tv.value_date, tv.value_datetime, tv.value_boolean
                FROM cs_{}.tag_value tv
                INNER JOIN cs_{}.tag_definition td ON td.id = tv.tag_id
                WHERE tv.item_id = :p_item_id ", customer_code, customer_code );

    let query = SQLQueryBlock {
        sql_query,
        start : 0,
        length : None,
        params,
    };

    let mut sql_result : SQLDataSet =  match query.execute( trans).map_err(err_fwd!("Query failed, [{}]", &query.sql_query)) {
        Ok(x) => x,
        Err(e) => {
            log_error!("{}", e);
            return props;
        }
    };

    while sql_result.next() {

        let _name : String = sql_result.get_string("name").unwrap_or("".to_owned());
        let tag_type : String = sql_result.get_string("type").unwrap_or("".to_owned());


        let value = match tag_type.to_lowercase().as_str() {
            "string" => {
                let value_string = sql_result.get_string("value_string");
                EnumTagValue::String(value_string)
            }
            "bool" => {
                let value_boolean  = sql_result.get_bool("value_boolean");
                EnumTagValue::Boolean(value_boolean)
            }
            "integer" => {
                let value_integer = sql_result.get_int("value_integer");
                EnumTagValue::Integer(value_integer)
            }
            "double" => {
                let value_double = sql_result.get_double("value_double");
                EnumTagValue::Double(value_double)
            }
            "date" => {
                let value_date = sql_result.get_naivedate_as_date("value_date");
                dbg!(&value_date);
                let opt_iso_d_str = value_date.as_ref().map(|x| date_to_iso(x));
                EnumTagValue::SimpleDate(opt_iso_d_str)
            }
            "datetime" => {
                let value_datetime = sql_result.get_timestamp_as_datetime("value_datetime");
                let opt_iso_dt_str = value_datetime.as_ref().map(|x| date_time_to_iso(x));
                EnumTagValue::DateTime(opt_iso_dt_str)
            }
            v => {
                log_error!("Wrong tag type, [{}]", v);
                return props;
            }
        };


        let tv = TagValueElement {
            tag_value_id: 0,
            item_id: 0,
            tag_id: 0,
            value,
        };
        let _ = &props.push(tv);
    }

    props
}

///
/// Find a item from its item id
///
#[get("/item/<item_id>")]
pub (crate) fn get_item(item_id: i64, session_token: SessionToken) -> Json<GetItemReply> {

    // Check if the token is valid
    if !session_token.is_valid() {
        log_error!("Invalid session token {:?}", &session_token);
        return Json(GetItemReply::invalid_token_error_reply());
    }

    let sid = session_token.take_value();

    log_info!("🚀 Start get_item api, sid=[{}], item_id=[{}]", &sid, item_id);

    // Read the session information
    let entry_session = match fetch_entry_session(&sid).map_err(err_fwd!("Session Manager failed")) {
        Ok(x) => x,
        Err(_) => {
            return Json(GetItemReply::internal_technical_error_reply());
        }
    };

    // Query the item
    let internal_database_error_reply: Json<GetItemReply> = Json(GetItemReply::internal_database_error_reply());

    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    let items = match search_item_by_sid(&mut trans, Some(item_id),
                                         None, None,
                                         &entry_session.customer_code ) {
        Ok(x) => {x}
        Err(_) => {
            return internal_database_error_reply;
        }
    };

    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    Json(GetItemReply{
        items,
        status: JsonErrorSet::from(SUCCESS),
    })
}

///
///
///
fn create_item_property(trans : &mut SQLTransaction, prop :&AddTagValue, item_id : i64, customer_code : &str) -> anyhow::Result<()> {

    // FIXME BUG: we named the variable :p_val_date because otherwise it conflict with :p_value_datetime
    //              the replacement expression should be ":variable:" to avoid this case
    let sql_query = format!(r"INSERT INTO cs_{}.tag_value (tag_id, item_id, value_boolean, value_string, value_integer, value_double, value_date, value_datetime)
                 VALUES (:p_tag_id, :p_item_id, :p_value_boolean, :p_value_string, :p_value_integer, :p_value_double, :p_val_date, :p_value_datetime) ", customer_code);

    let mut params = HashMap::new();
    params.insert("p_tag_id".to_string(), CellValue::from_raw_int(prop.tag_id));
    params.insert("p_item_id".to_string(), CellValue::from_raw_int(item_id));

    params.insert("p_value_string".to_string(), CellValue::String(None));
    params.insert("p_value_boolean".to_string(), CellValue::Bool(None));
    params.insert("p_value_integer".to_string(), CellValue::Int(None));
    params.insert("p_value_double".to_string(), CellValue::Double(None));
    params.insert("p_val_date".to_string(), CellValue::Date(None));
    params.insert("p_value_datetime".to_string(), CellValue::SystemTime(None));

    match &prop.value {
        EnumTagValue::String(tv) => {
            params.insert("p_value_string".to_string(), CellValue::String(tv.clone()));
        }
        EnumTagValue::Boolean(tv) => {
            params.insert("p_value_boolean".to_string(), CellValue::Bool(*tv));
        }
        EnumTagValue::Integer(tv) => {
            params.insert("p_value_integer".to_string(), CellValue::Int(*tv));
        }
        EnumTagValue::Double(tv) => {
            params.insert("p_value_double".to_string(), CellValue::Double(*tv));
        }
        EnumTagValue::SimpleDate(tv) => {
            let opt_st = (|| {
                let d_string = tv.as_ref()?;
                dbg!(&d_string);
                let dt = iso_to_date(d_string).map_err(err_fwd!("Cannot convert the string to datetime:[{}]", d_string)).ok()?;
                let nd = dt.naive_utc();
                dbg!(&nd);
                Some(nd)
            })();
            params.insert("p_val_date".to_string(), CellValue::Date(opt_st));
        }
        EnumTagValue::DateTime(tv) => {
            let opt_st = (|| {
                let dt_string = tv.as_ref()?;
                let dt = iso_to_datetime(dt_string).map_err(err_fwd!("Cannot convert the string to datetime:[{}]", dt_string)).ok()?;
                Some(SystemTime::from(dt))
            })();
            params.insert("p_value_datetime".to_string(), CellValue::SystemTime(opt_st));
        }
    }

    let sql_insert = SQLChange {
        sql_query,
        params,
        sequence_name: format!("cs_{}.tag_value_id_seq", customer_code)
    };

    let _ = sql_insert.insert(trans).map_err(err_fwd!("Cannot insert the tag value"))?;
    Ok(())
}

///
/// Create an item
///
#[post("/item", format = "application/json", data = "<add_item_request>")]
pub (crate) fn add_item(add_item_request: Json<AddItemRequest>, session_token: SessionToken) -> Json<AddItemReply> {
    dbg!(&add_item_request);
    // Check if the token is valid
    if !session_token.is_valid() {
        return Json(AddItemReply::invalid_token_error_reply());
    }
    let sid = session_token.take_value();

    log_info!("🚀 Start add_item api, sid={}", &sid);

    let internal_database_error_reply: Json<AddItemReply> = Json(AddItemReply::internal_database_error_reply());
    let _internal_technical_error: Json<AddItemReply> = Json(AddItemReply::internal_technical_error_reply());

    // Read the session information
    let entry_session = match fetch_entry_session(&sid).map_err(err_fwd!("Session Manager failed")) {
        Ok(x) => x,
        Err(_) => {
            return Json(AddItemReply {
                item_id: 0i64,
                name: "".to_string(),
                created: "".to_string(),
                last_modified: None,
                status: JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR),
            });
        }
    };

    let customer_code = entry_session.customer_code.as_str();

    // Open the transaction
    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    let item_id= match create_item(&mut trans,  &add_item_request.name, customer_code) {
        Ok(id) => {id}
        Err(_) => {
            return internal_database_error_reply;
        }
    };

    // | Insert all the properties
    if let Some(properties) = &add_item_request.properties {
        for prop in properties {
            if create_item_property(&mut trans, prop, item_id, customer_code).map_err(err_fwd!("Insertion of a new tag value failed, tag value=[{:?}]", prop)).is_err() {
                return internal_database_error_reply;
            }
        }
    }

    //
    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    dbg!(item_id);

    let now = SystemTime::now();
    let created : DateTime<Utc> = now.clone().into();
    let last_modified : DateTime<Utc> = now.clone().into();

    Json(AddItemReply {
        item_id,
        name: add_item_request.name.clone(),
        created : date_time_to_iso(&created),
        last_modified: Some(date_time_to_iso(&last_modified)),
        status: JsonErrorSet::from(SUCCESS),
    })
}

