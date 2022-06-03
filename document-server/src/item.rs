use std::collections::HashMap;
use std::time::SystemTime;
use anyhow::anyhow;
use chrono::{DateTime, Utc};
use rocket_contrib::json::Json;

use commons_pg::{CellValue, date_time_to_iso, date_to_iso, iso_to_date, iso_to_datetime, SQLChange, SQLConnection, SQLDataSet, SQLQueryBlock, SQLTransaction};
use commons_error::*;
use log::{error, info, debug};
use commons_services::database_lib::open_transaction;

use commons_services::token_lib::{SessionToken};
use commons_services::session_lib::{fetch_entry_session};
use commons_services::x_request_id::{Follower, XRequestID};
use dkdto::error_codes::{SUCCESS};
use dkdto::{AddItemReply, AddItemRequest, EnumTagValue, GetItemReply, ItemElement, JsonErrorSet, AddTagValue, TagValueElement};
use dkdto::error_replies::ErrorReply;
use doka_cli::request_client::TokenType;

pub(crate) struct ItemDelegate {
    pub session_token: SessionToken,
    pub follower: Follower,
}

impl ItemDelegate {
    pub fn new(session_token: SessionToken, x_request_id: XRequestID) -> Self {

        Self {
            session_token,
            follower: Follower {
                x_request_id : x_request_id.new_if_null(),
                token_type: TokenType::None,
            }
        }
    }

