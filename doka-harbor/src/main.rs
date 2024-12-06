use std::future::Future;
use std::io::Cursor;
use std::net::SocketAddr;

use axum::body::Body;
use axum::http::Method;
use axum::{response::IntoResponse, routing::get, Router};
use bytes::Bytes;
use chrono::{DateTime, Datelike, FixedOffset, NaiveDate, Timelike};
use ciborium::ser;
use hyper::header::CONTENT_TYPE;
use hyper::StatusCode;
use image::imageops::FilterType;
use image::{ImageEncoder, ImageFormat};
use log::*;
use serde::Serialize;
use serde_derive::Deserialize;
use tokio::io::AsyncWriteExt;
use tower_http::cors::{Any, CorsLayer};

use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::GetItemReply;
use doka_cli::async_request_client::{DocumentServerClientAsync, FileServerClientAsync};

mod component1;

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

async fn read_from_nats(bucket: &str, key: &str) -> anyhow::Result<Option<Vec<u8>>> {
    // Connect to the NATS server
    let client = async_nats::connect("localhost:4222").await?;
    println!("Connected to NATS for reading, {} {}", bucket, key);

    // Create a JetStream context
    let jetstream = async_nats::jetstream::new(client);

    // Create or access a Key-Value store

    let kv = jetstream.get_key_value(bucket.to_string()).await?;
    println!("Key-Value store '{}' ready", &bucket);

    let hash_key = DkEncrypt::hash_word(key);
    let mut data = Vec::new();
    let mut i = 0;
    loop {
        let key_i = format!("{}-{}", &hash_key, i);

        // Retrieve and print the stored data
        if let Some(entry) = kv.entry(key_i).await? {
            let chunk = entry.value.to_vec();
            data.extend_from_slice(&chunk);
        } else {
            println!("No value found for key '{}'", key);
            break;
        }
        i += 1;
    }

    if i == 0 {
        Ok(None)
    } else {
        Ok(Some(data))
    }
}

