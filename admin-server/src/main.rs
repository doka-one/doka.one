#![feature(proc_macro_hygiene, decl_macro)]
#![feature(let_else)]

mod schema_cs;
mod schema_fs;
mod dk_password;
mod customer;

use std::path::Path;
use std::process::exit;
use log::*;
use rocket::*;
use rocket_contrib::json::Json;
use dkconfig::conf_reader::{read_config};
use std::collections::HashMap;
use guard::guard;

use commons_error::*;
use rocket_contrib::templates::Template;
use rocket::config::Environment;
use rocket::http::RawStr;
use rs_uuid::iso::uuid_v4;
use commons_pg::{SQLConnection, SQLChange, CellValue, SQLQueryBlock, SQLDataSet, SQLTransaction, init_db_pool};
use commons_services::database_lib::open_transaction;
use commons_services::property_name::{SESSION_MANAGER_HOSTNAME_PROPERTY, SESSION_MANAGER_PORT_PROPERTY};
use commons_services::read_cek_and_store;
use commons_services::token_lib::SecurityToken;
use commons_services::x_request_id::{TwinId, XRequestID};
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use dkcrypto::dk_crypto::DkEncrypt;

use dkdto::{OpenSessionRequest, JsonErrorSet, LoginRequest, LoginReply};
use dkdto::error_codes::{INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR, INVALID_PASSWORD, INVALID_REQUEST, INVALID_TOKEN, SUCCESS};
use dkdto::error_replies::ErrorReply;
use doka_cli::request_client::{SessionManagerClient, TokenType};

use crate::dk_password::valid_password;
use crate::schema_fs::FS_SCHEMA;
use crate::schema_cs::CS_SCHEMA;

///
/// * Generate a x_request_id
/// * Generate a session id
/// * Looking for the user / customer in the db
/// * Validate the password
/// * Register a session (end point)
/// * Return the session_id (encrypted)
///
/// The security here is ensured by the user/password verification
/// The DDoS or Brute Force attack must be handle by the network architecture
///
#[post("/login", format = "application/json", data = "<login_request>")]
fn login(login_request: Json<LoginRequest>) -> Json<LoginReply> {

    // There isn't any token to check
    let x_request_id = XRequestID::new();
    log_info!("🚀 Start login api, login=[{}], x_request_id=[{}]", &login_request.login, x_request_id);

    // Generate a sessionId
    let clear_session_id= uuid_v4();

    // In Private Customer Key Mode, the user will provide its own CEK in the LoginRequest
    // This CEK cannot be stored anywhere, so must be passed along to all request call
    // in TLS encrypted headers.

    let cek = get_prop_value("cek");

    // let-else does not work with rocket ! :(
    let r_session_id = DkEncrypt::encrypt_str(&clear_session_id, &cek).map_err(err_fwd!("💣 Cannot encrypt the session id"));
    guard!(let Ok(session_id) = r_session_id else {
            return Json(LoginReply::invalid_token_error_reply());
    });

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
    // let-else does not work with rocket ! :(
    let r_trans = open_transaction(&mut r_cnx).map_err(err_fwd!("💣 Open transaction error"));
    guard!(let Ok(mut trans) = r_trans else {
         return internal_database_error_reply;
    });

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

    // let-else does not work with rocket :(
    let r_sql_result = query.execute(&mut trans).map_err(err_fwd!("💣 Query failed, [{}]", &query.sql_query));
    guard!(let Ok(mut sql_result) = r_sql_result else {
            return internal_database_error_reply;
    });

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
        return Json(LoginReply{
            session_id : "".to_string(),
            status: JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR),
        });
    }

    let session_id = open_session_request.session_id.clone();

    log_info!("😎 Login with success, twin_id=[{}]", &twin_id);

    log_info!("🏁 End login api, login=[{}], twin_id=[{}]", &login_request.login, &twin_id);

    Json(LoginReply{
        session_id,
        status: JsonErrorSet::from(SUCCESS),
    })
}




fn set_removable_flag_customer_from_db( trans : &mut SQLTransaction, customer_code : &str ) -> anyhow::Result<bool> {
    let mut params = HashMap::new();
    params.insert("p_customer_code".to_owned(), CellValue::from_raw_string(customer_code.to_string()));

    let query = SQLChange {
        sql_query: r"UPDATE dokaadmin.customer SET is_removable = TRUE  WHERE code = :p_customer_code".to_string(),
        params,
        sequence_name: "".to_string(),
    };
    let nb = query.update(trans).map_err(err_fwd!("Query failed"))?;

    if nb == 0 {
        return Err(anyhow::anyhow!("We did not set any removable flag for any customer"));
    }

    Ok(true)
}






#[patch("/customer/removable/<customer_code>")]
fn set_removable_flag_customer(customer_code: &RawStr, security_token: SecurityToken) -> Json<JsonErrorSet> {

    // Check if the token is valid
    if !security_token.is_valid() {
        return  Json(JsonErrorSet::from(INVALID_TOKEN));
    }

    let token = security_token.take_value();

    log_info!("🚀 Start set_removable_flag_customer api, token={}", &token);

    let customer_code = match customer_code.percent_decode().map_err(err_fwd!("Invalid input parameter [{}]", customer_code) ) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return Json(JsonErrorSet::from(INVALID_REQUEST));
        }
    };


    let internal_database_error_reply = Json(JsonErrorSet::from(INTERNAL_DATABASE_ERROR));

    // | Open the transaction
    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    if set_removable_flag_customer_from_db(&mut trans, &customer_code).map_err(err_fwd!("")).is_err() {
        return internal_database_error_reply;
    }

    // Close the transaction
    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    log_info!("😎 Set removable flag with success");

    log_info!("🏁 End set_removable_flag_customer, token_id = {}", &token);

    Json(JsonErrorSet::from(SUCCESS))

}



///
///
///
fn main() {

    const PROGRAM_NAME: &str = "Admin Server";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "admin-server";
    const VAR_NAME: &str = "DOKA_ENV";

    // Read the application config's file
    println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);

    let props = read_config(PROJECT_CODE, VAR_NAME);

    dbg!(&props);
    set_prop_values(props);

    let port = get_prop_value("server.port").parse::<u16>().unwrap();
    dbg!(port);

    let log_config: String = get_prop_value("log4rs.config");

    let log_config_path = Path::new(&log_config);

    // Read the global properties
    println!("😎 Read log properties from {:?}", &log_config_path);

    match log4rs::init_file(&log_config_path, Default::default()) {
        Err(e) => {
            eprintln!("{:?} {:?}", &log_config_path, e);
            exit(-59);
        }
        Ok(_) => {}
    }

    // Read the CEK
    log_info!("😎 Read Common Edible Key");
    read_cek_and_store();

    let new_prop = get_prop_value("cek");
    dbg!(&new_prop);

    // Init DB pool
    let (connect_string, db_pool_size) = match get_prop_pg_connect_string()
        .map_err(err_fwd!("Cannot read the database connection information")) {
        Ok(x) => x,
        Err(e) => {
            log_error!("{:?}", e);
            exit(-64);
        }
    };

    init_db_pool(&connect_string, db_pool_size);

    log_info!("🚀 Start {}", PROGRAM_NAME);

    let mut my_config = Config::new(Environment::Production);
    my_config.set_port(port);

    let base_url = format!("/{}", PROJECT_CODE);

    // let a = create_customer::create_customer();

    let _ = rocket::custom(my_config)
        .mount(&base_url, routes![set_removable_flag_customer, customer::delete_customer,
            customer::create_customer, login])
        .attach(Template::fairing())
        .launch();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
