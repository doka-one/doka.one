use log::*;
use std::collections::HashMap;
use std::time::SystemTime;
use anyhow::anyhow;
use chrono::Utc;
use rocket::http::RawStr;
use rocket_contrib::json::Json;
use commons_error::*;
use commons_pg::{CellValue, SQLChange, SQLConnection, SQLDataSet, SQLQueryBlock, SQLTransaction};
use commons_services::token_lib::SecurityToken;
use commons_services::x_request_id::{Follower, XRequestID};
use dkdto::{EntrySession, JsonErrorSet, OpenSessionReply, OpenSessionRequest, SessionReply};
use dkdto::error_codes::{INVALID_REQUEST, SESSION_CANNOT_BE_RENEWED, SESSION_NOT_FOUND, SESSION_TIMED_OUT, SUCCESS};
use dkdto::error_replies::ErrorReply;
use doka_cli::request_client::TokenType;

pub(crate) struct SessionDelegate {
    pub security_token: SecurityToken,
    pub follower: Follower,
}

impl SessionDelegate {
    pub fn new(security_token: SecurityToken, x_request_id: XRequestID) -> Self {
        Self {
            security_token,
            follower: Follower {
                x_request_id: x_request_id.new_if_null(),
                token_type: TokenType::None,
            }
        }
    }


