use std::future::Future;
use std::io::Cursor;
use std::net::SocketAddr;
use std::process::exit;

use axum::body::Body;
use axum::extract::Path;
use axum::http::Method;
use axum::{response::IntoResponse, routing::get, Router};
use bytes::Bytes;
use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Timelike};
use ciborium::ser;
use commons_error::log_info;
use dkconfig::conf_reader::{read_config, read_doka_env};
use dkconfig::properties::{get_prop_value, set_prop_values};
use dkcrypto::dk_crypto::CypherMode::AES;
use hyper::header::CONTENT_TYPE;
use hyper::StatusCode;
use image::imageops::FilterType;
use image::{ImageEncoder, ImageFormat};
use log::*;
use rayon::prelude::IntoParallelRefIterator;
use serde::Serialize;
use serde_derive::Deserialize;
use tokio::io::AsyncWriteExt;
use tokio::{io, task};
use tower_http::cors::{Any, CorsLayer};

use crate::component1::{GetItemReplyForComponent1, SearchResultHarbor};
use crate::kv_store::KvStore;
use crate::search_result_component::SearchResultComponent;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::GetItemReply;
use doka_cli::async_request_client::{DocumentServerClientAsync, FileServerClientAsync};

mod component1;
mod kv_store;
mod search_result_component;

/** REF TAG: DOKA_HARBOR */

// Define the structure of the CBOR data
#[derive(Serialize)]
struct MyTag {
    tag_name: String,
    value: String,
    formatted_value: String,
}

#[derive(Serialize)]
struct Record {
    id: u64,
    file_ref: Option<String>,
    document_metadata: Vec<String>,
    binary_content: Vec<u8>,
    tags: Vec<MyTag>,
}

#[derive(Serialize)]
struct Document {
    fields: Vec<String>,
    records: Vec<Record>,
}

#[derive(Serialize)]
struct CborFile {
    file_data: Bytes,
}

////

#[derive(Serialize, Deserialize, Debug)]
struct SampleData {
    id: u32,
    name: String,
    value: f64,
}

#[derive(Clone, Debug)]
pub struct HarborContext {
    pub date_format_fn: fn(&str) -> String,
    pub datetime_format_fn: fn(&str, i32) -> String,
}

fn format_date(iso_date: &str) -> String {
    // Parse the ISO date and format it to "DD Month YYYY"
    let date = NaiveDate::parse_from_str(iso_date, "%Y-%m-%d");
    match date {
        Ok(d) => format!("{} {} {}", d.day(), d.month(), d.year()),
        Err(_) => "Invalid date".to_string(),
    }
}

fn format_date_in_timezone(iso_date_time: &str, timezone_offset: i32) -> String {
    // Parse the ISO 8601 date string into a DateTime object
    let dt = DateTime::parse_from_rfc3339(iso_date_time).expect("Invalid ISO date");

    // Apply the desired timezone offset (in hours)
    let offset = FixedOffset::east(timezone_offset * 3600); // timezone_offset is in hours
    let dt_in_timezone = dt.with_timezone(&offset);

    // Extract the components of the formatted date
    let month = dt_in_timezone.month0() + 1; // months in chrono are 0-indexed
    let day = dt_in_timezone.day();
    let year = dt_in_timezone.year();
    let hour = dt_in_timezone.hour();
    let minute = dt_in_timezone.minute();

    // Format the date part (e.g., "October, 12th, 2024")
    let month_name = dt_in_timezone.format("%B").to_string(); // Full month name
    let day_suffix = match day {
        1 | 21 | 31 => "st",
        2 | 22 => "nd",
        3 | 23 => "rd",
        _ => "th",
    };

    // Format the time part (e.g., "6:51")
    let time_formatted = format!("{:02}:{:02}", hour, minute);

    // Final formatted output
    format!(
        "{}, {}{}, {}  {}",
        month_name, day, day_suffix, year, time_formatted
    )
}

trait MapToHarbor<T> {
    fn map_to_harbor(&self, context: &HarborContext) -> T;
}

