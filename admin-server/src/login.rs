use log::*;

use rocket_contrib::json::Json;

use std::collections::HashMap;
use anyhow::anyhow;

use commons_error::*;

use rs_uuid::iso::uuid_v4;
use commons_pg::{SQLConnection, CellValue, SQLQueryBlock, SQLTransaction};
use commons_services::database_lib::open_transaction;
use commons_services::property_name::{COMMON_EDIBLE_KEY_PROPERTY, SESSION_MANAGER_HOSTNAME_PROPERTY, SESSION_MANAGER_PORT_PROPERTY};


use commons_services::x_request_id::{Follower, XRequestID};
use dkconfig::properties::{get_prop_value};
use dkcrypto::dk_crypto::DkEncrypt;

use dkdto::{OpenSessionRequest, JsonErrorSet, LoginRequest, LoginReply};
use dkdto::error_codes::{INVALID_PASSWORD, SESSION_LOGIN_DENIED, SUCCESS};
use dkdto::error_replies::ErrorReply;
use doka_cli::request_client::{SessionManagerClient, TokenType};

pub(crate) struct LoginDelegate {
    // pub security_token: SecurityToken,
    pub follower: Follower,
}

impl LoginDelegate {
    pub fn new(x_request_id: XRequestID) -> Self {
        LoginDelegate {
            follower: Follower {
                x_request_id,
                token_type: TokenType::None,
            }
        }
    }

    pub fn login(mut self, login_request: Json<LoginRequest>) -> Json<LoginReply> {
        // There isn't any token to check

        self.follower.x_request_id = self.follower.x_request_id.new_if_null();
        log_info!("🚀 Start login api, login=[{}], follower=[{}]", &login_request.login, &self.follower);

        // Generate a sessionId
        let clear_session_id = uuid_v4();

        // In Private Customer Key Mode, the user will provide its own CEK in the LoginRequest
        // This CEK cannot be stored anywhere, so must be passed along to all request call
        // in TLS encrypted headers.

        let Ok(cek) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY)
            .map_err(err_fwd!("💣 Cannot read the cek, follower=[{}]", &self.follower)) else {
            return Json(LoginReply::invalid_common_edible_key());
        };

        // let-else
        let Ok(session_id) = DkEncrypt::encrypt_str(&clear_session_id, &cek)
            .map_err(err_fwd!("💣 Cannot encrypt the session id, follower=[{}]", &self.follower)) else {
            return Json(LoginReply::invalid_token_error_reply());
        };

        // The follower the an easiest way to pass the information
        // between local routines
        self.follower.token_type = TokenType::Sid(session_id);

        // Find the user and its company, and grab the hashed password from it.

        let internal_database_error_reply: Json<LoginReply> = Json(LoginReply::internal_database_error_reply());
        let invalid_password_reply: Json<LoginReply> = Json(LoginReply::from_error(INVALID_PASSWORD));

        let mut r_cnx = SQLConnection::new();
        // let-else
        let r_trans = open_transaction(&mut r_cnx).map_err(err_fwd!("💣 Open transaction error, follower=[{}]", &self.follower));
        let Ok(mut trans) = r_trans else {
            return internal_database_error_reply;
        };

        let Ok((open_session_request, password_hash)) = self.search_user(&mut trans, &login_request.login) else {
            log_warn!("⛔ login not found, login=[{}], follower=[{}]", &login_request.login, &self.follower);
            return Json(LoginReply::from_error(SESSION_LOGIN_DENIED));
        };


        if trans.commit().map_err(err_fwd!("💣 Commit failed")).is_err() {
            return internal_database_error_reply;
        }

        // Verify the password

        if !DkEncrypt::verify_password(&login_request.password, &password_hash) {
            log_warn!("💣 Incorrect password for login, login=[{}], follower=[{}]", &login_request.login, &self.follower);
            return invalid_password_reply;
        }

        log_info!("😎 Password verified, follower=[{}]", &self.follower);

        // Open a session

