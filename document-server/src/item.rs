use std::collections::HashMap;
use std::ops::Deref;
use std::time::SystemTime;

use anyhow::anyhow;
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Utc};
use log::{debug, error, info};
use serde::de::DeserializeOwned;

use commons_error::*;
use commons_pg::sql_transaction::{
    date_time_to_iso, iso_to_datetime, iso_to_naivedate, naivedate_to_iso, CellValue, SQLDataSet,
};
use commons_pg::sql_transaction_async::{
    SQLChangeAsync, SQLConnectionAsync, SQLQueryBlockAsync, SQLTransactionAsync,
};
use commons_services::session_lib::valid_sid_get_session;
use commons_services::token_lib::SessionToken;
use commons_services::try_or_return;
use commons_services::x_request_id::{Follower, XRequestID};
use dkdto::error_codes::{
    BAD_TAG_FOR_ITEM, INCORRECT_TAG_TYPE, INTERNAL_DATABASE_ERROR, MISSING_ITEM,
    MISSING_TAG_FOR_ITEM,
};
use dkdto::{
    AddItemReply, AddItemRequest, AddItemTagReply, AddItemTagRequest, AddTagRequest, AddTagValue,
    EnumTagValue, ErrorSet, GetItemReply, ItemElement, SimpleMessage, TagValueElement,
    WebTypeBuilder, TAG_TYPE_BOOL, TAG_TYPE_DATE, TAG_TYPE_DATETIME, TAG_TYPE_DOUBLE, TAG_TYPE_INT,
    TAG_TYPE_LINK, TAG_TYPE_STRING,
};
use doka_cli::request_client::TokenType;

use crate::filter_ast::{analyse_expression, to_sql_form, FilterExpressionAST};
use crate::{TagDelegate, WebType};

pub(crate) struct ItemDelegate {
    pub session_token: SessionToken,
    pub follower: Follower,
}

impl ItemDelegate {
    pub fn new(session_token: SessionToken, x_request_id: XRequestID) -> Self {
        Self {
            session_token,
            follower: Follower {
                x_request_id: x_request_id.new_if_null(),
                token_type: TokenType::None,
            },
        }
    }

