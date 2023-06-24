#![feature(proc_macro_hygiene, decl_macro)]
#![feature(let_else)]

mod key;
mod all_tests;

use std::env;
use std::path::Path;
use std::process::exit;
use log::{info, error};
use rocket::*;
use rocket_contrib::json::Json;
use rocket::http::RawStr;
use rocket_contrib::templates::Template;
use rocket::config::Environment;

use dkconfig::conf_reader::{read_config, read_doka_env};
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};

use commons_error::*;
use commons_pg::{init_db_pool};

use commons_services::property_name::{COMMON_EDIBLE_KEY_PROPERTY, LOG_CONFIG_FILE_PROPERTY, SERVER_PORT_PROPERTY};
use commons_services::read_cek_and_store;
use commons_services::token_lib::SecurityToken;
use commons_services::x_request_id::XRequestID;
use dkdto::{AddKeyReply, AddKeyRequest, CustomerKeyReply,};
use crate::key::{KeyDelegate};

///
/// ✨ Read the key for a specific customer code [customer_code]
/// ** NORM
///
#[get("/key/<customer_code>")]
fn read_key(customer_code: &RawStr, security_token: SecurityToken) -> Json<CustomerKeyReply> {
    let mut delegate = KeyDelegate::new(security_token, XRequestID::from_value(None));
    delegate.read_key(customer_code)
}


///
/// ✨ Read all the keys
/// ** NORM
///
#[get("/key")]
fn key_list(security_token: SecurityToken) -> Json<CustomerKeyReply> {
    let mut delegate = KeyDelegate::new(security_token, XRequestID::from_value(None));
    delegate.key_list()
}



///
/// ✨ Add a key for customer code [customer]
/// ** NORM
///
#[post("/key", format = "application/json", data = "<customer>")]
fn add_key(customer: Json<AddKeyRequest>, security_token: SecurityToken) -> Json<AddKeyReply> {
    let mut delegate = KeyDelegate::new(security_token, XRequestID::from_value(None));
    delegate.add_key(customer)
}

///
///
///
fn main() {

    const PROGRAM_NAME: &str = "Key Manager";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "key-manager";
    const VAR_NAME: &str = "DOKA_ENV";

    let doka_env = read_doka_env(&VAR_NAME);

    // Read the application config's file
    println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);

    let props = read_config(PROJECT_CODE, &doka_env);
    set_prop_values(props);

    let Ok(port) = get_prop_value(SERVER_PORT_PROPERTY).unwrap_or("".to_string()).parse::<u16>() else {
        eprintln!("💣 Cannot read the server port");
        exit(-56);
    };
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

    // Init DB pool
    log_info!("😎 Init DB pool");
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

    let Ok(cek) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY) else {
        panic!("💣 Cannot read the cek properties");
    };
    log_info!("😎 The CEK was correctly read : [{}]", format!("{}...", &cek[0..5]));

    let mut my_config = Config::new(Environment::Production);
    my_config.set_port(port);

    let base_url = format!("/{}", PROJECT_CODE);

    let _ = rocket::custom(my_config)
        .mount(&base_url, routes![key_list, add_key, read_key])
        .attach(Template::fairing())
        .launch();

    log_info!("🏁 End {}", PROGRAM_NAME);
}


#[cfg(test)]
mod test {
    use dkdto::AddKeyRequest;
    use dkdto::AddKeyReply;

    #[test]
    fn http_post_add_key() -> anyhow::Result<()> {
        let customer_code = "denis.zzzzzzz".to_string();
        let token= "j6nk2GaKdfLl3nTPbfWW0C_Tj-MFLrJVS2zdxiIKMZpxNOQGnMwFgiE4C9_cSScqshQvWrZDiPyAVYYwB8zCLRBzd3UUXpwLpK-LMnpqVIs".to_string();

        let new_post = AddKeyRequest {
            customer_code,
        };

        let reply: AddKeyReply = reqwest::blocking::Client::new()
            .post("http://localhost:30040/key-manager/key")
            .header("token", token.clone())
            .json(&new_post)
            .send()?.json()?;

        dbg!(&reply);

        Ok(())

    }
}
