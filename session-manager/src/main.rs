use std::net::SocketAddr;
use std::process::exit;

use axum::extract::Path;
use axum::routing::{get, post};
use axum::{Json, Router};
use log::*;

use commons_error::*;
use commons_pg::sql_transaction_async::init_db_pool_async;
use commons_services::read_cek_and_store;
use commons_services::token_lib::SecurityToken;
use commons_services::x_request_id::XRequestID;
use common_config::conf_reader::{read_config, read_env};
use common_config::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use common_config::property_name::{COMMON_EDIBLE_KEY_PROPERTY, LOG_CONFIG_FILE_PROPERTY, SERVER_PORT_PROPERTY};
use dkdto::web_types::{OpenSessionReply, OpenSessionRequest, SessionReply, WebType};

use crate::session::SessionDelegate;

mod session;

///
/// 🔑 Find a session from its sid
///
//#[get("/session/<session_id>")]
async fn read_session(
    Path(session_id): Path<String>,
    security_token: SecurityToken,
    x_request_id: XRequestID,
) -> WebType<SessionReply> {
    let mut delegate = SessionDelegate::new(security_token, x_request_id);
    delegate.read_session(&session_id).await
}

///
/// 🔑 Open a new session for the group and user
/// It's usually called by the Login end point using the session_id as a security_token
///
//#[post("/session", format = "application/json", data = "<session_request>")]
async fn open_session(
    security_token: SecurityToken,
    x_request_id: XRequestID,
    session_request: Json<OpenSessionRequest>,
) -> WebType<OpenSessionReply> {
    let mut delegate = SessionDelegate::new(security_token, x_request_id);
    delegate.open_session(session_request).await
}

///
#[tokio::main]
async fn main() {
    const PROGRAM_NAME: &str = "Session Manager";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "session-manager";
    const VAR_NAME: &str = "DOKA_ENV";

    // Read the application config's file
    println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);

    let props = read_config(PROJECT_CODE, &read_env(&VAR_NAME), &Some("DOKA_CLUSTER_PROFILE".to_string()));

    set_prop_values(props);

    let Ok(port) = get_prop_value(SERVER_PORT_PROPERTY).unwrap_or("".to_string()).parse::<u16>() else {
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
    log_info!("😎 The CEK was correctly read : [{}]", format!("{}...", &cek[0..5]));

    // Init DB pool
    let (connect_string, db_pool_size) =
        match get_prop_pg_connect_string().map_err(err_fwd!("Cannot read the database connection information")) {
            Ok(x) => x,
            Err(e) => {
                log_error!("{:?}", e);
                exit(-64);
            }
        };

    let _ = init_db_pool_async(&connect_string, db_pool_size).await;

    log_info!("🚀 Start {} on port {}", PROGRAM_NAME, port);

    // Build our application with some routes
    let base_url = format!("/{}", PROJECT_CODE);
    let key_routes =
        Router::new().route("/session/:session_id", get(read_session)).route("/session", post(open_session));

    let app = Router::new().nest(&base_url, key_routes);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
