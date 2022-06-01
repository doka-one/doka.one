#![feature(proc_macro_hygiene, decl_macro)]
#![feature(let_else)]

mod schema_cs;
mod schema_fs;
mod dk_password;
mod customer;
mod login;

use log::*;
use std::path::Path;
use std::process::exit;
use rocket::{Config, routes, post, patch, delete};
use rocket::config::Environment;
use rocket::http::RawStr;
use rocket_contrib::json::Json;
use rocket_contrib::templates::Template;
use commons_error::{err_fwd, err_closure_fwd, log_error, log_info};
use commons_pg::init_db_pool;
use commons_services::property_name::{COMMON_EDIBLE_KEY_PROPERTY, LOG_CONFIG_FILE_PROPERTY, SERVER_PORT_PROPERTY};
use commons_services::read_cek_and_store;
use commons_services::token_lib::SecurityToken;
use commons_services::x_request_id::XRequestID;
use dkconfig::conf_reader::read_config;
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use dkdto::{CreateCustomerReply, CreateCustomerRequest, JsonErrorSet, LoginReply, LoginRequest};
use crate::customer::{CustomerDelegate, set_removable_flag_customer_delegate};
use crate::login::login_delegate;



///
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
///  1A  ⛔ 2A  ✔ 3A  ✔1B  ✔2B  ✔3B  ✔4B  ✔5B  ✔1C  ✔1D  ✔
///
#[post("/login", format = "application/json", data = "<login_request>")]
pub fn login(login_request: Json<LoginRequest>) -> Json<LoginReply> {
    login_delegate(login_request)
}

///
/// Set a flag on a customer to allow its deletion
/// 1A ✔  2A  ✔ 3A  ✔1B  ✔2B  ✔3B  ✔4B  ✔5B  ✔1C  ✔1D  ✔
///
#[patch("/customer/removable/<customer_code>")]
pub fn set_removable_flag_customer(customer_code: &RawStr, security_token: SecurityToken) -> Json<JsonErrorSet> {
    set_removable_flag_customer_delegate(customer_code, security_token)
}


///
/// Create a brand new customer with schema and all
/// 1A ✔  2A  ✔ 3A  ✔1B  ✔2B  ✔3B  ✔4B  ✔5B  ✔1C  ✔1D  ✔
///
#[post("/customer", format = "application/json", data = "<customer_request>")]
pub fn create_customer(customer_request: Json<CreateCustomerRequest>, security_token: SecurityToken, x_request_id: XRequestID) -> Json<CreateCustomerReply> {
    let delegate = CustomerDelegate::new(security_token, x_request_id);
    delegate.create_customer(customer_request)
}

///
/// Delete a customer with schema and all
///
#[delete("/customer/<customer_code>")]
pub fn delete_customer(customer_code: &RawStr, security_token: SecurityToken, x_request_id: XRequestID) -> Json<JsonErrorSet> {
    // delete_customer_delegate(customer_code, security_token, x_request_id)
    let delegate = CustomerDelegate::new(security_token, x_request_id);
    delegate.delete_customer(customer_code)
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

    // TODO remove this for security purpose
    dbg!(&props);
    set_prop_values(props);

    let Ok(port) = get_prop_value(SERVER_PORT_PROPERTY).unwrap_or("".to_string()).parse::<u16>() else {
        eprintln!("💣 Cannot read the server port");
        exit(-56);
    };

    dbg!(port);

    let Ok(log_config) = get_prop_value(LOG_CONFIG_FILE_PROPERTY) else {
        eprintln!("💣 Cannot read the log4rs config");
        exit(-57);
    };
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

    let Ok(new_prop) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY) else {
        panic!("💣 Cannot read the cek properties");
    };
    log_info!("The CEK was correctly read : [{}]", format!("{}...", &new_prop[0..5]));

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

    let _ = rocket::custom(my_config)
        .mount(&base_url, routes![set_removable_flag_customer, delete_customer,
            create_customer, login])
        .attach(Template::fairing())
        .launch();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
