use std::net::SocketAddr;
use std::process::exit;

use axum::extract::{DefaultBodyLimit, Multipart, Path};
use axum::http::Method;
use axum::routing::{get, post};
use axum::Router;
use log::*;
use tower_http::cors::{Any, CorsLayer};

use commons_error::*;
use commons_pg::sql_transaction_async::init_db_pool_async;
use commons_services::property_name::{LOG_CONFIG_FILE_PROPERTY, SERVER_PORT_PROPERTY};
use commons_services::read_cek_and_store;
use commons_services::token_lib::SessionToken;
use commons_services::x_request_id::XRequestID;
use dkconfig::conf_reader::{read_config, read_doka_env};
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use dkdto::{
    DownloadReply, GetFileInfoReply, GetFileInfoShortReply, ListOfFileInfoReply,
    ListOfUploadInfoReply, UploadReply, WebType,
};

use crate::file_delegate::FileDelegate;

mod file_delegate;

///
/// ✨  Upload the binary content of a file v2
/// item_info : Base64Url encoded information representing a value from the possible target item (for instance, its filename)
///
// #[post("/upload2/<item_info>", data = "<file_data>")]
pub async fn upload(
    session_token: SessionToken,
    Path(item_info): Path<String>,
    mut file_data: Multipart,
) -> WebType<UploadReply> {
    let mut delegate = FileDelegate::new(session_token, XRequestID::from_value(None));
    delegate.upload2(&item_info, &mut file_data).await
}

///
/// ✨ Get the information about the files being loaded
///
// #[get("/loading")]
pub async fn file_loading(session_token: SessionToken) -> WebType<ListOfUploadInfoReply> {
    let mut delegate = FileDelegate::new(session_token, XRequestID::from_value(None));
    delegate.file_loading().await
}

//#[get("/info/<file_ref>")]
pub async fn file_info(
    session_token: SessionToken,
    Path(file_ref): Path<String>,
) -> WebType<Option<GetFileInfoReply>> {
    let mut delegate = FileDelegate::new(session_token, XRequestID::from_value(None));
    delegate.file_info(&file_ref).await
}

///
/// ✨ Get the information about the loading status of a file [file_ref]
///
// #[get("/stats/<file_ref>")]
pub async fn file_stats(
    session_token: SessionToken,
    Path(file_ref): Path<String>,
) -> WebType<GetFileInfoShortReply> {
    let mut delegate = FileDelegate::new(session_token, XRequestID::from_value(None));
    delegate.file_stats(&file_ref).await
}

/// ✨ Get the information about the composition of files [pattern of file_ref]
// #[get("/list/<pattern>")]
pub async fn file_list(
    session_token: SessionToken,
    Path(pattern): Path<String>,
) -> WebType<ListOfFileInfoReply> {
    let mut delegate = FileDelegate::new(session_token, XRequestID::from_value(None));
    delegate.file_list(&pattern).await
}

///
/// ✨  Download the binary content of a file
///
// #[get("/download/<file_ref>")]
pub async fn download(session_token: SessionToken, Path(file_ref): Path<String>) -> DownloadReply {
    // let session_token = SessionToken { 0: "9ARks93f49KdpZ3sPnPYpSRZUOk9shmbQVZKn9If6RQmwi25yGtCN3vCis4JnYxGO46Hf07hDEZc9LFPRW5ncPFCeO-14VyW-Hdq-Q".to_string() };
    let mut delegate = FileDelegate::new(session_token, XRequestID::from_value(None));
    delegate.download(&file_ref).await
}

#[derive(Debug)]
pub struct CORS;

#[tokio::main]
async fn main() {
    const PROGRAM_NAME: &str = "File Server";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "file-server";
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

    let _ = init_db_pool_async(&connect_string, db_pool_size).await;

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

    // Build our application with some routes
    let base_url = format!("/{}", PROJECT_CODE);
    let key_routes = Router::new()
        .route("/upload2/:item_info", post(upload))
        .route("/loading", get(file_loading))
        .route("/info/:file_ref", get(file_info))
        .route("/stats/:file_ref", get(file_stats))
        .route("/list/:pattern", get(file_list))
        // .route("/raw_download/:file_ref", get(raw_download))
        .route("/download/:file_ref", get(download))
        .layer(cors)
        .layer(DefaultBodyLimit::max(usize::MAX));

    let app = Router::new().nest(&base_url, key_routes);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    log_info!("🏁 End {}", PROGRAM_NAME);
}

