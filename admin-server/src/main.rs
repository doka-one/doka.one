//! admin-server handles the customer creation and login

use axum::extract::Path;
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use std::net::SocketAddr;
use std::process::exit;

use log::*;

use crate::customer::CustomerDelegate;
use crate::login::LoginDelegate;
use commons_error::{err_closure_fwd, err_fwd, log_error, log_info};
use commons_pg::sql_transaction::init_db_pool;
use commons_services::property_name::{
    COMMON_EDIBLE_KEY_PROPERTY, LOG_CONFIG_FILE_PROPERTY, SERVER_PORT_PROPERTY,
};
use commons_services::read_cek_and_store;
use commons_services::token_lib::SecurityToken;
use commons_services::x_request_id::XRequestID;
use dkconfig::conf_reader::{read_config, read_doka_env};
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use dkdto::{
    CreateCustomerReply, CreateCustomerRequest, LoginReply, LoginRequest, SimpleMessage, WebType,
};

mod customer;
mod dk_password;
mod login;
mod schema_cs;
mod schema_fs;

/// 0️ Login into the system with the provided credentials
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
/// **NORM
///
/// #[post("/login", format = "application/json", data = "<login_request>")]
pub async fn login(login_request: Json<LoginRequest>) -> WebType<LoginReply> {
    // TODO define the cases when a service needs a x_request_id has an entry parameter.
    let delegate = LoginDelegate::new(XRequestID::from_value(None));
    delegate.login(login_request).await
}

///
/// 🔑 Set a flag on a customer to allow its deletion
/// **NORM
///
/// #[patch("/customer/removable/<customer_code>")]
pub async fn set_removable_flag_customer(
    security_token: SecurityToken,
    Path(customer_code): Path<String>,
) -> WebType<SimpleMessage> {
    let delegate = CustomerDelegate::new(security_token, XRequestID::from_value(None));
    delegate.set_removable_flag_customer(&customer_code).await
}

/// 🔑 Create a brand new customer with schema and all
/// **NORM
///
/// #[post("/customer", format = "application/json", data = "<customer_request>")]
pub async fn create_customer(
    security_token: SecurityToken,
    x_request_id: XRequestID,
    customer_request: Json<CreateCustomerRequest>,
) -> WebType<CreateCustomerReply> {
    let delegate = CustomerDelegate::new(security_token, x_request_id);
    delegate.create_customer(customer_request).await
}

/// 🔑 Delete a customer with schema and all
/// **NORM
///
/// #[delete("/customer/<customer_code>")]
pub async fn delete_customer(
    security_token: SecurityToken,
    x_request_id: XRequestID,
    Path(customer_code): Path<String>,
) -> WebType<SimpleMessage> {
    let delegate = CustomerDelegate::new(security_token, x_request_id);
    delegate.delete_customer(&customer_code).await
}

#[tokio::main]
async fn main() {
    const PROGRAM_NAME: &str = "Admin Server";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "admin-server";
    const VAR_NAME: &str = "DOKA_ENV";

    // Read the application config's file
    println!(
        "😎 Config file using PROJECT_CODE={} VAR_NAME={}",
        PROJECT_CODE, VAR_NAME
    );

    let props = read_config(PROJECT_CODE, &read_doka_env(&VAR_NAME));

    set_prop_values(props);

    let Ok(port) = get_prop_value(SERVER_PORT_PROPERTY)
        .unwrap_or("".to_string())
        .parse::<u16>()
    else {
        eprintln!("💣 Cannot read the server port");
        exit(-56);
    };

    let Ok(log_config) = get_prop_value(LOG_CONFIG_FILE_PROPERTY) else {
        eprintln!("💣 Cannot read the log4rs config");
        exit(-57);
    };
    let log_config_path = std::path::Path::new(&log_config);

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

    let Ok(cek) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY) else {
        panic!("💣 Cannot read the cek properties");
    };
    log_info!(
        "😎 The CEK was correctly read : [{}]",
        format!("{}...", &cek[0..5])
    );

    // Init DB pool
    let (connect_string, db_pool_size) = match get_prop_pg_connect_string()
        .map_err(err_fwd!("Cannot read the database connection information"))
    {
        Ok(x) => x,
        Err(e) => {
            log_error!("{:?}", e);
            exit(-64);
        }
    };

    init_db_pool(&connect_string, db_pool_size);

    log_info!("🚀 Start {} on port {}", PROGRAM_NAME, port);

    // Build our application with some routes
    let base_url = format!("/{}", PROJECT_CODE);
    let key_routes = Router::new()
        .route("/login", post(login))
        .route("/customer", post(create_customer))
        .route("/customer/<:customer_code", delete(delete_customer))
        .route(
            "/session//customer/removable/<:customer_code",
            patch(set_removable_flag_customer),
        );

    let app = Router::new().nest(&base_url, key_routes);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
