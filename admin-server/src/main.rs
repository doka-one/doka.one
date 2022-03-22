#![feature(proc_macro_hygiene, decl_macro)]
mod schema_cs;
mod schema_fs;
mod dk_password;
mod create_customer;

use std::path::Path;
use std::process::exit;
use log::*;
use rocket::*;
use rocket_contrib::json::Json;
use dkconfig::conf_reader::{read_config};
use std::collections::HashMap;

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
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use dkcrypto::dk_crypto::DkEncrypt;

use dkdto::{OpenSessionRequest, JsonErrorSet, LoginRequest, LoginReply};
use dkdto::error_codes::{INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR, INVALID_PASSWORD, INVALID_REQUEST, INVALID_TOKEN, SUCCESS};
use dkdto::error_replies::ErrorReply;
use doka_cli::request_client::{ SessionManagerClient};

use crate::dk_password::valid_password;
use crate::schema_fs::FS_SCHEMA;
use crate::schema_cs::CS_SCHEMA;


#[post("/login", format = "application/json", data = "<login_request>")]
fn login(login_request: Json<LoginRequest>) -> Json<LoginReply> {

    // There isn't any token to check

    // Generate a sessionId
    let clear_session_id   = uuid_v4();
    let cek = get_prop_value("cek");
    let session_id = match DkEncrypt::encrypt_str(&clear_session_id, &cek).map_err(err_fwd!("Cannot encrypt the session id")) {
        Ok(x) => x,
        Err(_) => {
            return Json(LoginReply::invalid_token_error_reply());
        }
    };

    // Find the user and its company, and grab the hashed password from it.

    let internal_database_error_reply: Json<LoginReply> = Json(LoginReply::internal_database_error_reply());

    let invalid_password_reply: Json<LoginReply> = Json(LoginReply::from_error(INVALID_PASSWORD));

    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
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

    let mut sql_result : SQLDataSet =  match query.execute(&mut trans).map_err(err_fwd!("Query failed, [{}]", &query.sql_query)) {
        Ok(x) => x,
        Err(_) => {
            return internal_database_error_reply;
        }
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

            (OpenSessionRequest {
                customer_code,
                user_name,
                customer_id,
                user_id,
                session_id,
            }, password_hash )
        }
        _ => {
            log_warn!("login not found, login=[{}]", &login_request.login);
            return internal_database_error_reply;
        }
    };

    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    // Verify the password

    if ! DkEncrypt::verify_password(&login_request.password, &password_hash) {
        log_warn!("Incorrect password for login, login=[{}]", &login_request.login);
        return invalid_password_reply;
    }

    // Open a session

    let sm_host = get_prop_value(SESSION_MANAGER_HOSTNAME_PROPERTY);
    let sm_port : u16 = get_prop_value(SESSION_MANAGER_PORT_PROPERTY).parse().unwrap();
    let smc = SessionManagerClient::new(&sm_host, sm_port);
    let response = smc.open_session(&open_session_request, &open_session_request.session_id);

    if response.status.error_code != 0 {
        log_error!("Session Manager failed with status [{:?}]", response.status);
        return Json(LoginReply{
            session_id : "".to_string(),
            status: JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR),
        });
    }

    Json(LoginReply{
        session_id : open_session_request.session_id.clone(),
        status: JsonErrorSet::from(SUCCESS),
    })
}



fn search_customer( trans : &mut SQLTransaction, customer_code : &str ) -> anyhow::Result<i64> {
    let mut params = HashMap::new();
    params.insert("p_customer_code".to_owned(), CellValue::from_raw_string(customer_code.to_string()));

    let query = SQLQueryBlock {
        sql_query: "SELECT id FROM dokaadmin.customer WHERE code = :p_customer_code AND is_removable = TRUE".to_string(),
        start: 0,
        length: None,
        params
    };
    let mut data_set = query.execute(trans).map_err(err_fwd!("Query failed"))?;

    if data_set.len() == 0 {
        return Err(anyhow::anyhow!("Customer code not found"));
    }
    let _ = data_set.next();
    let customer_id = data_set.get_int("id").unwrap_or(0i64);
    Ok(customer_id)
}

fn delete_user_from_db( trans : &mut SQLTransaction, customer_code : &str ) -> anyhow::Result<bool> {
    let mut params = HashMap::new();
    params.insert("p_customer_code".to_owned(), CellValue::from_raw_string(customer_code.to_string()));

    let query = SQLChange {
        sql_query: r"DELETE FROM dokaadmin.appuser WHERE customer_id IN
        (SELECT id FROM dokaadmin.customer WHERE code = :p_customer_code AND is_removable = TRUE)".to_string(),
        params,
        sequence_name: "".to_string(),
    };
    let nb_delete = query.delete(trans).map_err(err_fwd!("Query failed"))?;

    if nb_delete == 0 {
        return Err(anyhow::anyhow!("We did not delete any user for the customer"));
    }

    Ok(true)
}


fn delete_customer_from_db( trans : &mut SQLTransaction, customer_code : &str ) -> anyhow::Result<bool> {
    let mut params = HashMap::new();
    params.insert("p_customer_code".to_owned(), CellValue::from_raw_string(customer_code.to_string()));

    let query = SQLChange {
        sql_query: r"DELETE FROM dokaadmin.customer WHERE code = :p_customer_code AND is_removable = TRUE".to_string(),
        params,
        sequence_name: "".to_string(),
    };
    let nb = query.delete(trans).map_err(err_fwd!("Query failed"))?;

    if nb == 0 {
        return Err(anyhow::anyhow!("We did not delete any customer"));
    }

    Ok(true)
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


fn drop_schema_from_db( trans : &mut SQLTransaction, customer_code : &str ) -> anyhow::Result<bool> {
    let query = SQLChange {
        sql_query: format!( r"DROP SCHEMA cs_{} CASCADE", customer_code ),
        params : Default::default(),
        sequence_name: "".to_string(),
    };
    let _ = query.batch(trans).map_err(err_fwd!("Dropping the schema failed, customer_code=[{}]", customer_code))?;

    Ok(true)
}

///
/// Delete a customer with schema and all
///
#[delete("/customer/<customer_code>")]
fn delete_customer(customer_code: &RawStr, security_token: SecurityToken) -> Json<JsonErrorSet> {

    // Check if the token is valid
    if !security_token.is_valid() {
        return Json(JsonErrorSet::from(INVALID_TOKEN));
    }

    let token = security_token.take_value();

    log_info!("🚀 Start delete_customer api, token={}", &token);

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

    // Check if the customer is removable (flag is_removable)

    let _customer_id = match search_customer(&mut trans, &customer_code) {
        Ok(x) => x,
        Err(_) => { return internal_database_error_reply; },
    };

    // Clear the customer table and user

    // TODO Look if we display the "e" in the fwd!
    if delete_user_from_db(&mut trans, &customer_code).map_err(err_fwd!("")).is_err() {
        return internal_database_error_reply;
    }

    if delete_customer_from_db(&mut trans, &customer_code).map_err(err_fwd!("")).is_err() {
        return internal_database_error_reply;
    }

    // Remove the db schema

    if drop_schema_from_db(&mut trans, &customer_code).map_err(err_fwd!("")).is_err() {
        return internal_database_error_reply;
    }


    // Close the transaction
    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    log_info!("😎 Customer delete created with success");

    log_info!("🏁 End delete_customer, token_id = {}", &token);

    Json(JsonErrorSet::from(SUCCESS))
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
        .mount(&base_url, routes![set_removable_flag_customer, delete_customer,
            create_customer::create_customer, login])
        .attach(Template::fairing())
        .launch();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