    ///
    /// ✨ Find all the items at page [start_page]
    ///
    pub fn get_all_item(mut self, start_page : Option<u32>, page_size : Option<u32>) -> Json<GetItemReply> {

        // Already done in the delegate constructor : self.follower.x_request_id = self.follower.x_request_id.new_if_null();

        log_info!("🚀 Start get_all_item api, start_page=[{:?}], page_size=[{:?}], follower=[{}]", start_page, page_size, &self.follower);

        // Check if the token is valid
        if !self.session_token.is_valid() {
            log_error!("Invalid session token {:?}", &self.session_token);
            return Json(GetItemReply::invalid_token_error_reply())
        }

        self.follower.token_type = TokenType::Sid(self.session_token.0.clone());

        // Read the session information
        let Ok(entry_session) =  fetch_entry_session(&self.follower.token_type.value())
            .map_err(err_fwd!("💣 Session Manager failed, follower=[{}]", &self.follower)) else {
            return Json(GetItemReply::internal_technical_error_reply());
        };

        log_info!("😎 We fetched the session, follower=[{}]", &self.follower);

        // Query the items
        let internal_database_error_reply: Json<GetItemReply> = Json(GetItemReply::internal_database_error_reply());

        let mut r_cnx = SQLConnection::new();
        let r_trans = open_transaction(&mut r_cnx).map_err(err_fwd!("💣 Open transaction error, follower=[{}]", &self.follower));
        let Ok(mut trans) = r_trans else {
            return internal_database_error_reply;
        };

        let Ok(items) = self.search_item_by_id(&mut trans, None,
                                          start_page, page_size,
                                          &entry_session.customer_code ) else {
            log_error!("💣 Cannot find item by id, follower=[{}]", &self.follower);
            return internal_database_error_reply;
        };

        log_info!("😎 We found the items, item count=[{}], follower=[{}]", items.len(), &self.follower);

        if trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower)).is_err() {
            return internal_database_error_reply;
        }

        log_info!("🏁 End get_all_item, follower=[{}]", &self.follower);

        Json(GetItemReply{
            items,
            status: JsonErrorSet::from(SUCCESS),
        })
    }


    /// Search items by id
    /// If no item id provided, return all existing items
    fn search_item_by_id(&self, mut trans : &mut SQLTransaction, item_id: Option<i64>,
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
                .ok_or(anyhow::anyhow!("Wrong created gmt"))
                .map_err(tr_fwd!())?;

            // Optional
            let last_modified_gmt = sql_result.get_timestamp_as_datetime("last_modified_gmt")
                .as_ref().map( |x| date_time_to_iso(x) );

            let props = self.find_item_properties(trans, id, customer_code).map_err(tr_fwd!())?;

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
    fn find_item_properties(&self, trans : &mut SQLTransaction, item_id : i64, customer_code : &str) -> anyhow::Result<Vec<TagValueElement>> {
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

        let mut sql_result=  query.execute( trans).map_err(err_fwd!("Query failed, [{}]", &query.sql_query))?;

        while sql_result.next() {

            let _name : String = sql_result.get_string("name").ok_or(anyhow!("Wrong name"))?;
            let tag_type : String = sql_result.get_string("type").ok_or(anyhow!("Wrong type"))?;

            let r_value = match tag_type.to_lowercase().as_str() {
                "string" => {
                    let value_string = sql_result.get_string("value_string");
                    Ok(EnumTagValue::String(value_string))
                }
                "bool" => {
                    let value_boolean  = sql_result.get_bool("value_boolean");
                    Ok(EnumTagValue::Boolean(value_boolean))
                }
                "integer" => {
                    let value_integer = sql_result.get_int("value_integer");
                    Ok(EnumTagValue::Integer(value_integer))
                }
                "double" => {
                    let value_double = sql_result.get_double("value_double");
                    Ok(EnumTagValue::Double(value_double))
                }
                "date" => {
                    let value_date = sql_result.get_naivedate_as_date("value_date");
                    let opt_iso_d_str = value_date.as_ref().map(|x| date_to_iso(x));
                    Ok(EnumTagValue::SimpleDate(opt_iso_d_str))
                }
                "datetime" => {
                    let value_datetime = sql_result.get_timestamp_as_datetime("value_datetime");
                    let opt_iso_dt_str = value_datetime.as_ref().map(|x| date_time_to_iso(x));
                    Ok(EnumTagValue::DateTime(opt_iso_dt_str))
                }
                v => {
                    Err(anyhow!(format!("Wrong tag type, [{}]", v)))
                }
            };

            let value = r_value.map_err(tr_fwd!())?;

            let tv = TagValueElement {
                tag_value_id: 0,
                item_id: 0,
                tag_id: 0,
                value,
            };
            let _ = &props.push(tv);
        }

        Ok(props)
    }


    ///
    /// ✨ Find a item from its item id
    ///
    pub fn get_item(mut self, item_id: i64) -> Json<GetItemReply> {

        // Done in the delegate constructor : self.follower.x_request_id = self.follower.x_request_id.new_if_null();

        log_info!("🚀 Start get_item api, item_id=[{}], follower=[{}]", item_id, &self.follower);

        // Check if the token is valid
        if !self.session_token.is_valid() {
            log_error!("💣 Invalid session token {:?}", &self.session_token);
            return Json(GetItemReply::invalid_token_error_reply());
        }

        self.follower.token_type = TokenType::Sid(self.session_token.0.clone());

        // Read the session information
        let Ok(entry_session) = fetch_entry_session(&self.follower.token_type.value())
                                    .map_err(err_fwd!("💣 Session Manager failed, follower=[{}]", &self.follower)) else {
            return Json(GetItemReply::internal_technical_error_reply());
        };

        log_info!("😎 We fetched the session, follower=[{}]", &self.follower);

        // Query the item
        let internal_database_error_reply: Json<GetItemReply> = Json(GetItemReply::internal_database_error_reply());

        let mut r_cnx = SQLConnection::new();
        let r_trans = open_transaction(&mut r_cnx).map_err(err_fwd!("💣 Open transaction error, follower=[{}]", &self.follower));
        let Ok(mut trans) = r_trans else {
            return internal_database_error_reply;
        };

        let Ok(items) =  self.search_item_by_id(&mut trans, Some(item_id),
                                                 None, None,
                                                 &entry_session.customer_code )
                                        .map_err(err_fwd!("💣 Cannot search item by id, follower=[{}]", &self.follower)) else {
            return internal_database_error_reply;
        };

        log_info!("😎 We found the item, item count=[{}], follower=[{}]", items.len(), &self.follower);

        if trans.commit().map_err(err_fwd!("💣 Commit failed")).is_err() {
            return internal_database_error_reply;
        }

        log_info!("🏁 End get_item, follower=[{}]", &self.follower);

        Json(GetItemReply{
            items,
            status: JsonErrorSet::from(SUCCESS),
        })
    }


    ///
    /// ✨ Create an item
    ///
    pub fn add_item(mut self, add_item_request: Json<AddItemRequest>) -> Json<AddItemReply> {

        log_info!("🚀 Start add_item api, add_item_request=[{:?}], follower=[{}]", &add_item_request, &self.follower);

        // Check if the token is valid
        if !self.session_token.is_valid() {
            return Json(AddItemReply::invalid_token_error_reply());
        }

        self.follower.token_type = TokenType::Sid(self.session_token.0.clone());

        let internal_database_error_reply: Json<AddItemReply> = Json(AddItemReply::internal_database_error_reply());

        // Read the session information
        let Ok(entry_session) = fetch_entry_session(&self.follower.token_type.value()).map_err(err_fwd!("💣 Session Manager failed, follower=[{}]", &self.follower)) else {
            return Json(AddItemReply::internal_technical_error_reply())
        };
        let customer_code = entry_session.customer_code.as_str();

        log_info!("😎 We read the session information, customer_code=[{}], follower=[{}]", customer_code, &self.follower);

        // Open the transaction
        let mut r_cnx = SQLConnection::new();
        let r_trans = open_transaction(&mut r_cnx).map_err(err_fwd!("💣 Open transaction error, follower=[{}]", &self.follower));
        let Ok(mut trans) = r_trans else {
            return internal_database_error_reply;
        };

        let Ok(item_id) = self.create_item(&mut trans,  &add_item_request.name, customer_code)
                                .map_err(err_fwd!("💣 Cannot create the item, follower=[{}]", &self.follower)) else {
            return internal_database_error_reply;
        };

        log_info!("😎 We created the item, item_id=[{}], follower=[{}]", item_id, &self.follower);

        // | Insert all the properties
        if let Some(properties) = &add_item_request.properties {
            for prop in properties {
                if self.create_item_property(&mut trans, prop, item_id, customer_code)
                    .map_err(err_fwd!("💣 Insertion of a new tag value failed, tag value=[{:?}], follower=[{}]", prop, &self.follower)).is_err() {
                    return internal_database_error_reply;
                }
                log_debug!("😎 We added the property to the item, prop name=[{:?}], follower=[{}]", prop.value, &self.follower);
            }
        }

        log_info!("😎 We added all the properties to the item, item_id=[{}], follower=[{}]", item_id, &self.follower);

        if trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower)).is_err() {
            return internal_database_error_reply;
        }

        let now = SystemTime::now();
        let created : DateTime<Utc> = now.clone().into();
        let last_modified : DateTime<Utc> = now.clone().into();

        log_info!("🏁 End add_item, follower=[{}]", &self.follower);

        Json(AddItemReply {
            item_id,
            name: add_item_request.name.clone(),
            created : date_time_to_iso(&created),
            last_modified: Some(date_time_to_iso(&last_modified)),
            status: JsonErrorSet::from(SUCCESS),
        })
    }


    fn create_item(&self, trans : &mut SQLTransaction, item_name: &str, customer_code : &str) -> anyhow::Result<i64> {
        let sql_query = format!( r"INSERT INTO cs_{}.item(name, created_gmt, last_modified_gmt)
                                        VALUES (:p_name, :p_created, :p_last_modified)", customer_code );

        let sequence_name = format!( "cs_{}.item_id_seq", customer_code );

        let now = SystemTime::now();
        let mut params = HashMap::new();
        params.insert("p_name".to_string(), CellValue::from_raw_string(item_name.to_string()));
        params.insert("p_created".to_string(), CellValue::from_raw_systemtime(now.clone()));
        params.insert("p_last_modified".to_string(), CellValue::from_raw_systemtime(now.clone()));

        let sql_insert = SQLChange {
            sql_query,
            params,
            sequence_name,
        };

        let item_id = sql_insert.insert(trans).map_err(err_fwd!("Insertion of a new item failed, follower=[{}]", &self.follower))?;

        log_info!("Created item : item_id=[{}]", item_id);
        Ok(item_id)
    }

    ///
    fn create_item_property(&self, trans : &mut SQLTransaction, prop :&AddTagValue, item_id : i64, customer_code : &str) -> anyhow::Result<()> {

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
                    let dt = iso_to_date(d_string)
                        .map_err(err_fwd!("Cannot convert the string to datetime:[{}], follower=[{}]", d_string, &self.follower)).ok()?;
                    let nd = dt.naive_utc();
                    Some(nd)
                })();
                params.insert("p_val_date".to_string(), CellValue::Date(opt_st));
            }
            EnumTagValue::DateTime(tv) => {
                let opt_st = (|| {
                    let dt_string = tv.as_ref()?;
                    let dt = iso_to_datetime(dt_string)
                        .map_err(err_fwd!("Cannot convert the string to datetime:[{}], follower=[{}]", dt_string, &self.follower)).ok()?;
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

        log_debug!("Created the property, prop tag id=[{}], follower=[{}]", prop.tag_id, &self.follower);

        let _ = sql_insert.insert(trans).map_err(err_fwd!("Cannot insert the tag value, follower=[{}]", &self.follower))?;
        Ok(())
    }
}