fn resize_jpeg(
    raw_data: Vec<u8>,
    new_width: u32,
    new_height: u32,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Load the raw JPEG data into a DynamicImage
    let img = image::load_from_memory(&raw_data)?;
    // Resize the image using a filter (e.g., Lanczos3 for high quality)
    let resized_img = img.resize(new_width, new_height, FilterType::Lanczos3);
    // Create a buffer to hold the resized JPEG
    let mut resized_raw_data = Vec::new();
    // Write the resized image back to JPEG format
    resized_img.write_to(&mut Cursor::new(&mut resized_raw_data), ImageFormat::Jpeg)?;
    Ok(resized_raw_data)
}

/// Get a reduced binary content from either
/// * the Doka API after reducing the raw image
/// * the storage   
async fn smart_fetch_file(micro_trans: &str, file_ref: &str) -> anyhow::Result<Box<Vec<u8>>> {
    let file_bucket = "files-60";
    let server_host = "localhost";
    let sid = "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g";
    let file_key = format!("{}-{}-{}", &sid, &micro_trans, &file_ref);

    let kv_store = KvStore::new(file_bucket, "0123456789ABCDEF");

    let raw_data = match kv_store.read_from_nats(&file_key).await.unwrap() {
        None => {
            let fs_client = FileServerClientAsync::new(server_host, 30080);
            let r = fs_client.download(&file_ref, &sid).await;
            let data = r.unwrap().data;
            println!("😎 File downloaded : {}", data.len());
            let r = kv_store
                .store_to_nats(&file_key, data.to_vec().clone())
                .await;
            data.to_vec()
        }
        Some(data) => {
            println!("Big File already in Nats. size {}", data.len());
            data
        }
    };

    let file_reduced_key = format!("{}-{}-{}-REDUCED", &sid, &micro_trans, &file_ref);

    let reduced_data = match kv_store.read_from_nats(&file_reduced_key).await.unwrap() {
        None => {
            println!("About to resize. Original size {}", raw_data.len());
            // Resizing can be CPU blocking so we run a separate task
            let reduced_data =
                task::spawn_blocking(move || resize_jpeg(raw_data, 800, 600).unwrap())
                    .await
                    .expect("Task panicked");
            println!("Reduction done");

            let r = kv_store
                .store_to_nats(&file_reduced_key, reduced_data.to_vec().clone())
                .await;
            println!("--> Store resize to nats is done");
            reduced_data
        }
        Some(reduced_data) => {
            println!("Reduced file already in Nats. size {}", reduced_data.len());
            reduced_data
        }
    };
    println!("End of smart fetch");
    Ok(Box::new(reduced_data))
}

/// GET /get_file/:file_ref
async fn get_file(Path(file_ref): Path<String>) -> impl IntoResponse {
    println!("!!! Start the get_file API");
    let micro_trans = "7cf98e6a";
    // let file_ref = "5aa40f74-284b-43d5-6406-13f0b9bd67e9";
    let sid = "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g";

    let reduced_data = smart_fetch_file(&micro_trans, &file_ref).await.unwrap();

    let cbor_file = CborFile {
        file_data: Bytes::from(reduced_data.to_vec()),
    };

    // Serialize the data to CBOR format
    let mut cbor_data = Vec::new();
    if let Err(err) = ser::into_writer(&cbor_file, &mut cbor_data) {
        eprintln!("Failed to serialize to CBOR: {}", err);
        return axum::response::Response::builder()
            .status(500)
            .body(Body::from("Cbor Serialisation Error"))
            .unwrap();
    }

    println!("CBOR image : size {}", cbor_data.len());

    // Return the CBOR data with content type
    let ret = (
        StatusCode::OK,
        [(CONTENT_TYPE, "application/cbor")],
        Bytes::from(cbor_data),
    )
        .into_response();

    ret
}

