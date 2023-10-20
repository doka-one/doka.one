use std::collections::HashMap;
use std::thread;
use std::time::Duration;

use log::*;
use rocket::http::Status;

use commons_error::*;
use commons_pg::{CellValue, SQLChange, SQLConnection};
use dkdto::{WebType, WebTypeBuilder};
use dkdto::error_codes::INTERNAL_DATABASE_ERROR;

pub(crate) struct DbPoolDelegate {
    // pub security_token: SecurityToken,
    // pub follower: Follower,
}

impl DbPoolDelegate {
    pub fn new(/*security_token: SecurityToken, x_request_id: XRequestID*/) -> Self {
        Self {
            // security_token,
            // follower: Follower {
            //     x_request_id: x_request_id.new_if_null(),
            //     token_type: TokenType::None,
            // }
        }
    }

    ///
    /// 🔑 Grab...
    ///
    /// It's usually ...
    ///
    pub fn grab_ctx(&self, duration: u32) -> WebType<String> {

        log_info!("🚀 Start grab ctx api, duration=[{}]", duration);

        let Ok(mut cnx) = SQLConnection::new().map_err(err_fwd!("💣 Connection issue, duration=[{}]", duration)) else {
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        };

        let r_trans = cnx.sql_transaction().map_err(err_fwd!("💣 Transaction issue, duration=[{}]", duration));
        let Ok(mut trans) = r_trans else {
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        };

        let sql_insert = r#"INSERT INTO public.connection_history
                            (timer, description, status)
                            VALUES (now(), :p_description, :p_status)"#;

        let mut params : HashMap<String, CellValue> = HashMap::new();
        params.insert("p_description".to_owned(), CellValue::from_raw_str(&format!("DESC_{}", duration)));
        params.insert("p_status".to_owned(), CellValue::from_raw_str("OPEN"));

        let query = SQLChange {
            sql_query :  sql_insert.to_string(),
            params,
            sequence_name : "public.connection_history_id_seq".to_string(),
        };

        let Ok(session_db_id) = query.insert(&mut trans)
                            .map_err( err_fwd!("💣 Cannot insert the session, duration=[{}]", duration)) else {
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        };

        thread::sleep(Duration::from_secs(30));

        if trans.commit().map_err(err_fwd!("💣 Commit failed, duration=[{}]", duration)).is_err() {
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        }

        log_info!("😎 Connection was grabbed with success, session_db_id=[{}], duration=[{}]", session_db_id, duration);

        log_info!("🏁 End grab ctx, duration=[{}]", duration);
        WebType::from_item(Status::Accepted.code, "OK".to_string())
    }

    // /// Search the session information from the session id
    // fn search_session_by_sid(&self, mut trans : &mut SQLTransaction, session_id: Option<&str>) -> anyhow::Result<Vec<EntrySession>> {
    //     let p_sid = CellValue::from_opt_str(session_id);
    //
    //     let mut params = HashMap::new();
    //     params.insert("p_sid".to_owned(), p_sid);
    //
    //     let query = SQLQueryBlock {
    //         sql_query : r"SELECT id, customer_code, customer_id, user_name, user_id, session_id, start_time_gmt, renew_time_gmt, termination_time_gmt
    //                 FROM dokasys.sessions
    //                 WHERE session_id = :p_sid OR :p_sid IS NULL ".to_string(),
    //         start : 0,
    //         length : Some(1),
    //         params,
    //     };
    //
    //     let mut sql_result : SQLDataSet =  query.execute(&mut trans).map_err(err_fwd!("Query failed, [{}], follower=[{}]", &query.sql_query, &self.follower))?;
    //
    //     let mut sessions = vec![];
    //     while sql_result.next() {
    //         let id = sql_result.get_int("id").ok_or(anyhow!("Wrong column id"))?;
    //         let customer_code: String = sql_result.get_string("customer_code").ok_or(anyhow!("Wrong column customer_code"))?;
    //         let customer_id: i64 = sql_result.get_int("customer_id").ok_or(anyhow!("Wrong column customer_id"))?;
    //         let user_name: String = sql_result.get_string("user_name").ok_or(anyhow!("Wrong column user_name"))?;
    //         let user_id: i64 = sql_result.get_int("user_id").ok_or(anyhow!("Wrong column user_id"))?;
    //         let session_id: String = sql_result.get_string("session_id").ok_or(anyhow!("Wrong column session_id"))?;
    //         let start_time_gmt  = sql_result.get_timestamp_as_datetime("start_time_gmt")
    //             .ok_or(anyhow::anyhow!("Wrong column start_time_gmt"))
    //             .map_err(err_fwd!("Cannot read the start time"))?;
    //
    //         // Optional
    //         let renew_time_gmt = sql_result.get_timestamp_as_datetime("renew_time_gmt")
    //                 .as_ref()
    //                 .map( |x| x.to_string() );
    //         // Optional
    //         let termination_time_gmt = sql_result.get_timestamp_as_datetime("termination_time_gmt")
    //                 .as_ref()
    //                 .map( |x| x.to_string() );
    //
    //         let session_info = EntrySession {
    //             id,
    //             customer_code,
    //             user_name,
    //             customer_id,
    //             user_id,
    //             session_id,
    //             start_time_gmt : start_time_gmt.to_string(),
    //             renew_time_gmt,
    //             termination_time_gmt,
    //         };
    //
    //         let _ = &sessions.push(session_info);
    //
    //     }
    //
    //     Ok(sessions)
    // }
    //
    // /// Set the renew timestamp of the session to the current UTC time.
    // fn update_renew_time(&self, mut trans : &mut SQLTransaction, session_id: &str) -> anyhow::Result<bool> {
    //     let p_sid = CellValue::from_raw_string(session_id.to_owned());
    //
    //     let mut params = HashMap::new();
    //     params.insert("p_session_id".to_owned(), p_sid);
    //
    //     let sql_update = r#"UPDATE dokasys.SESSIONS
    //                          SET renew_time_gmt = ( NOW() at time zone 'UTC'  )
    //                          WHERE session_id = :p_session_id "#;
    //
    //     let query = SQLChange {
    //         sql_query :  sql_update.to_string(),
    //         params,
    //         sequence_name : "".to_string(),
    //     };
    //
    //     let _ = query.update(&mut trans).map_err( err_fwd!("Cannot update the session renew timestamp, follower=[{}]", &self.follower))?;
    //     Ok(true)
    // }
}
