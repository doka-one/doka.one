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
use rocket::{Config, routes, post, patch};
use rocket::config::Environment;
use rocket::http::RawStr;
use rocket_contrib::json::Json;
use rocket_contrib::templates::Template;
use commons_error::{err_fwd, err_closure_fwd, log_error, log_info};
use commons_pg::init_db_pool;
use commons_services::read_cek_and_store;
use commons_services::token_lib::SecurityToken;
use dkconfig::conf_reader::read_config;
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use dkdto::{JsonErrorSet, LoginReply, LoginRequest};
use crate::customer::set_removable_flag_customer_delegate;
use crate::login::login_delegate;



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
    login_delegate(login_request)
}


#[patch("/customer/removable/<customer_code>")]
fn set_removable_flag_customer(customer_code: &RawStr, security_token: SecurityToken) -> Json<JsonErrorSet> {
    set_removable_flag_customer_delegate(customer_code, security_token)
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

    let _ = rocket::custom(my_config)
        .mount(&base_url, routes![set_removable_flag_customer, customer::delete_customer,
            customer::create_customer, login])
        .attach(Template::fairing())
        .launch();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