async fn store_to_nats(bucket: &str, key: &str, data: Vec<u8>) -> anyhow::Result<()> {
    // Connect to the NATS server
    let client = async_nats::connect("localhost:4222").await?;
    println!("Connected to NATS, {} {}", bucket, key);

    // Create a JetStream context
    let jetstream = async_nats::jetstream::new(client);

    // Create or access a Key-Value store
    let kv = jetstream.get_key_value(bucket.to_string()).await?;
    println!("Key-Value store '{}' ready", &bucket);

    // Define chunk size (1 MB)
    const CHUNK_SIZE: usize = 1 * 1024 * 1024;

    let hash_key = DkEncrypt::hash_word(key);

    // Loop over the data in chunks
    for (i, chunk) in data.chunks(CHUNK_SIZE).enumerate() {
        let key_i = format!("{}-{}", &hash_key, i);
        let d = chunk.to_vec();
        kv.put(&key_i, d.into()).await?;
        println!("Data stored with key '{}', size {}", &key_i, chunk.len());
    }

    Ok(())
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

async fn smart_fetch_file(micro_trans: &str, file_ref: &str) -> anyhow::Result<Box<Vec<u8>>> {
    let file_bucket = "files-60";
    let server_host = "localhost";
    let sid = "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g";
    let file_key = format!("{}-{}-{}", &sid, &micro_trans, &file_ref);

    let raw_data = match read_from_nats(&file_bucket, &file_key).await.unwrap() {
        None => {
            let fs_client = FileServerClientAsync::new(server_host, 30080);
            let r = fs_client.download(&file_ref, &sid).await;
            let data = r.unwrap().data;
            println!("😎 File downloaded : {}", data.len());
            let r = store_to_nats(&file_bucket, &file_key, data.to_vec().clone()).await;
            dbg!(&r);
            data.to_vec()
        }
        Some(data) => {
            println!("Data already in Nats. size {}", data.len());
            data
        }
    };

    let file_reduced_key = format!("{}-{}-{}-REDUCED", &sid, &micro_trans, &file_ref);

    let reduced_data = match read_from_nats(&file_bucket, &file_reduced_key)
        .await
        .unwrap()
    {
        None => {
            let reduced_data = resize_jpeg(raw_data, 1600, 1200).unwrap();
            let r = store_to_nats(
                &file_bucket,
                &file_reduced_key,
                reduced_data.to_vec().clone(),
            )
            .await;
            //dbg!(&r);
            reduced_data
        }
        Some(reduced_data) => {
            //println!("Data already in Nats. size {}", data.len());
            reduced_data
        }
    };

    Ok(Box::new(reduced_data))
}

async fn get_file() -> impl IntoResponse {
    let micro_trans = "7cf98e6a";
    let file_ref = "5aa40f74-284b-43d5-6406-13f0b9bd67e9";
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

////
async fn get_item() -> impl IntoResponse {
    // Call the doka API

    let query_name = "MY_ITEM";
    let micro_trans = "7cf98e6a";
    let item_id: i64 = 9;
    let sid = "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g";

    let document_bucket = "docs-60";
    let document_key = format!("{}-{}-{}", &sid, &micro_trans, item_id);

    let file_bucket = "files-60";

    let habor_bucket = "harbor-60";
    let query_key = format!("{}-{}-{}-{}", &sid, &micro_trans, &query_name, item_id);

    let server_host = "localhost"; // get_prop_value("server.host")?;
    let document_server_port: u16 = 30070; // get_prop_value("ds.port")?.parse()?;

    let get_item_reply = match read_from_nats(&document_bucket, &document_key)
        .await
        .unwrap()
    {
        None => {
            // Call the first API
            let client = DocumentServerClientAsync::new(&server_host, document_server_port);
            let data = match client.get_item(item_id, &sid).await {
                Ok(reply) => {
                    println!(
                        "😎 Item successfully fetch from API, count : {} ",
                        reply.items.len()
                    );
                    reply
                }
                Err(e) => panic!(), /*Err(anyhow!("{} - {}", e.http_error_code, e.message))*/
            };

            // Store the API data in the SQLite database (could be in the API stub)
            // ....
            let s_item = serde_json::to_string(&data).unwrap().into_bytes();
            let r = store_to_nats(&document_bucket, &document_key, s_item).await;
            dbg!(&r);
            data
        }
        Some(d) => serde_json::from_str::<GetItemReply>(&(String::from_utf8(d).unwrap())).unwrap(),
    };

    // Call the second API

    if get_item_reply.items.len() > 0 {
        let my_item = get_item_reply.items.get(0).unwrap();
        if let Some(file_ref) = my_item.file_ref.as_ref() {
            let file_ref_clone = file_ref.clone();
            let sid_clone = sid.to_string();

            let file_key = format!("{}-{}-{}", &sid, &micro_trans, &file_ref_clone);
            match read_from_nats(&file_bucket, &file_key).await.unwrap() {
                None => {
                    let _handle = tokio::spawn(async move {
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
    let context = HarborContext {
        date_format_fn: format_date,
        datetime_format_fn: format_date_in_timezone,
    };
    let harbor_data = get_item_reply.map_to_harbor(&context);

    //

    // Serialize the data to CBOR format
    let mut cbor_data = Vec::new();
    if let Err(err) = ser::into_writer(&harbor_data, &mut cbor_data) {
        eprintln!("Failed to serialize to CBOR: {}", err);
        return axum::response::Response::builder()
            .status(500)
            .body(Body::from("Internal Server Error"))
            .unwrap();
    }

    // Store the CBOR data in the SQLite database
    // ....

    let r = store_to_nats(&habor_bucket, &query_key, cbor_data.clone()).await;
    dbg!(&r);

    // Return the CBOR data with content type
    let ret = (
        StatusCode::OK,
        [(CONTENT_TYPE, "application/cbor")],
        Bytes::from(cbor_data),
    )
        .into_response();

    ret
}

// The handler for the /cbor/get-data endpoint
async fn get_cbor_data() -> impl IntoResponse {
    // Construct the CBOR data
    let response = Document {
        fields: vec![
            "document_id".to_string(),
            "document_name".to_string(),
            "update_datetime".to_string(),
        ],
        records: vec![
            Record {
                id: 9856,
                file_ref: Some("45ef78a".to_string()),
                document_metadata: vec![
                    "gh789".to_string(),
                    "planet.pdf".to_string(),
                    "12 jan, 2024::2024-01-12".to_string(),
                ],
                binary_content: "asdkflakjhdlkjh...".as_bytes().to_owned(),
                tags: vec![MyTag {
                    tag_name: "Science".to_string(),
                    value: "4.569:6.456".to_string(),
                    formatted_value: "4.569:6.456".to_string(),
                }],
            },
            Record {
                id: 15_648,
                file_ref: None,
                document_metadata: vec![
                    "gh790".to_string(),
                    "forest.pdf".to_string(),
                    "13 jan, 2024::2024-01-13".to_string(),
                ],
                binary_content: "troidilkjlsdél...".as_bytes().to_owned(),
                tags: vec![MyTag {
                    tag_name: "Nature".to_string(),
                    value: "4.547:7.876".to_string(),
                    formatted_value: "4.547:7.876".to_string(),
                }],
            },
        ],
    };

    // Serialize the data to CBOR format
    let mut cbor_data = Vec::new();
    if let Err(err) = ser::into_writer(&response, &mut cbor_data) {
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

    ret
}

#[tokio::main]
async fn main() {
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
        .route("/cbor/get-data", get(get_cbor_data))
        .route("/cbor/get_item", get(get_item))
        .route("/cbor/get_file", get(get_file))
        .layer(cors);

    // Start the server.
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