/// End point for the search result component
async fn search_result() -> impl IntoResponse {
    println!("!!! Start the search result API");
    io::stdout().flush().await.expect("Failed to flush stdout");

    // Call the doka API

    let micro_trans = "7cf98e6a";
    let search_filters = "NONE";
    let sid = "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g";

    let document_bucket = "docs-60";
    let search_key = format!("{}-{}-{}", &sid, &micro_trans, search_filters);

    let file_bucket = "files-60";
    let file_store = KvStore::new(file_bucket, "0123456789ABCDEF");

    let server_host = "localhost"; // get_prop_value("server.host")?;
    let document_server_port: u16 = 30070; // get_prop_value("ds.port")?.parse()?;

    let document_store = KvStore::new(document_bucket, "0123456789ABCDEF");

    let get_item_reply = match document_store.read_from_nats(&search_key).await.unwrap() {
        None => {
            // Call the first API
            let client = DocumentServerClientAsync::new(&server_host, document_server_port);
            let items = match client.search_item(&sid).await {
                Ok(reply) => {
                    println!(
                        "😎 Item successfully fetch from API, count : {} ",
                        reply.items.len()
                    );
                    reply
                }
                Err(e) => panic!(), /*Err(anyhow!("{} - {}", e.http_error_code, e.message))*/
            };

            // Store the API data, in JSON format, in the storage
            // ....
            let binary_json = serde_json::to_string(&items).unwrap().into_bytes();
            let r = document_store.store_to_nats(&search_key, binary_json).await;
            // dbg!(&r);
            items
        }
        Some(binary_json) => {
            serde_json::from_str::<GetItemReply>(&(String::from_utf8(binary_json).unwrap()))
                .unwrap()
        }
    };

    // Call the second API, for each items
    println!("!!! Call the second API, for each items");

    for my_item in &get_item_reply.items {
        if let Some(file_ref) = my_item.file_ref.as_ref() {
            let file_ref_clone = file_ref.clone();
            let file_key = format!("{}-{}-{}", &sid, &micro_trans, &file_ref_clone);
            match file_store.read_from_nats(&file_key).await.unwrap() {
                None => {
                    let _handle = tokio::spawn(async move {
                        println!("Smart fetch file for file ref : {}", &file_ref_clone);
                        let _reduced_data = smart_fetch_file(&micro_trans, &file_ref_clone)
                            .await
                            .unwrap();
                    });
                }
                Some(data) => {
                    println!("Data already in Nats. size {}", data.len())
                }
            }
        }
    }

    // Transform the API data into something "front"
    println!("!!! Transform the API data into something front");
    let context = HarborContext {
        date_format_fn: format_date,
        datetime_format_fn: format_date_in_timezone,
    };
    let harbor_data: SearchResultHarbor = get_item_reply.map_to_harbor(&context);

    // Serialize the data to CBOR format
    println!("!!! Serialize the data to CBOR format ");
    let mut cbor_data = Vec::new();
    if let Err(err) = ser::into_writer(&harbor_data, &mut cbor_data) {
        eprintln!("Failed to serialize to CBOR: {}", err);
        return axum::response::Response::builder()
            .status(500)
            .body(Body::from("Internal Server Error"))
            .unwrap();
    }

    // Return the CBOR data with content type
    let ret = (
        StatusCode::OK,
        [(CONTENT_TYPE, "application/cbor")],
        Bytes::from(cbor_data),
    )
        .into_response();

    println!("!!! End search item ");

    ret
}

/// TODO manage the return value
///
async fn get_item() -> impl IntoResponse {
    let mut delegate = SearchResultComponent::new(/*session_token, XRequestID::from_value(None)*/);
    delegate.get_item().await
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

    let props = read_config(PROJECT_CODE, &read_doka_env(&VAR_NAME));
    set_prop_values(props);

    let Ok(port) = get_prop_value("server.port")
        .unwrap_or("".to_string())
        .parse::<u16>()
    else {
        eprintln!("💣 Cannot read the server port");
        exit(056);
    };

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
    let app = Router::new()
        .route("/cbor/get_item", get(get_item))
        .route("/cbor/get_file/:file_ref", get(get_file))
        .route("/cbor/search_result", get(search_result))
        .layer(cors);

    // Start the server.
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