    ///
    /// 🔑 Open a new session for the group and user
    ///
    /// It's usually called by the Login end point using the session_id as a security_token
    ///
    pub fn open_session(&mut self, session_request: Json<OpenSessionRequest>) -> Json<OpenSessionReply> {

        log_info!("🚀 Start open_session api, follower=[{}]", &self.follower);
        log_debug!("session_request=[{:?}], follower=[{}]", &session_request, &self.follower);

        // Check if the token is valid
        if !self.security_token.is_valid() {
            log_error!("💣 Invalid security token, token=[{:?}], follower=[{}]", &self.security_token, &self.follower);
            return Json(OpenSessionReply::invalid_token_error_reply());
        }

        self.follower.token_type = TokenType::Sid(self.security_token.0.clone());

        let internal_database_error_reply = Json(OpenSessionReply::internal_database_error_reply());

        let Ok(mut cnx) = SQLConnection::new().map_err(err_fwd!("💣 Connection issue, follower=[{}]", &self.follower)) else {
            return internal_database_error_reply;
        };

        let r_trans = cnx.sql_transaction().map_err(err_fwd!("💣 Transaction issue, follower=[{}]", &self.follower));
        let Ok(mut trans) = r_trans else {
            return internal_database_error_reply;
        };

        let sql_insert = r#"INSERT INTO dokasys.SESSIONS
                            (customer_code, customer_id, user_name, user_id, session_id, start_time_gmt)
                            VALUES (:p_customer_code, :p_customer_id, :p_user_name, :p_user_id, :p_session_id, :p_start_time_gmt)"#;

        let current_datetime = SystemTime::now();
        let session_id = session_request.session_id.to_owned();

        let mut params : HashMap<String, commons_pg::CellValue> = HashMap::new();
        params.insert("p_customer_code".to_owned(), CellValue::from_raw_string(session_request.customer_code.to_owned()));
        params.insert("p_customer_id".to_owned(), CellValue::from_raw_int(session_request.customer_id));
        params.insert("p_user_name".to_owned(), CellValue::from_raw_string(session_request.user_name.to_owned()));
        params.insert("p_user_id".to_owned(), CellValue::from_raw_int(session_request.user_id));
        params.insert("p_session_id".to_owned(), CellValue::from_raw_string(session_id.clone()));
        params.insert("p_start_time_gmt".to_owned(), CellValue::from_raw_systemtime(current_datetime));

        let query = SQLChange {
            sql_query :  sql_insert.to_string(),
            params,
            sequence_name : "dokasys.sessions_id_seq".to_string(),
        };

        let Ok(session_db_id) = query.insert(&mut trans)
                            .map_err( err_fwd!("💣 Cannot insert the session, follower=[{}]", &self.follower)) else {
            return internal_database_error_reply;
        };

        if trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower)).is_err() {
            return internal_database_error_reply;
        }

        log_info!("😎 Session was opened with success, session_db_id=[{}], follower=[{}]", session_db_id, &self.follower);

        let ret = OpenSessionReply {
            session_id,
            status : JsonErrorSet::from(SUCCESS),
        };
        log_info!("🏁 End open_session, follower=[{}]", &self.follower);
        Json(ret)
    }


    ///
    /// 🔑 Find a session from its session id
    ///
    pub fn read_session(&mut self, session_id: &RawStr) -> Json<SessionReply> {

        log_info!("🚀 Start read_session api, follower=[{}]", &self.follower);

        // Check if the token is valid
        if ! self.security_token.is_valid() {
            log_error!("💣 Invalid security token, token=[{:?}], follower=[{}]", &self.security_token, &self.follower);
            return Json(SessionReply::invalid_token_error_reply());
        }
        self.follower.token_type = TokenType::Sid(self.security_token.0.clone());

        let Ok(session_id) =  session_id.percent_decode().map_err(err_fwd!("💣 Invalid input parameter, [{}]", session_id) ) else {
            return Json(SessionReply::from_error(INVALID_REQUEST) );
        };

        // Open Db connection
        let internal_database_error_reply = Json(SessionReply::internal_database_error_reply());

        let Ok(mut cnx) = SQLConnection::new().map_err(err_fwd!("💣 New Db connection failed, follower=[{}]", &self.follower)) else {
            return internal_database_error_reply;
        };

        let r_trans = cnx.sql_transaction().map_err(err_fwd!("💣 Error transaction, follower=[{}]", &self.follower));
        let Ok(mut trans) = r_trans else {
            return internal_database_error_reply;
        };

        // Query the sessions to find the right one
        let Ok(sessions) =  self.search_session_by_sid(&mut trans, Some(&session_id))
                .map_err(err_fwd!("💣 Session search failed for session id=[{}], follower=[{}]", session_id, &self.follower)) else {
            return internal_database_error_reply;
        };

        log_info!("😎 Found the session information, number of sessions=[{}], follower=[{}]", sessions.len(), &self.follower);

        // Customer key to return
        let mut session_reply = SessionReply{ sessions, status: JsonErrorSet::from(SUCCESS) };

        // Check if the session was found
        if session_reply.sessions.is_empty() {
            log_warn!("⛔ The session was not found, follower=[{}]", &self.follower);
            return Json(SessionReply { sessions : vec![], status: JsonErrorSet::from(SESSION_NOT_FOUND) } )
        }

        let Ok(session) = session_reply.sessions.get_mut(0)
                    .ok_or(anyhow!("Wrong index 0"))
                    .map_err(err_fwd!("💣 Cannot find the session in the list of sessions, follower=[{}]", &self.follower)) else {
            return Json(SessionReply { sessions : vec![], status: JsonErrorSet::from(SESSION_NOT_FOUND) } );
        };

        // If the termination time exists, it means the session is closed
        if session.termination_time_gmt.is_some() {
            log_warn!("⛔ The session is closed. Closing time =[{}], follower=[{}]", &session.termination_time_gmt.as_ref().unwrap(), &self.follower);
            return Json(SessionReply { sessions : vec![], status: JsonErrorSet::from(SESSION_TIMED_OUT) } )
        }

        // Update the session renew_time_gmt
        let r_update = self.update_renew_time(&mut trans, &session_id);

        if r_update.is_err() {
            trans.rollback();
            log_warn!("💣 Rollback. Cannot update the renew time of the session, follower=[{}]", &self.follower);
            return Json(SessionReply { sessions : vec![], status: JsonErrorSet::from(SESSION_CANNOT_BE_RENEWED) } )
        }

        session.renew_time_gmt = Some(Utc::now().to_string());

        // End the transaction
        if trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower)).is_err() {
            return internal_database_error_reply;
        }

        log_info!("😎 Updated the session renew timestamp, session id=[{}], follower=[{}]", &session_id, &self.follower);

        log_info!("🏁 End read_session api, follower=[{}]", &self.follower);

        Json(session_reply)

    }


    /// Search the session information from the session id
    fn search_session_by_sid(&self, mut trans : &mut SQLTransaction, session_id: Option<&str>) -> anyhow::Result<Vec<EntrySession>> {
        let p_sid = CellValue::from_opt_str(session_id);

        let mut params = HashMap::new();
        params.insert("p_sid".to_owned(), p_sid);

        let query = SQLQueryBlock {
            sql_query : r"SELECT id, customer_code, customer_id, user_name, user_id, session_id, start_time_gmt, renew_time_gmt, termination_time_gmt
                    FROM dokasys.sessions
                    WHERE session_id = :p_sid OR :p_sid IS NULL ".to_string(),
            start : 0,
            length : Some(1),
            params,
        };

        let mut sql_result : SQLDataSet =  query.execute(&mut trans).map_err(err_fwd!("Query failed, [{}], follower=[{}]", &query.sql_query, &self.follower))?;

        let mut sessions = vec![];
        while sql_result.next() {
            let id = sql_result.get_int("id").ok_or(anyhow!("Wrong column id"))?;
            let customer_code: String = sql_result.get_string("customer_code").ok_or(anyhow!("Wrong column customer_code"))?;
            let customer_id: i64 = sql_result.get_int("customer_id").ok_or(anyhow!("Wrong column customer_id"))?;
            let user_name: String = sql_result.get_string("user_name").ok_or(anyhow!("Wrong column user_name"))?;
            let user_id: i64 = sql_result.get_int("user_id").ok_or(anyhow!("Wrong column user_id"))?;
            let session_id: String = sql_result.get_string("session_id").ok_or(anyhow!("Wrong column session_id"))?;
            let start_time_gmt  = sql_result.get_timestamp_as_datetime("start_time_gmt")
                .ok_or(anyhow::anyhow!("Wrong column start_time_gmt"))
                .map_err(err_fwd!("Cannot read the start time"))?;

            // Optional
            let renew_time_gmt = sql_result.get_timestamp_as_datetime("renew_time_gmt")
                    .as_ref()
                    .map( |x| x.to_string() );
            // Optional
            let termination_time_gmt = sql_result.get_timestamp_as_datetime("termination_time_gmt")
                    .as_ref()
                    .map( |x| x.to_string() );

            let session_info = EntrySession {
                id,
                customer_code,
                user_name,
                customer_id,
                user_id,
                session_id,
                start_time_gmt : start_time_gmt.to_string(),
                renew_time_gmt,
                termination_time_gmt,
            };

            let _ = &sessions.push(session_info);

        }

        Ok(sessions)
    }

    /// Set the renew timestamp of the session to the current UTC time.
    fn update_renew_time(&self, mut trans : &mut SQLTransaction, session_id: &str) -> anyhow::Result<bool> {
        let p_sid = CellValue::from_raw_string(session_id.to_owned());

        let mut params = HashMap::new();
        params.insert("p_session_id".to_owned(), p_sid);

        let sql_update = r#"UPDATE dokasys.SESSIONS
                             SET renew_time_gmt = ( NOW() at time zone 'UTC'  )
                             WHERE session_id = :p_session_id "#;

        let query = SQLChange {
            sql_query :  sql_update.to_string(),
            params,
            sequence_name : "".to_string(),
        };

        let _ = query.update(&mut trans).map_err( err_fwd!("Cannot update the session renew timestamp, follower=[{}]", &self.follower))?;
        Ok(true)
    }
}