#[cfg(test)]
mod test {
    // use std::path::Path;
    // use std::process::exit;
    // use commons_pg::{init_db_pool, SQLConnection, SQLTransaction};
    // use commons_services::database_lib::open_transaction;
    // use dkconfig::conf_reader::read_config;
    // use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
    // //use crate::{insert_document_part, parse_content, select_tsvector};
    // use log::{error,info};
    // use commons_error::*;
    // use commons_services::property_name::LOG_CONFIG_FILE_PROPERTY;
    //
    // fn init_test() {
    //     const PROGRAM_NAME: &str = "Test File Server";
    //
    //     println!("😎 Init {}", PROGRAM_NAME);
    //
    //     const PROJECT_CODE: &str = "file-server";
    //     const VAR_NAME: &str = "DOKA_ENV";
    //
    //     // Read the application config's file
    //     println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);
    //
    //     let props = read_config(PROJECT_CODE, VAR_NAME);
    //     set_prop_values(props);
    //
    //     log_info!("🚀 Start {}", PROGRAM_NAME);
    //
    //     let log_config: String = get_prop_value(LOG_CONFIG_FILE_PROPERTY).unwrap();
    //     let log_config_path = Path::new(&log_config);
    //
    //     // Read the global properties
    //     println!("😎 Read log properties from {:?}", &log_config_path);
    //
    //     match log4rs::init_file(&log_config_path, Default::default()) {
    //         Err(e) => {
    //             eprintln!("{:?} {:?}", &log_config_path, e);
    //             exit(-59);
    //         }
    //         Ok(_) => {}
    //     }
    //
    //
    //     // Init DB pool
    //     let (connect_string, db_pool_size) = match get_prop_pg_connect_string()
    //         .map_err(err_fwd!("Cannot read the database connection information")) {
    //         Ok(x) => x,
    //         Err(e) => {
    //             log_error!("{:?}", e);
    //             exit(-64);
    //         }
    //     };
    //
    //     init_db_pool(&connect_string, db_pool_size);
    // }
    //
    // #[test]
    // fn test_parse_content() -> anyhow::Result<()> {
    //     init_test();
    //     let mem_file: Vec<u8> = std::fs::read("C:/Users/denis/wks-poc/tika/big_planet.pdf")?;
    //     let ret = parse_content("0f373b54-5dbb-4c75-98e7-98fd141593dc", mem_file, "f1248fab", "MY_SID")?;
    //     Ok(())
    // }
    //
    //
    // #[test]
    // fn test_compute_tsvector() -> anyhow::Result<()> {
    //     init_test();
    //     let mut r_cnx = SQLConnection2::from_pool().await;
    //     let mut trans = open_transaction(&mut r_cnx)?;
    //     let ret = select_tsvector(&mut trans,Some("french"), "Planète Phase formation cœurs planétaires Phase formation noyaux telluriques moderne")?;
    //     assert_eq!("'coeur':4 'format':3,7 'modern':10 'noyal':8 'phas':2,6 'planet':1 'planetair':5 'tellur':9", ret);
    //     Ok(())
    // }
    //
    // #[test]
    // fn test_insert_document() -> anyhow::Result<()> {
    //     init_test();
    //     let mut r_cnx = SQLConnection2::from_pool().await;
    //     let mut trans = open_transaction(&mut r_cnx)?;
    //
    //     let id = insert_document_part(&mut trans, "0f373b54-5dbb-4c75-98e7-98fd141593dc", 107,
    //                                  "Phase formation cœurs planétaires Phase formation noyaux telluriques moderne",
    //                                  "french", "f1248fab" )?;
    //
    //     trans.commit();
    //     log_info!("ID = [{}]", id);
    //     assert!(id > 0);
    //     Ok(())
    // }
}
