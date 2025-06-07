use std::env;
use std::net::SocketAddr;
use std::process::exit;

use axum::extract::Path;
use axum::http::Method;
use axum::response::Html;
use axum::{routing::get, Router};
use bytes::Bytes;
use chrono::Timelike;
use commons_error::log_info;
use commons_services::read_cek_and_store;
use commons_services::token_lib::SessionToken;
use commons_services::x_request_id::XRequestID;
use dkconfig::conf_reader::{read_config, read_doka_env};
use dkconfig::properties::{get_prop_value, set_prop_values};
use dkconfig::property_name::{COMMON_EDIBLE_KEY_PROPERTY, LOG_CONFIG_FILE_PROPERTY};
use dkdto::cbor_type::CborBytes;
use handlebars::Handlebars;
use log::*;
use serde_derive::Serialize;
use tower_http::cors::{Any, CorsLayer};
use tower_http::services::ServeDir;

use crate::search_result_component::SearchResultComponent;
use crate::search_result_model::SearchResultHarbor;
use crate::upload_watcher_component::UploadWatcherComponent;

mod buckets;
mod date_tools;
mod kv_store;
mod search_result_component;
mod search_result_model;
mod upload_watcher_component;

/** REF TAG: DOKA_HARBOR */

/// 🌟 GET /get_file/:file_ref
async fn get_file(Path(file_ref): Path<String>) -> CborBytes {
    // let session_token = SessionToken { 0: "".to_string() };
    let session_token = SessionToken { 0: "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g".to_string() };
    let mut delegate = SearchResultComponent::new(session_token, XRequestID::from_value(None));
    delegate.get_file_cbor(&file_ref).await.into()
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
    delegate.search_result_cbor().await.into()
}

/// 🌟 File upload watcher
///
/// GET /cbor/upload_watch
async fn upload_watch() -> CborBytes {
    let session_token = SessionToken { 0: "".to_string() };
    let mut delegate = UploadWatcherComponent::new(session_token, XRequestID::from_value(None));
    delegate.upload_watch_cbor().await.into()
}

#[derive(Serialize)]
struct TemplateData {
    message: String,
    year: u16,
    items: SearchResultHarbor,
}

#[derive(Serialize, Debug)]
struct ImageData {
    image_base64: String,
}

/// Handler to serve HTML

async fn index_html() -> Html<String> {
    // The web server will serve the HTML files located in all the subdirectories of the "root" directory
    // If you run the harbor program from the "doka-harbor" directory, it will be the root directory

    // Build the data
    let path = env::current_dir().unwrap();

    //dbg!(&path);

    let session_token = SessionToken { 0: "".to_string() };
    let mut delegate = SearchResultComponent::new(session_token, XRequestID::from_value(None));
    let items = delegate.search_result().await.unwrap();

    //dbg!(&items);

    let data = TemplateData {
        message: format!("Current path is: {}", path.display()),
        year: 2023,
        items,
    };

    // Register and render the template
    let mut hb = Handlebars::new();
    hb.register_template_file("index", "./templates/index.hbs")
        .expect("Failed to load template");

    hb.register_template_file("footer", "./templates/footer.hbs")
        .unwrap();

    hb.register_template_file("image_partial", "./templates/image.hbs")
        .unwrap();

    let rendered = hb
        .render("index", &data)
        .expect("Failed to render template");

    Html(rendered)
}

async fn image_html(Path(file_ref): Path<String>) -> Html<String> {
    // The web server will serve the HTML files located in all the subdirectories of the "root" directory
    // If you run the harbor program from the "doka-harbor" directory, it will be the root directory

    let session_token = SessionToken { 0: "".to_string() };
    let mut delegate = SearchResultComponent::new(session_token, XRequestID::from_value(None));
    let image_bytes = delegate.get_file(&file_ref).await.unwrap();

    use base64::engine::general_purpose;
    use base64::Engine;
    let image_base64 = general_purpose::STANDARD.encode(image_bytes);
    let data = ImageData { image_base64 };

    // Register and render the template
    let mut hb = Handlebars::new();
    hb.register_template_file("image", "./templates/image.hbs")
        .expect("Failed to load template");

    let rendered = hb
        .render("image", &data)
        .expect("Failed to render template");

    Html(rendered)
}

async fn item_update_html(Path(item_id): Path<String>) -> Html<String> {
    // The web server will serve the HTML files located in all the subdirectories of the "root" directory
    // If you run the harbor program from the "doka-harbor" directory, it will be the root directory

    // let session_token = SessionToken { 0: "".to_string() };
    // let mut delegate = SearchResultComponent::new(session_token, XRequestID::from_value(None));
    // let image_bytes = delegate.get_file(&file_ref).await.unwrap();

    // use base64::engine::general_purpose;
    // use base64::Engine;
    // let image_base64 = general_purpose::STANDARD.encode(image_bytes);
    // let data = ImageData { image_base64 };

    #[derive(Serialize)]
    struct TemplateData {}
    let data = TemplateData {};

    // Register and render the template
    let mut hb = Handlebars::new();
    hb.register_template_file("item_update", "./templates/item_update.hbs")
        .expect("Failed to load template");

    let rendered = hb
        .render("item_update", &data)
        .expect("Failed to render template");

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
        .route("/cbor/upload_watch", get(upload_watch))
        // TODO below is a test page to serve a static content
        .route("/index2", get(index_html))
        .route("/image/:file_ref", get(image_html))
        .route("/item_update/:item_id", get(item_update_html))
        .nest_service("/static", ServeDir::new("static"))
        .layer(cors);

    let base_url = format!("/{}", PROJECT_CODE);
    let app = Router::new().nest(&base_url, key_routes);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
