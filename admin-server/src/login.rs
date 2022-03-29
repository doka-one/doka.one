

use log::*;

use rocket_contrib::json::Json;

use std::collections::HashMap;

use commons_error::*;



use rs_uuid::iso::uuid_v4;
use commons_pg::{SQLConnection, CellValue, SQLQueryBlock};
use commons_services::database_lib::open_transaction;
use commons_services::property_name::{SESSION_MANAGER_HOSTNAME_PROPERTY, SESSION_MANAGER_PORT_PROPERTY};


use commons_services::x_request_id::{TwinId, XRequestID};
use dkconfig::properties::{get_prop_value};
use dkcrypto::dk_crypto::DkEncrypt;

use dkdto::{OpenSessionRequest, JsonErrorSet, LoginRequest, LoginReply};
use dkdto::error_codes::{INVALID_PASSWORD, SUCCESS};
use dkdto::error_replies::ErrorReply;
use doka_cli::request_client::{SessionManagerClient, TokenType};





pub (crate) fn login_delegate(login_request: Json<LoginRequest>) -> Json<LoginReply> {
    // There isn't any token to check
    let x_request_id = XRequestID::new();
    log_info!("🚀 Start login api, login=[{}], x_request_id=[{}]", &login_request.login, x_request_id);

    // Generate a sessionId
    let clear_session_id= uuid_v4();

    // In Private Customer Key Mode, the user will provide its own CEK in the LoginRequest
    // This CEK cannot be stored anywhere, so must be passed along to all request call
    // in TLS encrypted headers.

    let cek = get_prop_value("cek");

    // let-else
    let Ok(session_id) = DkEncrypt::encrypt_str(&clear_session_id, &cek).map_err(err_fwd!("💣 Cannot encrypt the session id")) else {
        return Json(LoginReply::invalid_token_error_reply());
    };

    // The twin id is an easiest way to pass the information
    // between local routines
    let twin_id = TwinId {
        token_type : TokenType::Sid(&session_id),
        x_request_id: x_request_id
    };

    // Find the user and its company, and grab the hashed password from it.

    let internal_database_error_reply: Json<LoginReply> = Json(LoginReply::internal_database_error_reply());
    let invalid_password_reply: Json<LoginReply> = Json(LoginReply::from_error(INVALID_PASSWORD));

    let mut r_cnx = SQLConnection::new();
    // let-else
    let r_trans = open_transaction(&mut r_cnx).map_err(err_fwd!("💣 Open transaction error"));
    let Ok(mut trans) = r_trans else {
        return internal_database_error_reply;
    };

    let mut params = HashMap::new();
    params.insert("p_login".to_owned(), CellValue::from_raw_string(login_request.login.clone()));

    let query = SQLQueryBlock {
        sql_query : r"SELECT u.id, u.customer_id, u.login, u.password_hash, u.default_language, u.default_time_zone, u.admin,
                        c.code as customer_code,  u.full_name as user_name, c.full_name as company_name
                        FROM dokaadmin.appuser u INNER JOIN dokaadmin.customer c ON (c.id = u.customer_id)
                        WHERE login = :p_login ".to_string(),
        start : 0,
        length : Some(1),
        params,
    };

    // let-else
    let Ok(mut sql_result) = query.execute(&mut trans).map_err(err_fwd!("💣 Query failed, [{}]", &query.sql_query)) else {
        return internal_database_error_reply;
    };

    let (open_session_request, password_hash) = match sql_result.next() {
        true => {
            let user_id: i64 = sql_result.get_int("id").unwrap_or(0i64);
            let customer_id: i64 = sql_result.get_int("customer_id").unwrap_or(0i64);
            let _login: String = sql_result.get_string("login").unwrap_or("".to_owned());
            let password_hash: String = sql_result.get_string("password_hash").unwrap_or("".to_owned());
            let _default_language: String = sql_result.get_string("default_language").unwrap_or("".to_owned());
            let _default_time_zone: String = sql_result.get_string("default_time_zone").unwrap_or("".to_owned());
            let _is_admin: bool = sql_result.get_bool("admin").unwrap_or(false);
            let customer_code: String = sql_result.get_string("customer_code").unwrap_or("".to_owned());
            let user_name: String = sql_result.get_string("user_name").unwrap_or("".to_owned());
            let _company_name: String = sql_result.get_string("company_name").unwrap_or("".to_owned());

            log_info!("Found user information for user, login=[{}], user id=[{}], customer id=[{}], twin_id=[{}]",
                &login_request.login, user_id, customer_id, &twin_id);

            (OpenSessionRequest {
                customer_code,
                user_name,
                customer_id,
                user_id,
                session_id : twin_id.token_type.value(),
            }, password_hash )
        }
        _ => {
            log_warn!("⛔ login not found, login=[{}], twin_id=[{}]", &login_request.login, &twin_id);
            return internal_database_error_reply;
        }
    };

    if trans.commit().map_err(err_fwd!("💣 Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    // Verify the password

    if ! DkEncrypt::verify_password(&login_request.password, &password_hash) {
        log_warn!("💣 Incorrect password for login, login=[{}]", &login_request.login);
        return invalid_password_reply;
    }

    // Open a session

    let sm_host = get_prop_value(SESSION_MANAGER_HOSTNAME_PROPERTY);
    let sm_port : u16 = get_prop_value(SESSION_MANAGER_PORT_PROPERTY).parse().map_err(err_fwd!("Cannot read Session Manager port")).unwrap();
    let smc = SessionManagerClient::new(&sm_host, sm_port);

    // !!! The generated session_id is also used as a token_id !!!!
    let response = smc.open_session(&open_session_request, &open_session_request.session_id, x_request_id.value());

    if response.status.error_code != 0 {
        log_error!("💣 Session Manager failed with status [{:?}]", response.status);
        return Json(LoginReply::internal_technical_error_reply());
    }

    let session_id = open_session_request.session_id.clone();

    log_info!("😎 Login with success, twin_id=[{}]", &twin_id);

    log_info!("🏁 End login api, login=[{}], twin_id=[{}]", &login_request.login, &twin_id);

    Json(LoginReply{
        session_id,
        status: JsonErrorSet::from(SUCCESS),
    })
}