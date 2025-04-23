use std::env;
use std::net::SocketAddr;
use std::process::exit;

use axum::extract::Path;
use axum::http::Method;
use axum::response::Html;
use axum::{routing::get, Router};
use chrono::Timelike;
use handlebars::Handlebars;
use commons_error::log_info;
use commons_services::read_cek_and_store;
use commons_services::token_lib::SessionToken;
use commons_services::x_request_id::XRequestID;
use dkconfig::conf_reader::{read_config, read_doka_env};
use dkconfig::properties::{get_prop_value, set_prop_values};
use dkconfig::property_name::{COMMON_EDIBLE_KEY_PROPERTY, LOG_CONFIG_FILE_PROPERTY};
use dkdto::cbor_type::CborBytes;
use log::*;
use serde_derive::Serialize;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use crate::search_result_component::SearchResultComponent;

mod buckets;
mod date_tools;
mod kv_store;
mod search_result_component;
mod search_result_model;

/** REF TAG: DOKA_HARBOR */

/// 🌟 GET /get_file/:file_ref
async fn get_file(Path(file_ref): Path<String>) -> CborBytes {
    // let session_token = SessionToken { 0: "".to_string() };
    let session_token = SessionToken { 0: "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g".to_string() };
    let mut delegate = SearchResultComponent::new(session_token, XRequestID::from_value(None));
    delegate.get_file(&file_ref).await.into()
}

/// 🌟 View the original file
///
/// GET /cbor/view_file/:file_ref
async fn view_file(Path(file_ref): Path<String>) -> CborBytes {
    // let session_token = SessionToken { 0: "".to_string() };
    let session_token = SessionToken { 0: "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g".to_string() };
    let mut delegate = SearchResultComponent::new(session_token, XRequestID::from_value(None));
    delegate.view_file(&file_ref).await.into()
}

/// 🌟 End point for the search result component
///
/// GET /cbor/search_result
async fn search_result() -> CborBytes {
    let session_token = SessionToken { 0: "".to_string() };
    let mut delegate = SearchResultComponent::new(session_token, XRequestID::from_value(None));
    delegate.search_result().await.into()
}

#[derive(Serialize)]
struct TemplateData {
    message: String,
}
/// Handler to serve HTML

async fn serve_html() -> Html<String> {
    // Build the data
    let path = env::current_dir().unwrap();
    let data = TemplateData {
        message: format!("Current path is: {}", path.display()),
    };

    // Register and render the template
    let mut hb = Handlebars::new();
    hb.register_template_file("index", "./templates/index.hbs")
        .expect("Failed to load template");

    let rendered = hb.render("index", &data).expect("Failed to render template");

    Html(rendered)
}


/// Main async routine
#[tokio::main(flavor = "multi_thread", worker_threads = 6)]
async fn main() {
    const PROGRAM_NAME: &str = "Harbor";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "harbor";
    const VAR_NAME: &str = "DOKA_ENV";

    // Read the application config's file
    println!(
        "😎 Config file using PROJECT_CODE={} VAR_NAME={}",
        PROJECT_CODE, VAR_NAME
    );

    let props = read_config(
        PROJECT_CODE,
        &read_doka_env(&VAR_NAME),
        &Some("DOKA_CLUSTER_PROFILE".to_string()),
    );
    set_prop_values(props);

    let Ok(port) = get_prop_value("server.port")
        .unwrap_or("".to_string())
        .parse::<u16>()
    else {
        eprintln!("💣 Cannot read the server port");
        exit(056);
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

    log_info!("🚀 Start {} on port {}", PROGRAM_NAME, port);

    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::OPTIONS,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_origin(Any) // You can restrict origins instead of using Any
        .allow_headers(Any);

    // Create the Axum application with the GET route.
    let key_routes = Router::new()
        .route("/cbor/get_file/:file_ref", get(get_file))
        .route("/cbor/view_file/:file_ref", get(view_file))
        .route("/cbor/search_result", get(search_result))
        // TODO below is a test page to serve a static content
        .route("/index2", get(serve_html))
        .nest_service("/static", ServeDir::new("static"))
        .layer(cors);

    let base_url = format!("/{}", PROJECT_CODE);
    let app = Router::new().nest(&base_url, key_routes);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