        let Ok(smc) = (|| -> anyhow::Result<SessionManagerClient> {
            let sm_host = get_prop_value(SESSION_MANAGER_HOSTNAME_PROPERTY)
                .map_err(err_fwd!("💣 Cannot read Session Manager hostname, follower=[{}]", &self.follower))?;
            let sm_port: u16 = get_prop_value(SESSION_MANAGER_PORT_PROPERTY)?.parse()
                .map_err(err_fwd!("💣 Cannot read Session Manager port, follower=[{}]", &self.follower))?;
            let smc = SessionManagerClient::new(&sm_host, sm_port);
            Ok(smc)
        }) () else {
            log_error!("💣 Session Manager Client creation failed, follower=[{}]", &self.follower);
            return Json(LoginReply::internal_technical_error_reply());
        };

        // !!! The generated session_id is also used as a token_id !!!!
        let response = smc.open_session(&open_session_request, &open_session_request.session_id, self.follower.x_request_id.value());

        if response.status.error_code != 0 {
            log_error!("💣 Session Manager failed with status [{:?}], follower=[{}]", response.status, &self.follower);
            return Json(LoginReply::internal_technical_error_reply());
        }

        let session_id = open_session_request.session_id.clone();

        log_info!("😎 Login with success, follower=[{}]", &self.follower);

        log_info!("🏁 End login api, login=[{}], follower=[{}]", &login_request.login, &self.follower);

        Json(LoginReply{
            session_id,
            status: JsonErrorSet::from(SUCCESS),
        })
    }


    ///
    fn search_user(&self, trans : &mut SQLTransaction, login: &str) -> anyhow::Result<(OpenSessionRequest, String)> {

        let mut params = HashMap::new();
        params.insert("p_login".to_owned(), CellValue::from_raw_string(login.to_string()));

        let query = SQLQueryBlock {
            sql_query : r"SELECT u.id, u.customer_id, u.login, u.password_hash, u.default_language, u.default_time_zone, u.admin,
                        c.code as customer_code,  u.full_name as user_name, c.full_name as company_name
                        FROM dokaadmin.appuser u INNER JOIN dokaadmin.customer c ON (c.id = u.customer_id)
                        WHERE login = :p_login ".to_string(),
            start : 0,
            length : Some(1),
            params,
        };

        let mut sql_result = query.execute( trans)
                        .map_err(err_fwd!("💣 Query failed, [{}], follower=[{}]", &query.sql_query, &self.follower))?;

        let session_and_pass = match sql_result.next() {
            true => {
                let user_id: i64 = sql_result.get_int("id").ok_or(anyhow!("Wrong id"))?;
                let customer_id: i64 = sql_result.get_int("customer_id").ok_or(anyhow!("Wrong customer id"))?;
                let _login: String = sql_result.get_string("login").ok_or(anyhow!("Wrong login name"))?;
                let password_hash: String = sql_result.get_string("password_hash").ok_or(anyhow!("Wrong password hash"))?;
                let _default_language: String = sql_result.get_string("default_language").ok_or(anyhow!("Wrong default language"))?;
                let _default_time_zone: String = sql_result.get_string("default_time_zone").ok_or(anyhow!("Wrong time zone"))?;
                let _is_admin: bool = sql_result.get_bool("admin").ok_or(anyhow!("Wrong admin flag"))?;
                let customer_code: String = sql_result.get_string("customer_code").ok_or(anyhow!("Wrong customer code"))?;
                let user_name: String = sql_result.get_string("user_name").ok_or(anyhow!("Wrong user name"))?;
                let _company_name: String = sql_result.get_string("company_name").ok_or(anyhow!("Wrong company name"))?;

                log_info!("Found user information for user, login=[{}], user id=[{}], customer id=[{}], follower=[{}]",
                            login, user_id, customer_id, &self.follower);

                (OpenSessionRequest {
                    customer_code,
                    user_name,
                    customer_id,
                    user_id,
                    session_id : self.follower.token_type.value(),
                }, password_hash )
            }
            _ => {
                log_warn!("⛔ login not found, login=[{}], follower=[{}]", login, &self.follower);
                return Err(anyhow!("login not found"));
            }
        };

        Ok(session_and_pass)
    }

}