    ///
    /// ✨ Find all the items at page [start_page]
    ///
    pub async fn search_item(
        mut self,
        start_page: Option<u32>,
        page_size: Option<u32>,
        filters: Option<String>,
    ) -> WebType<GetItemReply> {
        log_info!(
            "🚀 Start get_all_item api, start_page=[{:?}], page_size=[{:?}], follower=[{}]",
            start_page,
            page_size,
            &self.follower
        );

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        let filter_tokens: Box<FilterExpressionAST> =
            match analyse_expression(&filters.unwrap_or("()".to_owned())) {
                Ok(v) => v,
                Err(_) => {
                    // TODO
                    panic!("Cannot lex the expression");
                }
            };

        let s = to_sql_form(&filter_tokens.deref()).unwrap(); // TODO
        log_info!("sql = {}", &s);

        // let r_lexemes = filter_lexer::lex3(&filters.0);
        // // Normalise !!!
        //
        // let Ok(lexeme) = r_lexemes else {
        //     panic!("Cannot lex the expression");
        // };
        //
        // let filter_tokens: Box<FilterExpressionAST> = parse_tokens(&lexeme).unwrap(); // TODO * 2

        // Verify the the attributes are existing tag in doka and the type complies with the filter condition
        // ...

        log_info!("😎 We fetched the session, follower=[{}]", &self.follower);

        // Open Db connection
        let Ok(mut cnx) = SQLConnectionAsync::from_pool().await.map_err(err_fwd!(
            "💣 New Db connection failed, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(mut trans) = cnx.begin().await.map_err(err_fwd!(
            "💣 Transaction issue, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(items) = self
            .search_item_with_filter(
                &mut trans,
                &filter_tokens.deref(),
                start_page,
                page_size,
                &entry_session.customer_code,
            )
            .await
        else {
            log_error!("💣 Cannot find item by id, follower=[{}]", &self.follower);
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        log_info!(
            "😎 We found the items, item count=[{}], follower=[{}]",
            items.len(),
            &self.follower
        );

        if trans
            .commit()
            .await
            .map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))
            .is_err()
        {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        log_info!("🏁 End get_all_item, follower=[{}]", &self.follower);

        WebType::from_errorset(&INTERNAL_DATABASE_ERROR)
    }

    /// Deprecated - replace it with search_item
    /// ✨ Find all the items at page [start_page]
    ///
    pub async fn get_all_item(
        mut self,
        start_page: Option<u32>,
        page_size: Option<u32>,
    ) -> WebType<GetItemReply> {
        // Already done in the delegate constructor : self.follower.x_request_id = self.follower.x_request_id.new_if_null();

        log_info!(
            "🚀 Start get_all_item api, start_page=[{:?}], page_size=[{:?}], follower=[{}]",
            start_page,
            page_size,
            &self.follower
        );

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        self.follower.token_type = TokenType::Sid(self.session_token.0.clone());

        // // Check if the token is valid
        // if !self.session_token.is_valid() {
        //     log_error!(
        //         "💣 Invalid session token=[{:?}], follower=[{}]",
        //         &self.session_token,
        //         &self.follower
        //     );
        //     return WebType::from_errorset(&INVALID_TOKEN);
        // }
        //
        // // Read the session information
        // let Ok(entry_session) = fetch_entry_session(&self.follower.token_type.value())
        //     .await
        //     .map_err(err_fwd!(
        //         "💣 Session Manager failed, follower=[{}]",
        //         &self.follower
        //     ))
        // else {
        //     return WebType::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        // };

        log_info!("😎 We fetched the session, follower=[{}]", &self.follower);

        // Open Db connection
        let Ok(mut cnx) = SQLConnectionAsync::from_pool().await.map_err(err_fwd!(
            "💣 New Db connection failed, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(mut trans) = cnx.begin().await.map_err(err_fwd!(
            "💣 Transaction issue, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(items) = self
            .search_item_by_id(
                &mut trans,
                None,
                start_page,
                page_size,
                &entry_session.customer_code,
            )
            .await
        else {
            log_error!("💣 Cannot find item by id, follower=[{}]", &self.follower);
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        log_info!(
            "😎 We found the items, item count=[{}], follower=[{}]",
            items.len(),
            &self.follower
        );

        if trans
            .commit()
            .await
            .map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))
            .is_err()
        {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        log_info!("🏁 End get_all_item, follower=[{}]", &self.follower);

        WebType::from_item(StatusCode::OK.as_u16(), GetItemReply { items })
    }

    /// Search items from the filter given
    /// If no item id provided, return all existing items
    /// TODO Merge the main query with the property query in order to reduce the number of SQL queries
    async fn search_item_with_filter(
        &self,
        mut trans: &mut SQLTransactionAsync<'_>,
        filters: &FilterExpressionAST,
        start_page: Option<u32>,
        page_size: Option<u32>,
        customer_code: &str,
    ) -> anyhow::Result<Vec<ItemElement>> {
        let sql_condition = match to_sql_form(&filters) {
            Ok(sq) => sq,
            Err(e) => {
                log_error!("Syntax error in {:?}", e);
                return Err(anyhow!("Impossible to parse the filters")); // TODO find a way to inform the client about the detail of the error
            }
        };

        let params = HashMap::new();

        let sql_query = format!(
            r"SELECT id, name, file_ref, created_gmt, last_modified_gmt
                    FROM cs_{0}.item INNER JOIN   cs_{0}.property prop
                    WHERE  ({1})
                    ORDER BY prop.col1 ",
            customer_code, sql_condition
        );

        let query = SQLQueryBlockAsync {
            sql_query,
            start: start_page.unwrap_or(0) * page_size.unwrap_or(0),
            length: page_size,
            params,
        };

        let mut _sql_result: SQLDataSet = query
            .execute(&mut trans)
            .await
            .map_err(err_fwd!("Query failed, [{}]", &query.sql_query))?;

        Ok(vec![])
    }

    /// ! Deprecated - user search_with_filter instead
    /// Search items by id
    /// If no item id provided, return all existing items
    /// TODO Merge the main query with the property query in order to reduce the number of SQL queries
    async fn search_item_by_id(
        &self,
        mut trans: &mut SQLTransactionAsync<'_>,
        item_id: Option<i64>,
        start_page: Option<u32>,
        page_size: Option<u32>,
        customer_code: &str,
    ) -> anyhow::Result<Vec<ItemElement>> {
        let p_item_id = CellValue::Int(item_id);

        let mut params = HashMap::new();
        params.insert("p_item_id".to_owned(), p_item_id);

        let sql_query = format!(
            r"SELECT id, name, file_ref, created_gmt, last_modified_gmt
                    FROM cs_{}.item
                    WHERE ( id = :p_item_id OR  :p_item_id IS NULL )
                    ORDER BY name ",
            customer_code
        );

        let query = SQLQueryBlockAsync {
            sql_query,
            start: start_page.unwrap_or(0) * page_size.unwrap_or(0),
            length: page_size,
            params,
        };

        let mut sql_result: SQLDataSet = query
            .execute(&mut trans)
            .await
            .map_err(err_fwd!("Query failed, [{}]", &query.sql_query))?;

        let mut items = vec![];
        while sql_result.next() {
            let id: i64 = sql_result.get_int("id").ok_or(anyhow!("Wring id"))?;
            let name: String = sql_result.get_string("name").unwrap_or("".to_owned());
            let o_file_ref: Option<String> = sql_result.get_string("file_ref"); // .unwrap_or("".to_owned());
            let created_gmt = sql_result
                .get_timestamp_as_datetime("created_gmt")
                .ok_or(anyhow::anyhow!("Wrong created gmt"))
                .map_err(tr_fwd!())?;

            // Optional
            let last_modified_gmt = sql_result
                .get_timestamp_as_datetime("last_modified_gmt")
                .as_ref()
                .map(|x| date_time_to_iso(x));

            let props = self
                .find_item_properties(trans, id, customer_code)
                .await
                .map_err(tr_fwd!())?;

            let item = ItemElement {
                item_id: id,
                name,
                file_ref: o_file_ref,
                created: date_time_to_iso(&created_gmt),
                last_modified: last_modified_gmt,
                properties: Some(props),
            };

            let _ = &items.push(item);
        }

        Ok(items)
    }

    ///
    ///
    ///
    async fn find_item_properties(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        item_id: i64,
        customer_code: &str,
    ) -> anyhow::Result<Vec<TagValueElement>> {
        let mut props = vec![];

        let p_item_id = CellValue::from_raw_int(item_id);

        let mut params = HashMap::new();
        params.insert("p_item_id".to_owned(), p_item_id);

        let sql_query = format!(
            r"SELECT td.name, td.type, tv.id, tv.tag_id, tv.item_id, tv.value_string, tv.value_integer, tv.value_double,
                tv.value_date, tv.value_datetime, tv.value_boolean
                FROM cs_{}.tag_value tv
                INNER JOIN cs_{}.tag_definition td ON td.id = tv.tag_id
                WHERE tv.item_id = :p_item_id ",
            customer_code, customer_code
        );

        let query = SQLQueryBlockAsync {
            sql_query,
            start: 0,
            length: None,
            params,
        };

        let mut sql_result = query
            .execute(trans)
            .await
            .map_err(err_fwd!("Query failed, [{}]", &query.sql_query))?;

        while sql_result.next() {
            let tag_name: String = sql_result.get_string("name").ok_or(anyhow!("Wrong name"))?;
            let tag_type: String = sql_result.get_string("type").ok_or(anyhow!("Wrong type"))?;

            let tag_id = sql_result
                .get_int("tag_id")
                .ok_or(anyhow!("Wrong tag_id"))?;
            let tag_value_id = sql_result
                .get_int("id")
                .ok_or(anyhow!("Wrong tag_value_id"))?;
            let row_item_id = sql_result
                .get_int("item_id")
                .ok_or(anyhow!("Wrong item id"))?;

            let r_value = match tag_type.to_lowercase().as_str() {
                TAG_TYPE_STRING => {
                    let value_string = sql_result.get_string("value_string");
                    Ok(EnumTagValue::String(value_string))
                }
                TAG_TYPE_LINK => {
                    let value_string = sql_result.get_string("value_string");
                    Ok(EnumTagValue::Link(value_string))
                }
                TAG_TYPE_BOOL => {
                    let value_boolean = sql_result.get_bool("value_boolean");
                    Ok(EnumTagValue::Boolean(value_boolean))
                }
                TAG_TYPE_INT => {
                    let value_integer = sql_result.get_int("value_integer");
                    Ok(EnumTagValue::Integer(value_integer))
                }
                TAG_TYPE_DOUBLE => {
                    let value_double = sql_result.get_double("value_double");
                    Ok(EnumTagValue::Double(value_double))
                }
                TAG_TYPE_DATE => {
                    // Changed to simply get the naive date and change it to iso string, no need of "Date"
                    let value_naivedate = sql_result.get_naivedate("value_date");
                    //let value_date = sql_result.get_naivedate_as_date("value_date");
                    let opt_iso_d_str = value_naivedate.as_ref().map(|x| naivedate_to_iso(x));
                    Ok(EnumTagValue::SimpleDate(opt_iso_d_str))
                }
                TAG_TYPE_DATETIME => {
                    let value_datetime = sql_result.get_timestamp_as_datetime("value_datetime");
                    let opt_iso_dt_str = value_datetime.as_ref().map(|x| date_time_to_iso(x));
                    Ok(EnumTagValue::DateTime(opt_iso_dt_str))
                }
                v => Err(anyhow!(format!("Wrong tag type, [{}]", v))),
            };

            let value = r_value.map_err(tr_fwd!())?;

            let tv = TagValueElement {
                tag_value_id,
                item_id: row_item_id,
                tag_id,
                tag_name,
                value,
            };
            let _ = &props.push(tv);
        }

        Ok(props)
    }

    ///
    /// ✨ Find an item from its item id
    ///
    pub async fn get_item(mut self, item_id: i64) -> WebType<GetItemReply> {
        // Done in the delegate constructor : self.follower.x_request_id = self.follower.x_request_id.new_if_null();

        log_info!(
            "🚀 Start get_item api, item_id=[{}], follower=[{}]",
            item_id,
            &self.follower
        );

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        // // Check if the token is valid
        // if !self.session_token.is_valid() {
        //     log_error!(
        //         "💣 Invalid session token=[{:?}], follower=[{}]",
        //         &self.session_token,
        //         &self.follower
        //     );
        //     return WebType::from_errorset(&INVALID_TOKEN);
        // }

        self.follower.token_type = TokenType::Sid(self.session_token.0.clone());

        // // Read the session information
        // let Ok(entry_session) = fetch_entry_session(&self.follower.token_type.value())
        //     .await
        //     .map_err(err_fwd!(
        //         "💣 Session Manager failed, follower=[{}]",
        //         &self.follower
        //     ))
        // else {
        //     return WebType::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        // };

        log_info!("😎 We fetched the session, follower=[{}]", &self.follower);

        // Query the item
        // | Open Db connection
        let Ok(mut cnx) = SQLConnectionAsync::from_pool().await.map_err(err_fwd!(
            "💣 New Db connection failed, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(mut trans) = cnx.begin().await.map_err(err_fwd!(
            "💣 Transaction issue, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(items) = self
            .search_item_by_id(
                &mut trans,
                Some(item_id),
                None,
                None,
                &entry_session.customer_code,
            )
            .await
            .map_err(err_fwd!(
                "💣 Cannot search item by id, follower=[{}]",
                &self.follower
            ))
        else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        if items.is_empty() {
            log_error!(
                "💣 Missing item=[{:?}], follower=[{}]",
                item_id,
                &self.follower
            );
            let wt = WebType::from_errorset(&MISSING_ITEM);
            return wt;
        }

        log_info!(
            "😎 We found the item, item count=[{}], follower=[{}]",
            items.len(),
            &self.follower
        );

        if trans
            .commit()
            .await
            .map_err(err_fwd!("💣 Commit failed"))
            .is_err()
        {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        log_info!("🏁 End get_item, follower=[{}]", &self.follower);
        WebType::from_item(StatusCode::OK.as_u16(), GetItemReply { items })
    }

    ///
    /// ✨ Delegate for delete_item_tag
    ///
    pub async fn delete_item_tag(
        mut self,
        item_id: i64,
        tag_names: Vec<String>,
    ) -> WebType<SimpleMessage> {
        log_info!(
            "🚀 Start delete_item_tag api, item_id=[{}], tag_names=[{:?}], follower=[{}]",
            item_id,
            &tag_names,
            &self.follower
        );

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        let customer_code = entry_session.customer_code.as_str();

        // // TODO use the correct routine and the try_or_return macro
        // let customer_code = &match self.valid_sid_get_session().await {
        //     Ok(cc) => cc,
        //     Err(e) => {
        //         return WebType::from_errorset(e);
        //     }
        // };

        log_info!(
            "😎 We read the session information, customer_code=[{}], follower=[{}]",
            customer_code,
            &self.follower
        );

        // Open Db connection
        let Ok(mut cnx) = SQLConnectionAsync::from_pool().await.map_err(err_fwd!(
            "💣 New Db connection failed, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(mut trans) = cnx.begin().await.map_err(err_fwd!(
            "💣 Transaction issue, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        // if tag_names.0.is_empty() {
        //     return WebType::from_errorset(&INVALID_REQUEST);
        // };

        //let item_id = add_item_tag_request.item_id;
        for tag_name in tag_names {
            if let Err(e) = self
                .delete_item_tag_value(&mut trans, item_id, &tag_name, customer_code)
                .await
            {
                log_error!(
                    "💣 Delete item tag value error, error=[{:?}], follower=[{}]",
                    e,
                    &self.follower
                );
                return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
            };
            log_info!(
                "😎 We deleted the tag, tag_name=[{}], follower=[{}]",
                &tag_name,
                &self.follower
            );
        }

        if trans
            .commit()
            .await
            .map_err(err_fwd!("💣 Commit failed"))
            .is_err()
        {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        log_info!("🏁 End delete_item_tag, follower=[{}]", &self.follower);
        WebType::from_item(
            StatusCode::OK.as_u16(),
            SimpleMessage {
                message: "Done".to_string(),
            },
        )
    }

    /// Delete a specific tag for the given item
    // fn delete_tag_for_item(&self, mut trans: &mut SQLTransaction, item_id: u64, tag: &AddTagValue, customer_code: &str) -> Result<(), ErrorSet<'static>> {
    //
    //     // Check the tag
    //     let tag_id = match (tag.tag_id, &tag.tag_name) {
    //         (None, None) => {
    //             // Impossible case, return an error.
    //             log_error!("💣 A tag must have a tag_id or a tag_name, follower=[{}]", &self.follower);
    //             return Err(INTERNAL_DATABASE_ERROR);
    //         }
    //         (Some(tag_id), None) => {
    //             // Tag id only, verify if the tag exists, return the tag_id
    //             // Any case with a tag_name provided, check / create, return the tag_id
    //             let Ok(tid) = self.check_tag_id_validity(&mut trans, tag_id, tag, customer_code)
    //                 .map_err(err_fwd!("💣 The definition of the new tag failed, tag name=[{:?}], follower=[{}]", tag, &self.follower)) else {
    //                 return Err(MISSING_TAG_FOR_ITEM);
    //             };
    //             log_info!("😎 The tag is already in the system, tag id=[{}], follower=[{}]", tid, &self.follower);
    //             tid
    //         }
    //         (_, Some(tag_name)) => {
    //             let session_token = self.session_token.clone();
    //             let x_request_id = self.follower.x_request_id.clone();
    //             let tag_delegate = TagDelegate::new(session_token, x_request_id);
    //             // Any case with a tag_name provided, check / create , return the tag_id
    //             let tag_id = match tag_delegate.search_tag_by_name(&mut trans, tag_name.as_str(), customer_code)
    //             {
    //                 Ok(tag) => {
    //                     // We found the tag by it's name
    //                     tag.tag_id
    //                 }
    //                 Err(_) => {
    //                     return Err(BAD_TAG_FOR_ITEM);
    //                 }
    //             };
    //
    //             log_info!("The tag is in the system, tag id=[{:?}], follower=[{}]", tag_id, &self.follower);
    //             tag_id
    //         }
    //     };
    //
    //     // Verify if the tag exists on the item
    //     match self.is_tags_on_item(&mut trans, item_id as i64, tag_id, customer_code) {
    //         Ok((o_tag_value_id, o_tag_type)) => {
    //
    //         }
    //         Err(e) => {
    //
    //         }
    //     }
    //
    //     // Remove the tag value
    //
    //     Ok(())
    // }

    ///
    /// ✨ Delegate for add_item_tag
    ///
    pub async fn update_item_tag(
        mut self,
        item_id: i64,
        add_item_tag_request: Json<AddItemTagRequest>,
    ) -> WebType<AddItemTagReply> {
        log_info!(
            "🚀 Start update_item_tag api, add_item_tag_request=[{:?}], follower=[{}]",
            &add_item_tag_request,
            &self.follower
        );

        // let customer_code = &match self.valid_sid_get_session() {
        //     Ok(cc) => cc,
        //     Err(e) => {
        //         return WebType::from_errorset(e);
        //     }
        // };

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        let customer_code = entry_session.customer_code.as_str();

        log_info!(
            "😎 We read the session information, customer_code=[{}], follower=[{}]",
            customer_code,
            &self.follower
        );

        // Open Db connection
        let Ok(mut cnx) = SQLConnectionAsync::from_pool().await.map_err(err_fwd!(
            "💣 New Db connection failed, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(mut trans) = cnx.begin().await.map_err(err_fwd!(
            "💣 Transaction issue, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        // Add the tags
        let r_add_tags = self
            .update_tags_on_item(
                &mut trans,
                item_id,
                customer_code,
                &add_item_tag_request.properties,
            )
            .await;
        if let Err(e) = r_add_tags {
            return WebType::from_errorset(&e);
        }

        if trans
            .commit()
            .await
            .map_err(err_fwd!("💣 Commit failed"))
            .is_err()
        {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        log_info!("🏁 End update_item_tag, follower=[{}]", &self.follower);

        WebType::from_item(
            StatusCode::OK.as_u16(),
            AddItemTagReply {
                status: "Ok".to_string(),
            },
        )
    }

    // // TODO deprecated
    // #[deprecated]
    // fn valid_sid_get_session(&mut self) -> Result<String, ErrorSet<'static>> {
    //     // Check if the token is valid
    //     if !self.session_token.is_valid() {
    //         log_error!(
    //             "💣 Invalid session token, token=[{:?}], follower=[{}]",
    //             &self.session_token,
    //             &self.follower
    //         );
    //         return Err(&INVALID_TOKEN);
    //         // return Json(AddItemReply::invalid_token_error_reply());
    //     }
    //
    //     self.follower.token_type = TokenType::Sid(self.session_token.0.clone());
    //
    //     // Read the session information
    //     let Ok(entry_session) = fetch_entry_session(&self.follower.token_type.value()).map_err(
    //         err_fwd!("💣 Session Manager failed, follower=[{}]", &self.follower),
    //     ) else {
    //         return Err(&INTERNAL_TECHNICAL_ERROR);
    //     };
    //     let customer_code = entry_session.customer_code.as_str();
    //     Ok(customer_code.to_owned())
    // }

    ///
    /// ✨ Create an item
    ///
    pub async fn add_item(
        mut self,
        add_item_request: Json<AddItemRequest>,
    ) -> WebType<AddItemReply> {
        log_info!(
            "🚀 Start add_item api, add_item_request=[{:?}], follower=[{}]",
            &add_item_request,
            &self.follower
        );

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        let customer_code = entry_session.customer_code.as_str();

        log_info!(
            "😎 We read the session information, customer_code=[{}], follower=[{}]",
            customer_code,
            &self.follower
        );

        // Open Db connection
        let Ok(mut cnx) = SQLConnectionAsync::from_pool().await.map_err(err_fwd!(
            "💣 New Db connection failed, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(mut trans) = cnx.begin().await.map_err(err_fwd!(
            "💣 Transaction issue, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let o_file_ref = add_item_request.file_ref.clone();
        let Ok(item_id) = self
            .create_item(
                &mut trans,
                &add_item_request.name,
                customer_code,
                o_file_ref,
            )
            .await
            .map_err(err_fwd!(
                "💣 Cannot create the item, follower=[{}]",
                &self.follower
            ))
        else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        log_info!(
            "😎 We created the item, item_id=[{}], follower=[{}]",
            item_id,
            &self.follower
        );

        // | Insert all the properties
        if let Some(properties) = &add_item_request.properties {
            if let Err(e) = self
                .update_tags_on_item(&mut trans, item_id, customer_code, properties)
                .await
            {
                return WebType::from_errorset(e);
            }
        }

        log_info!(
            "😎 We added all the properties to the item, item_id=[{}], follower=[{}]",
            item_id,
            &self.follower
        );

        if trans
            .commit()
            .await
            .map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))
            .is_err()
        {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        let now = SystemTime::now();
        let created: DateTime<Utc> = now.clone().into();
        let last_modified: DateTime<Utc> = now.clone().into();

        log_info!("🏁 End add_item, follower=[{}]", &self.follower);

        WebType::from_item(
            StatusCode::OK.as_u16(),
            AddItemReply {
                item_id,
                name: add_item_request.name.clone(),
                created: date_time_to_iso(&created),
                last_modified: Some(date_time_to_iso(&last_modified)),
            },
        )
    }

    /// Add tags on an item
    async fn update_tags_on_item(
        &self,
        mut trans: &mut SQLTransactionAsync<'_>,
        item_id: i64,
        customer_code: &str,
        properties: &Vec<AddTagValue>,
    ) -> Result<(), &ErrorSet<'static>> {
        for tag in properties {
            // Check / Define the property
            let tag_id = match (tag.tag_id, &tag.tag_name) {
                (None, None) => {
                    // Impossible case, return an error.
                    log_error!(
                        "💣 A tag must have a tag_id or a tag_name, follower=[{}]",
                        &self.follower
                    );
                    return Err(&INTERNAL_DATABASE_ERROR);
                }
                (Some(tag_id), None) => {
                    // Tag id only, verify if the tag exists, return the tag_id
                    // Any case with a tag_name provided, check / create, return the tag_id
                    let Ok(tid) = self.check_tag_id_validity(&mut trans, tag_id, tag, customer_code).await
                        .map_err(err_fwd!("💣 The definition of the new tag failed, tag name=[{:?}], follower=[{}]", tag, &self.follower)) else {
                        //let message = format!("tag_id: [{}]", &tag_id);
                        return Err(&MISSING_TAG_FOR_ITEM);
                    };
                    log_info!(
                        "😎 The tag is already in the system, tag id=[{}], follower=[{}]",
                        tid,
                        &self.follower
                    );
                    tid
                }
                (_, Some(_tag_name)) => {
                    // Any case with a tag_name provided, check / create , return the tag_id
                    let Ok(tid) = self
                        .define_tag_if_needed(&mut trans, tag, customer_code)
                        .await
                        .map_err(err_fwd!(
                        "💣 The definition of the new tag failed, tag name=[{:?}], follower=[{}]",
                        tag,
                        &self.follower
                    )) else {
                        //let message = format!("tag_name: [{}]", &tag_name);
                        return Err(&BAD_TAG_FOR_ITEM);
                    };
                    log_info!(
                        "The tag is in the system, tag id=[{:?}], follower=[{}]",
                        tid,
                        &self.follower
                    );
                    tid
                }
            };

            // Verify if the tag exists on the item
            match self
                .is_tags_on_item(&mut trans, item_id, tag_id, customer_code)
                .await
            {
                Ok((o_tag_value_id, o_tag_type)) => {
                    match (o_tag_value_id, o_tag_type) {
                        (None, None) => {
                            // The tag hasn't any value for the item, create the tag values for the item
                            let add_tag_value = AddTagValue {
                                tag_id: Some(tag_id),
                                tag_name: tag.tag_name.clone(),
                                value: tag.value.clone(),
                            };

                            if self.create_item_property(&mut trans, &add_tag_value, item_id, customer_code).await
                                .map_err(err_fwd!("💣 Insertion of a new tag value failed, tag value=[{:?}], follower=[{}]", tag, &self.follower)).is_err() {
                                return Err(&INTERNAL_DATABASE_ERROR);
                            }
                            log_info!(
                                "😎 We added the tag to the item, tag name=[{:?}], follower=[{}]",
                                tag.value,
                                &self.follower
                            );
                        }
                        (Some(tag_value_id), Some(tag_type)) => {
                            // The tag has a value for the item, change the tag values for the item
                            log_info!(
                                "The tag has an existing value id=[{}], follower=[{}]",
                                tag_value_id,
                                &self.follower
                            );

                            if Self::enum_tag_value_to_tag_type(&tag) != tag_type {
                                log_error!("💣 Trying to change the tag value to different type : final type=[{:?}], original type=[{}], item id=[{}], tag_id=[{}], follower=[{}]"
                                    , tag.value, &tag_type, item_id, tag_id, &self.follower);
                                return Err(&INCORRECT_TAG_TYPE);
                            }

                            let add_tag_value = AddTagValue {
                                tag_id: Some(tag_id),
                                tag_name: tag.tag_name.clone(),
                                value: tag.value.clone(),
                            };

                            if self.change_item_tag_value(&mut trans, &add_tag_value, tag_value_id, customer_code).await
                                .map_err(err_fwd!("💣 Change of tag value failed, tag value=[{:?}], follower=[{}]", tag, &self.follower)).is_err() {
                                return Err(&INTERNAL_DATABASE_ERROR);
                            }
                            log_info!("😎 We changed the tag value of the item, tag name=[{:?}], follower=[{}]", tag.value, &self.follower);
                        }
                        (_, _) => {
                            log_error!("💣 Error while checking the existing the tag value, item id=[{}], tag_id=[{}], follower=[{}]", item_id, tag_id, &self.follower);
                            return Err(&INTERNAL_DATABASE_ERROR);
                        }
                    }
                }
                Err(e) => {
                    log_error!("💣 Error while reading the tag value, item id=[{}], tag_id=[{}], message=[{}], follower=[{}]", item_id, tag_id, e.to_string(), &self.follower);
                    return Err(&INTERNAL_DATABASE_ERROR);
                }
            }
        }
        Ok(())
    }

    ///
    async fn change_item_tag_value(
        &self,
        mut trans: &mut SQLTransactionAsync<'_>,
        tag: &AddTagValue,
        tag_value_id: i64,
        customer_code: &str,
    ) -> anyhow::Result<()> {
        let sql_update = format!(
            r"UPDATE cs_{0}.tag_value
                                            SET
                                                value_boolean = :p_value_boolean,
                                                value_string = :p_value_string,
                                                value_integer = :p_value_integer,
                                                value_double = :p_value_double,
                                                value_date = :p_val_date,
                                                value_datetime = :p_value_datetime
                                            WHERE id = :p_tag_value_id
                                                 ",
            customer_code
        );

        let mut params = HashMap::new();

        params.insert(
            "p_tag_value_id".to_string(),
            CellValue::from_raw_int(tag_value_id),
        );
        params = self.build_params_for_insert_and_update(&tag, params);
        let query = SQLChangeAsync {
            sql_query: sql_update.to_string(),
            params,
            sequence_name: "".to_string(),
        };
        let _id = query.update(&mut trans).await.map_err(err_fwd!(
            "💣 Query failed, [{}], , follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;
        Ok(())
    }

    ///
    async fn delete_item_tag_value(
        &self,
        mut trans: &mut SQLTransactionAsync<'_>,
        item_id: i64,
        tag_name: &str,
        customer_code: &str,
    ) -> anyhow::Result<()> {
        let sql_delete = format!(
            r"DELETE FROM cs_{0}.tag_value
                                            WHERE tag_id = (SELECT td.id FROM cs_{0}.tag_definition td WHERE td.name = :p_tag_name)
                                            AND item_id = :p_item_id
                                                 ",
            customer_code
        );

        let mut params = HashMap::new();

        params.insert("p_item_id".to_string(), CellValue::from_raw_int(item_id));
        params.insert("p_tag_name".to_string(), CellValue::from_raw_str(tag_name));
        let query = SQLChangeAsync {
            sql_query: sql_delete.to_string(),
            params,
            sequence_name: "".to_string(),
        };

        let _id = query.delete(&mut trans).await.map_err(err_fwd!(
            "💣 Query failed, [{}], , follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;
        Ok(())
    }

    /// find if the tag is already assigned to the item
    async fn is_tags_on_item(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        item_id: i64,
        tag_id: i64,
        customer_code: &str,
    ) -> anyhow::Result<(Option<i64>, Option<String>)> {
        let sql_query = format!(
            r#"SELECT tv.id, td.type FROM
                                            cs_{0}.tag_value tv INNER JOIN cs_{0}.tag_definition td ON td.id = tv.tag_id
                                        WHERE tv.tag_id = :p_tag_id
                                            AND tv.item_id = :p_item_id"#,
            &customer_code
        );

        let mut params = HashMap::new();
        params.insert("p_tag_id".to_string(), CellValue::from_raw_int(tag_id));
        params.insert("p_item_id".to_string(), CellValue::from_raw_int(item_id));

        let query = SQLQueryBlockAsync {
            sql_query,
            start: 0,
            length: None,
            params,
        };

        let mut sql_result: SQLDataSet = query.execute(trans).await.map_err(err_fwd!(
            "Query failed, [{}], , follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;

        let (tag_value_id, tag_type) = if sql_result.next() {
            (sql_result.get_int("id"), sql_result.get_string("type"))
        } else {
            (None, None)
        };
        Ok((tag_value_id, tag_type))
    }

    ///
    async fn create_item(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        item_name: &str,
        customer_code: &str,
        file_ref: Option<String>,
    ) -> anyhow::Result<i64> {
        let sql_query = format!(
            r"INSERT INTO cs_{}.item(name, created_gmt, last_modified_gmt, file_ref)
                                        VALUES (:p_name, :p_created, :p_last_modified, :p_file_ref)",
            customer_code
        );

        let sequence_name = format!("cs_{}.item_id_seq", customer_code);

        let now = SystemTime::now();
        let mut params = HashMap::new();
        params.insert(
            "p_name".to_string(),
            CellValue::from_raw_string(item_name.to_string()),
        );
        params.insert(
            "p_created".to_string(),
            CellValue::from_raw_systemtime(now.clone()),
        );
        params.insert(
            "p_last_modified".to_string(),
            CellValue::from_raw_systemtime(now.clone()),
        );
        params.insert("p_file_ref".to_string(), CellValue::String(file_ref));

        let sql_insert = SQLChangeAsync {
            sql_query,
            params,
            sequence_name,
        };

        let item_id = sql_insert.insert(trans).await.map_err(err_fwd!(
            "Insertion of a new item failed, follower=[{}]",
            &self.follower
        ))?;

        log_info!("Created item : item_id=[{}]", item_id);
        Ok(item_id)
    }

    /// Ensure the tag_id exists
    async fn check_tag_id_validity(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        tag_id: i64,
        prop: &AddTagValue,
        customer_code: &str,
    ) -> anyhow::Result<i64> {
        // Find tag by name
        let session_token = self.session_token.clone();
        let x_request_id = self.follower.x_request_id.clone();
        let tag_delegate = TagDelegate::new(session_token, x_request_id);

        let tags = tag_delegate
            .search_tag_by_id(trans, Some(tag_id), None, None, customer_code)
            .await
            .map_err(err_fwd!(
                "Tag not found, tag_id=[{}], follower=[{}]",
                tag_id,
                &self.follower
            ))?;

        if tags.is_empty() {
            return Err(anyhow!(
                "Tag not found, tag_id=[{}], follower=[{}]",
                tag_id,
                &self.follower
            ));
        };

        let tag = tags.get(0).ok_or(anyhow!("Missing tag element"))?;
        if tag.tag_type != Self::enum_tag_value_to_tag_type(&prop) {
            return Err(anyhow!(
                "Tag has a different value type than its definition, tag_id=[{}], follower=[{}]",
                tag_id,
                &self.follower
            ));
        }

        log_info!(
            "Tag is valid, tag_id=[{}], follower=[{}]",
            tag_id,
            &self.follower
        );

        Ok(tag_id)
    }

    ///
    async fn define_tag_if_needed(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        prop: &AddTagValue,
        customer_code: &str,
    ) -> anyhow::Result<i64> {
        let Some(tag_name) = &prop.tag_name else {
            return Err(anyhow!(
                "Tag name cannot be empty, follower=[{}]",
                &self.follower
            ));
        };

        // Find tag by name
        let session_token = self.session_token.clone();
        let x_request_id = self.follower.x_request_id.clone();
        let tag_delegate = TagDelegate::new(session_token, x_request_id);
        let tag_id = match tag_delegate
            .search_tag_by_name(trans, tag_name.as_str(), customer_code)
            .await
        {
            Ok(tag) => {
                // We found the tag by it's name
                tag.tag_id
            }
            Err(_) => {
                // We did not find the tag, we simply create a new one
                let add_tag_request = AddTagRequest {
                    name: tag_name.clone(),
                    tag_type: Self::enum_tag_value_to_tag_type(&prop),
                    default_value: None,
                };

                if let Err(err) = tag_delegate.check_input_values(&add_tag_request) {
                    log_error!("Tag definition is not correct, tag_name=[{}], err message=[{}], follower=[{}]",
                                                            tag_name,  err.err_message, &self.follower);
                    return Err(anyhow!("Tag definition is not correct"));
                }

                let tag_id = tag_delegate
                    .insert_tag_definition(trans, &add_tag_request, customer_code)
                    .await
                    .map_err(tr_fwd!())?;
                log_info!(
                    "😎 Defined the tag, tag_id=[{}], follower=[{}]",
                    tag_id,
                    &self.follower
                );
                tag_id
            }
        };

        Ok(tag_id)
    }

    fn enum_tag_value_to_tag_type(prop: &AddTagValue) -> String {
        match prop.value {
            EnumTagValue::String(_) => TAG_TYPE_STRING,
            EnumTagValue::Boolean(_) => TAG_TYPE_BOOL,
            EnumTagValue::Integer(_) => TAG_TYPE_INT,
            EnumTagValue::Double(_) => TAG_TYPE_DOUBLE,
            EnumTagValue::SimpleDate(_) => TAG_TYPE_DATE,
            EnumTagValue::DateTime(_) => TAG_TYPE_DATETIME,
            EnumTagValue::Link(_) => TAG_TYPE_LINK,
        }
        .to_string()
    }

    ///
    async fn create_item_property(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        tag: &AddTagValue,
        item_id: i64,
        customer_code: &str,
    ) -> anyhow::Result<()> {
        let tag_id = tag.tag_id.ok_or(anyhow!(
            "Tag id must be provided, follower=[{}]",
            &self.follower
        ))?;

        // FIXME BUG: we named the variable :p_val_date because otherwise it conflict with :p_value_datetime
        //              the replacement expression should be ":variable:" to avoid this case
        let sql_query = format!(
            r"INSERT INTO cs_{}.tag_value (tag_id, item_id, value_boolean, value_string, value_integer, value_double, value_date, value_datetime)
                 VALUES (:p_tag_id, :p_item_id, :p_value_boolean, :p_value_string, :p_value_integer, :p_value_double, :p_val_date, :p_value_datetime) ",
            customer_code
        );

        let mut params = HashMap::new();

        params.insert("p_tag_id".to_string(), CellValue::from_raw_int(tag_id));
        params.insert("p_item_id".to_string(), CellValue::from_raw_int(item_id));

        params = self.build_params_for_insert_and_update(&tag, params);

        let sql_insert = SQLChangeAsync {
            sql_query,
            params,
            sequence_name: format!("cs_{}.tag_value_id_seq", customer_code),
        };

        log_debug!(
            "Created the property, prop tag id=[{}], follower=[{}]",
            tag_id,
            &self.follower
        );

        let _ = sql_insert.insert(trans).await.map_err(err_fwd!(
            "Cannot insert the tag value, follower=[{}]",
            &self.follower
        ))?;
        Ok(())
    }

    fn build_params_for_insert_and_update(
        &self,
        tag: &AddTagValue,
        mut params: HashMap<String, CellValue>,
    ) -> HashMap<String, CellValue> {
        params.insert("p_value_string".to_string(), CellValue::String(None));
        params.insert("p_value_boolean".to_string(), CellValue::Bool(None));
        params.insert("p_value_integer".to_string(), CellValue::Int(None));
        params.insert("p_value_double".to_string(), CellValue::Double(None));
        params.insert("p_val_date".to_string(), CellValue::Date(None));
        params.insert("p_value_datetime".to_string(), CellValue::SystemTime(None));

        match &tag.value {
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
                    let nd = iso_to_naivedate(d_string)
                        .map_err(err_fwd!(
                            "Cannot convert the string to datetime:[{}], follower=[{}]",
                            d_string,
                            &self.follower
                        ))
                        .ok()?;
                    Some(nd)
                })();
                params.insert("p_val_date".to_string(), CellValue::Date(opt_st));
            }
            EnumTagValue::DateTime(tv) => {
                let opt_st = (|| {
                    let dt_string = tv.as_ref()?;
                    let dt = iso_to_datetime(dt_string)
                        .map_err(err_fwd!(
                            "Cannot convert the string to datetime:[{}], follower=[{}]",
                            dt_string,
                            &self.follower
                        ))
                        .ok()?;
                    Some(SystemTime::from(dt))
                })();
                params.insert(
                    "p_value_datetime".to_string(),
                    CellValue::SystemTime(opt_st),
                );
            }
            EnumTagValue::Link(tv) => {
                params.insert("p_value_string".to_string(), CellValue::String(tv.clone()));
            }
        }
        params
    }

    fn web_type_error<T>() -> impl Fn(&ErrorSet<'static>) -> WebType<T>
    where
        T: DeserializeOwned,
    {
        |e| {
            log_error!("💣 Error after try {:?}", e);
            WebType::from_errorset(e)
        }
    }
}
