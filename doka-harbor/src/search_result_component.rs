use axum::body::Body;
use axum::response::IntoResponse;
use bytes::Bytes;
use ciborium::ser;
use hyper::header::CONTENT_TYPE;
use hyper::StatusCode;

use dkdto::GetItemReply;
use doka_cli::async_request_client::DocumentServerClientAsync;

use crate::component1::GetItemReplyForComponent1;
use crate::kv_store::KvStore;
use crate::{format_date, format_date_in_timezone, smart_fetch_file, HarborContext, MapToHarbor};

pub(crate) struct SearchResultComponent {}

impl SearchResultComponent {
    pub fn new() -> Self {
        SearchResultComponent {}
    }

    pub async fn get_item(
        &self, /*, session_token: SessionToken, pattern: String*/
    ) -> impl IntoResponse {
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

        let document_store = KvStore::new(document_bucket, "0123456789ABCDEF");

        let get_item_reply = match document_store.read_from_nats(&document_key).await.unwrap() {
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
                let r = document_store.store_to_nats(&document_key, s_item).await;
                // dbg!(&r);
                data
            }
            Some(d) => {
                serde_json::from_str::<GetItemReply>(&(String::from_utf8(d).unwrap())).unwrap()
            }
        };

        // Call the second API
        let file_store = KvStore::new(file_bucket, "0123456789ABCDEF");

        if get_item_reply.items.len() > 0 {
            let my_item = get_item_reply.items.get(0).unwrap();
            if let Some(file_ref) = my_item.file_ref.as_ref() {
                let file_ref_clone = file_ref.clone();
                let sid_clone = sid.to_string();

                let file_key = format!("{}-{}-{}", &sid, &micro_trans, &file_ref_clone);
                match file_store.read_from_nats(&file_key).await.unwrap() {
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
        let harbor_data: GetItemReplyForComponent1 = get_item_reply.map_to_harbor(&context);

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

        // Return the CBOR data with content type
        let ret = (
            StatusCode::OK,
            [(CONTENT_TYPE, "application/cbor")],
            Bytes::from(cbor_data),
        )
            .into_response();

        ret
    }
}
