use std::collections::HashMap;
use std::time::SystemTime;

use anyhow::anyhow;
use axum::http::StatusCode;
use axum::Json;
use chrono::Utc;
use log::*;

use commons_error::*;
use commons_pg::sql_transaction::{CellValue, SQLDataSet};
use commons_pg::sql_transaction2::{SQLChange2, SQLConnection2, SQLQueryBlock2, SQLTransaction2};
use commons_services::database_lib::run_blocking_spawn;
use commons_services::token_lib::SecurityToken;
use commons_services::try_or_return;
use commons_services::x_request_id::{Follower, XRequestID};
use dkdto::error_codes::{
    INTERNAL_DATABASE_ERROR, INVALID_TOKEN, SESSION_CANNOT_BE_RENEWED, SESSION_NOT_FOUND,
    SESSION_TIMED_OUT,
};
use dkdto::{
    EntrySession, OpenSessionReply, OpenSessionRequest, SessionReply, WebResponse, WebType,
    WebTypeBuilder,
};
use doka_cli::request_client::TokenType;

#[derive(Debug, Clone)]
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
            },
        }
    }

    /// 🔑 Open a new session for the group and user
    ///
    /// It's usually called by the Login end point using the session_id as a security_token
    pub async fn open_session(
        &mut self,
        session_request: Json<OpenSessionRequest>,
    ) -> WebType<OpenSessionReply> {
        log_info!("🚀 Start open_session api, follower=[{}]", &self.follower);
        log_debug!(
            "session_request=[{:?}], follower=[{}]",
            &session_request,
            &self.follower
        );

        // Check if the token is valid
        if !self.security_token.is_valid() {
            log_error!(
                "💣 Invalid security token, token=[{:?}], follower=[{}]",
                &self.security_token,
                &self.follower
            );
            return WebType::from_errorset(&INVALID_TOKEN);
        }

        self.follower.token_type = TokenType::Token(self.security_token.0.clone());

        let session_id = try_or_return!(self.create_new_session(&session_request).await, |e| {
            WebType::from(e)
        });

        let ret = OpenSessionReply { session_id };
        log_info!("🏁 End open_session, follower=[{}]", &self.follower);
        WebType::from_item(StatusCode::ACCEPTED.as_u16(), ret)
    }

    async fn create_new_session(
        &self,
        session_request: &OpenSessionRequest,
    ) -> WebResponse<String> {
        let Ok(mut cnx) = SQLConnection2::from_pool().await.map_err(err_fwd!(
            "💣 Connection issue, follower=[{}]",
            &self.follower
        )) else {
            return WebResponse::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(mut trans) = cnx.begin().await.map_err(err_fwd!(
            "💣 Transaction issue, follower=[{}]",
            &self.follower
        )) else {
            return WebResponse::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let sql_insert = r#"INSERT INTO dokasys.SESSIONS
                            (customer_code, customer_id, user_name, user_id, session_id, start_time_gmt)
                            VALUES (:p_customer_code, :p_customer_id, :p_user_name, :p_user_id, :p_session_id, :p_start_time_gmt)"#;

        let current_datetime = SystemTime::now();
        let session_id = session_request.session_id.to_owned();

        let mut params: HashMap<String, CellValue> = HashMap::new();
        params.insert(
            "p_customer_code".to_owned(),
            CellValue::from_raw_string(session_request.customer_code.to_owned()),
        );
        params.insert(
            "p_customer_id".to_owned(),
            CellValue::from_raw_int(session_request.customer_id),
        );
        params.insert(
            "p_user_name".to_owned(),
            CellValue::from_raw_string(session_request.user_name.to_owned()),
        );
        params.insert(
            "p_user_id".to_owned(),
            CellValue::from_raw_int(session_request.user_id),
        );
        params.insert(
            "p_session_id".to_owned(),
            CellValue::from_raw_string(session_id.clone()),
        );
        params.insert(
            "p_start_time_gmt".to_owned(),
            CellValue::from_raw_systemtime(current_datetime),
        );

        let query = SQLChange2 {
            sql_query: sql_insert.to_string(),
            params,
            sequence_name: "dokasys.sessions_id_seq".to_string(),
        };

        let Ok(session_db_id) = query.insert(&mut trans).await.map_err(err_fwd!(
            "💣 Cannot insert the session, follower=[{}]",
            &self.follower
        )) else {
            return WebResponse::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        if trans
            .commit()
            .await
            .map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))
            .is_err()
        {
            return WebResponse::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        log_info!(
            "😎 Session was opened with success, session_db_id=[{}], follower=[{}]",
            session_db_id,
            &self.follower
        );

        WebResponse::from_item(StatusCode::OK.as_u16(), session_id)
    }

    /// 🔑 Find a session from its session id
    pub async fn read_session(&mut self, session_id: &str) -> WebType<SessionReply> {
        log_info!("🚀 Start read_session api, follower=[{}]", &self.follower);

        // Check if the token is valid
        if !self.security_token.is_valid() {
            log_error!(
                "💣 Invalid security token, token=[{:?}], follower=[{}]",
                &self.security_token,
                &self.follower
            );
            return WebType::from_errorset(&&INVALID_TOKEN);
        }
        self.follower.token_type = TokenType::Token(self.security_token.0.clone());

        let session_reply = try_or_return!(self.read_session_and_update(&session_id).await, |e| {
            WebType::from(e)
        });

        log_info!(
            "😎 Updated the session renew timestamp, session id=[{}], follower=[{}]",
            &session_id,
            &self.follower
        );
        log_info!("🏁 End read_session api, follower=[{}]", &self.follower);

        WebType::from_item(StatusCode::OK.as_u16(), session_reply)
    }

    async fn read_session_and_update(&self, session_id: &str) -> WebResponse<SessionReply> {
        // Open Db connection
        let Ok(mut cnx) = SQLConnection2::from_pool().await.map_err(err_fwd!(
            "💣 New Db connection failed, follower=[{}]",
            &self.follower
        )) else {
            return WebResponse::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(mut trans) = cnx.begin().await.map_err(err_fwd!(
            "💣 Transaction issue, follower=[{}]",
            &self.follower
        )) else {
            return WebResponse::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        // Query the sessions to find the right one
        let Ok(sessions) = self
            .search_session_by_sid(&mut trans, Some(&session_id))
            .await
            .map_err(err_fwd!(
                "💣 Session search failed for session id=[{}], follower=[{}]",
                session_id,
                &self.follower
            ))
        else {
            return WebResponse::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        log_info!(
            "😎 Found the session information, number of sessions=[{}], follower=[{}]",
            sessions.len(),
            &self.follower
        );

        // Customer key to return
        let mut session_reply = SessionReply { sessions };

        // Check if the session was found
        if session_reply.sessions.is_empty() {
            log_warn!(
                "⛔ The session was not found, follower=[{}]",
                &self.follower
            );
            return WebResponse::from_errorset(&SESSION_NOT_FOUND);
        }

        let Ok(session) = session_reply
            .sessions
            .get_mut(0)
            .ok_or(anyhow!("Wrong index 0"))
            .map_err(err_fwd!(
                "💣 Cannot find the session in the list of sessions, follower=[{}]",
                &self.follower
            ))
        else {
            return WebResponse::from_errorset(&SESSION_NOT_FOUND);
        };

        // If the termination time exists, it means the session is closed
        if session.termination_time_gmt.is_some() {
            log_warn!(
                "⛔ The session is closed. Closing time =[{}], follower=[{}]",
                &session.termination_time_gmt.as_ref().unwrap(),
                &self.follower
            );
            return WebResponse::from_errorset(&SESSION_TIMED_OUT);
        }

        // Update the session renew_time_gmt
        let r_update = self.update_renew_time(&mut trans, &session_id).await;

        if r_update.is_err() {
            trans.rollback().await;
            log_warn!(
                "💣 Rollback. Cannot update the renew time of the session, follower=[{}]",
                &self.follower
            );
            return WebResponse::from_errorset(&SESSION_CANNOT_BE_RENEWED);
        }

        session.renew_time_gmt = Some(Utc::now().to_string());

        // End the transaction
        if trans
            .commit()
            .await
            .map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))
            .is_err()
        {
            return WebResponse::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        WebResponse::from_item(StatusCode::OK.as_u16(), session_reply)
    }

    /// Search the session information from the session id
    async fn search_session_by_sid(
        &self,
        mut trans: &mut SQLTransaction2<'_>,
        session_id: Option<&str>,
    ) -> anyhow::Result<Vec<EntrySession>> {
        let p_sid = CellValue::from_opt_str(session_id);

        let mut params = HashMap::new();
        params.insert("p_sid".to_owned(), p_sid);

        let query = SQLQueryBlock2 {
            sql_query : r"SELECT id, customer_code, customer_id, user_name, user_id, session_id, start_time_gmt, renew_time_gmt, termination_time_gmt
                    FROM dokasys.sessions
                    WHERE session_id = :p_sid OR :p_sid IS NULL ".to_string(),
            start : 0,
            length : Some(1),
            params,
        };

        let mut sql_result: SQLDataSet = query.execute(&mut trans).await.map_err(err_fwd!(
            "Query failed, [{}], follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;

        let mut sessions = vec![];
        while sql_result.next() {
            let id = sql_result.get_int("id").ok_or(anyhow!("Wrong column id"))?;
            let customer_code: String = sql_result
                .get_string("customer_code")
                .ok_or(anyhow!("Wrong column customer_code"))?;
            let customer_id: i64 = sql_result
                .get_int("customer_id")
                .ok_or(anyhow!("Wrong column customer_id"))?;
            let user_name: String = sql_result
                .get_string("user_name")
                .ok_or(anyhow!("Wrong column user_name"))?;
            let user_id: i64 = sql_result
                .get_int("user_id")
                .ok_or(anyhow!("Wrong column user_id"))?;
            let session_id: String = sql_result
                .get_string("session_id")
                .ok_or(anyhow!("Wrong column session_id"))?;
            let start_time_gmt = sql_result
                .get_timestamp_as_datetime("start_time_gmt")
                .ok_or(anyhow::anyhow!("Wrong column start_time_gmt"))
                .map_err(err_fwd!("Cannot read the start time"))?;

            // Optional
            let renew_time_gmt = sql_result
                .get_timestamp_as_datetime("renew_time_gmt")
                .as_ref()
                .map(|x| x.to_string());
            // Optional
            let termination_time_gmt = sql_result
                .get_timestamp_as_datetime("termination_time_gmt")
                .as_ref()
                .map(|x| x.to_string());

            let session_info = EntrySession {
                id,
                customer_code,
                user_name,
                customer_id,
                user_id,
                session_id,
                start_time_gmt: start_time_gmt.to_string(),
                renew_time_gmt,
                termination_time_gmt,
            };

            let _ = &sessions.push(session_info);
        }

        Ok(sessions)
    }

    /// Set the renew timestamp of the session to the current UTC time.
    async fn update_renew_time(
        &self,
        mut trans: &mut SQLTransaction2<'_>,
        session_id: &str,
    ) -> anyhow::Result<bool> {
        let p_sid = CellValue::from_raw_string(session_id.to_owned());

        let mut params = HashMap::new();
        params.insert("p_session_id".to_owned(), p_sid);

        let sql_update = r#"UPDATE dokasys.SESSIONS
                             SET renew_time_gmt = ( NOW() at time zone 'UTC'  )
                             WHERE session_id = :p_session_id "#;

        let query = SQLChange2 {
            sql_query: sql_update.to_string(),
            params,
            sequence_name: "".to_string(),
        };

        let _ = query.update(&mut trans).await.map_err(err_fwd!(
            "Cannot update the session renew timestamp, follower=[{}]",
            &self.follower
        ))?;
        Ok(true)
    }
}
