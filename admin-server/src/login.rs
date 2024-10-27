use std::collections::HashMap;

use anyhow::anyhow;
use axum::http::StatusCode;
use axum::Json;
use log::*;
use rs_uuid::iso::uuid_v4;

use commons_error::*;
use commons_pg::sql_transaction::CellValue;
use commons_pg::sql_transaction_async::{
    SQLConnectionAsync, SQLQueryBlockAsync, SQLTransactionAsync,
};
use commons_services::property_name::{
    COMMON_EDIBLE_KEY_PROPERTY, SESSION_MANAGER_HOSTNAME_PROPERTY, SESSION_MANAGER_PORT_PROPERTY,
};
use commons_services::try_or_return;
use commons_services::x_request_id::{Follower, XRequestID};
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::error_codes::{
    INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR, INVALID_CEK, INVALID_TOKEN,
    SESSION_LOGIN_DENIED,
};
use dkdto::{LoginReply, LoginRequest, OpenSessionRequest, WebResponse, WebType, WebTypeBuilder};
use doka_cli::async_request_client::SessionManagerClientAsync;
use doka_cli::request_client::TokenType;

#[derive(Debug, Clone)]
pub(crate) struct LoginDelegate {
    // pub security_token: SecurityToken,
    pub follower: Follower,
}

impl LoginDelegate {
    pub fn new(x_request_id: XRequestID) -> Self {
        LoginDelegate {
            follower: Follower {
                x_request_id: x_request_id.new_if_null(),
                token_type: TokenType::None,
            },
        }
    }

