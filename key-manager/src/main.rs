use std::net::SocketAddr;
use std::process::exit;

use axum::extract::{Json, Path};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::Router;
use log::{error, info};

use commons_error::*;
use commons_pg::sql_transaction::init_db_pool;
use commons_pg::sql_transaction2::{init_db_pool2, SQLConnection2};
use commons_services::property_name::{
    COMMON_EDIBLE_KEY_PROPERTY, LOG_CONFIG_FILE_PROPERTY, SERVER_PORT_PROPERTY,
};
use commons_services::read_cek_and_store;
use commons_services::token_lib::SecurityToken;
use commons_services::x_request_id::XRequestID;
use dkconfig::conf_reader::{read_config, read_doka_env};
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use dkdto::{AddKeyReply, AddKeyRequest, CustomerKeyReply, WebType, WebTypeBuilder};

use crate::key::KeyDelegate;

mod all_tests;
mod key;

///
/// ✨ Read the key for a specific customer code [customer_code]
/// ** NORM
///
// #[get("/key/<customer_code>")]
async fn read_key(
    Path(customer_code): Path<String>,
    security_token: SecurityToken,
) -> WebType<CustomerKeyReply> {
    let mut delegate = KeyDelegate::new(security_token, XRequestID::from_value(None));
    delegate.read_key(&customer_code).await
}

///
/// ✨ Read all the keys
/// ** NORM
///
// #[get("/key")]
async fn key_list(security_token: SecurityToken) -> WebType<CustomerKeyReply> {
    let mut delegate = KeyDelegate::new(security_token, XRequestID::from_value(None));
    delegate.key_list().await
}

///
/// ✨ Add a key for customer code [customer]
/// ** NORM
///
// #[post("/key", format = "application/json", data = "<customer>")]
async fn add_key(
    security_token: SecurityToken,
    customer: Json<AddKeyRequest>,
) -> WebType<AddKeyReply> {
    let mut delegate = KeyDelegate::new(security_token, XRequestID::from_value(None));
    delegate.add_key(customer).await
}

async fn read_toto() -> WebType<CustomerKeyReply> {
    let mut cnx = SQLConnection2::from_pool().await.unwrap();
    let trans = cnx.begin().await.unwrap();
    let customer_key_reply = CustomerKeyReply {
        keys: Default::default(),
    };
    WebType::from_item(StatusCode::OK.as_u16(), customer_key_reply)
}

///
///
///
#[tokio::main]
async fn main() {
    const PROGRAM_NAME: &str = "Key Manager";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "key-manager";
    const VAR_NAME: &str = "DOKA_ENV";

    let doka_env = read_doka_env(&VAR_NAME);

    // Read the application config's file
    println!(
        "😎 Config file using PROJECT_CODE={} VAR_NAME={}",
        PROJECT_CODE, VAR_NAME
    );

    let props = read_config(PROJECT_CODE, &doka_env);
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

    // Init DB pool
    log_info!("😎 Init DB pool");
    let (connect_string, db_pool_size) = match get_prop_pg_connect_string()
        .map_err(err_fwd!("Cannot read the database connection information"))
    {
        Ok(x) => x,
        Err(e) => {
            log_error!("{:?}", e);
            exit(-64);
        }
    };

    let r = init_db_pool2(&connect_string, db_pool_size).await;

    let Ok(cek) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY) else {
        panic!("💣 Cannot read the cek properties");
    };
    log_info!(
        "😎 The CEK was correctly read : [{}]",
        format!("{}...", &cek[0..5])
    );

    log_info!("🚀 Start {} on port {}", PROGRAM_NAME, port);

    // Build our application with some routes
    let base_url = format!("/{}", PROJECT_CODE);
    let key_routes = Router::new()
        .route("/toto", get(read_toto))
        .route("/key", get(key_list))
        .route("/key/:customer_code", get(read_key))
        .route("/key", post(add_key));

    let app = Router::new().nest(&base_url, key_routes);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    log_info!("🏁 End {}", PROGRAM_NAME);
}

#[cfg(test)]
mod test {
    use dkdto::AddKeyReply;
    use dkdto::AddKeyRequest;

    #[test]
    fn http_post_add_key() -> anyhow::Result<()> {
        let customer_code = "denis.zzzzzzz".to_string();
        let token= "j6nk2GaKdfLl3nTPbfWW0C_Tj-MFLrJVS2zdxiIKMZpxNOQGnMwFgiE4C9_cSScqshQvWrZDiPyAVYYwB8zCLRBzd3UUXpwLpK-LMnpqVIs".to_string();

        let new_post = AddKeyRequest { customer_code };

        let reply: AddKeyReply = reqwest::blocking::Client::new()
            .post("http://localhost:30040/key-manager/key")
            .header("token", token.clone())
            .json(&new_post)
            .send()?
            .json()?;

        dbg!(&reply);

        Ok(())
    }
}