    pub async fn login(mut self, login_request: Json<LoginRequest>) -> WebType<LoginReply> {
        // There isn't any token to check

        // Already done : self.follower.x_request_id = self.follower.x_request_id.new_if_null();
        log_info!(
            "🚀 Start login api, login=[{}], follower=[{}]",
            &login_request.login,
            &self.follower
        );

        // Generate a sessionId
        let clear_session_id = uuid_v4();

        // In Private Customer Key Mode, the user will provide its own CEK in the LoginRequest
        // This CEK cannot be stored anywhere, so must be passed along to all request call
        // in TLS encrypted headers.

        let Ok(cek) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY).map_err(err_fwd!(
            "💣 Cannot read the cek, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INVALID_CEK);
        };

        // let-else
        let Ok(session_id) = DkEncrypt::encrypt_str(&clear_session_id, &cek).map_err(err_fwd!(
            "💣 Cannot encrypt the session id, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INVALID_TOKEN);
        };

        // The follower the an easiest way to pass the information
        // between local routines
        self.follower.token_type = TokenType::Sid(session_id);

        // Get the password hash and the open session request
        let (open_session_request, password_hash) =
            try_or_return!(self.find_user_and_company(&login_request).await, |e| {
                WebType::from(e)
            });

        // Verify the password

        if !DkEncrypt::verify_password(&login_request.password, &password_hash) {
            log_warn!(
                "⛔ Incorrect password for login, login=[{}], follower=[{}]",
                &login_request.login,
                &self.follower
            );
            return WebType::from_errorset(&SESSION_LOGIN_DENIED);
        }

        log_info!("😎 Password verified, follower=[{}]", &self.follower);

        // Open a session

        let Ok(smc) = self.build_session_manager_client().await else {
            log_error!(
                "💣 Session Manager Client creation failed, follower=[{}]",
                &self.follower
            );
            return WebType::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };

        log_info!(
            "Session client service is ok, follower=[{}]",
            &self.follower
        );

        let response = smc
            .open_session(
                &open_session_request,
                &open_session_request.session_id,
                self.follower.x_request_id.value(),
            )
            .await;

        if let Err(e) = response {
            log_error!(
                "💣 Session Manager failed with status [{:?}], follower=[{}]",
                e.message,
                &self.follower
            );
            return WebType::from(e);
        }

        let customer_code = open_session_request.customer_code.clone();
        let session_id = open_session_request.session_id.clone();

        log_info!("😎 Login with success, follower=[{}]", &self.follower);

        log_info!(
            "🏁 End login api, login=[{}], follower=[{}]",
            &login_request.login,
            &self.follower
        );

        WebType::from_item(
            StatusCode::ACCEPTED.as_u16(),
            LoginReply {
                session_id,
                customer_code,
            },
        )
    }

    async fn build_session_manager_client(&self) -> anyhow::Result<SessionManagerClientAsync> {
        let sm_host = get_prop_value(SESSION_MANAGER_HOSTNAME_PROPERTY).map_err(err_fwd!(
            "💣 Cannot read Session Manager hostname, follower=[{}]",
            &self.follower
        ))?;
        let sm_port: u16 = get_prop_value(SESSION_MANAGER_PORT_PROPERTY)?
            .parse()
            .map_err(err_fwd!(
                "💣 Cannot read Session Manager port, follower=[{}]",
                &self.follower
            ))?;
        Ok(SessionManagerClientAsync::new(&sm_host, sm_port))
    }

    // async fn find_user_and_company_async(
    //     &self,
    //     login_request: &LoginRequest,
    // ) -> WebResponse<(OpenSessionRequest, String)> {
    //     let local_self = self.clone();
    //     let local_login_request = login_request.clone();
    //     let sync_call = move || local_self.find_user_and_company(&local_login_request);
    //     run_blocking_spawn(sync_call, &self.follower).await
    // }

    /// Find the user and its company, and grab the hashed password from it.
    async fn find_user_and_company(
        &self,
        login_request: &LoginRequest,
    ) -> WebResponse<(OpenSessionRequest, String)> {
        // Open Db connection
        let Ok(mut cnx) = SQLConnectionAsync::from_pool().await.map_err(err_fwd!(
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

        let Ok((open_session_request, password_hash)) =
            self.search_user(&mut trans, &login_request.login).await
        else {
            log_warn!(
                "⛔ user not found, login=[{}], follower=[{}]",
                &login_request.login,
                &self.follower
            );
            return WebResponse::from_errorset(&SESSION_LOGIN_DENIED);
        };

        if trans
            .commit()
            .await
            .map_err(err_fwd!("💣 Commit failed"))
            .is_err()
        {
            return WebResponse::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        WebResponse::from_item(
            StatusCode::OK.as_u16(),
            (open_session_request, password_hash),
        )
    }

    ///
    async fn search_user(
        &self,
        trans: &mut SQLTransactionAsync<'_>,
        login: &str,
    ) -> anyhow::Result<(OpenSessionRequest, String)> {
        let mut params = HashMap::new();
        params.insert(
            "p_login".to_owned(),
            CellValue::from_raw_string(login.to_string()),
        );

        let query = SQLQueryBlockAsync {
            sql_query : r"SELECT u.id, u.customer_id, u.login, u.password_hash, u.default_language, u.default_time_zone, u.admin,
                        c.code as customer_code,  u.full_name as user_name, c.full_name as company_name
                        FROM dokaadmin.appuser u INNER JOIN dokaadmin.customer c ON (c.id = u.customer_id)
                        WHERE login = :p_login ".to_string(),
            start : 0,
            length : Some(1),
            params,
        };

        let mut sql_result = query.execute(trans).await.map_err(err_fwd!(
            "💣 Query failed, [{}], follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;
        let session_and_pass = match sql_result.next() {
            true => {
                let user_id: i64 = sql_result.get_int("id").ok_or(anyhow!("Wrong id"))?;
                let customer_id: i64 = sql_result
                    .get_int("customer_id")
                    .ok_or(anyhow!("Wrong customer id"))?;
                let _login: String = sql_result
                    .get_string("login")
                    .ok_or(anyhow!("Wrong login name"))?;
                let password_hash: String = sql_result
                    .get_string("password_hash")
                    .ok_or(anyhow!("Wrong password hash"))?;
                let _default_language: String = sql_result
                    .get_string("default_language")
                    .ok_or(anyhow!("Wrong default language"))?;
                let _default_time_zone: String = sql_result
                    .get_string("default_time_zone")
                    .ok_or(anyhow!("Wrong time zone"))?;
                let _is_admin: bool = sql_result
                    .get_bool("admin")
                    .ok_or(anyhow!("Wrong admin flag"))?;
                let customer_code: String = sql_result
                    .get_string("customer_code")
                    .ok_or(anyhow!("Wrong customer code"))?;
                let user_name: String = sql_result
                    .get_string("user_name")
                    .ok_or(anyhow!("Wrong user name"))?;
                let _company_name: String = sql_result
                    .get_string("company_name")
                    .ok_or(anyhow!("Wrong company name"))?;

                log_info!("Found user information for user, login=[{}], user id=[{}], customer id=[{}], follower=[{}]",
                            login, user_id, customer_id, &self.follower);

                (
                    OpenSessionRequest {
                        customer_code,
                        user_name,
                        customer_id,
                        user_id,
                        session_id: self.follower.token_type.value(),
                    },
                    password_hash,
                )
            }
            false => {
                log_warn!(
                    "⛔ login not found, login=[{}], follower=[{}]",
                    login,
                    &self.follower
                );
                return Err(anyhow!("login not found"));
            }
        };

        Ok(session_and_pass)
    }
}
