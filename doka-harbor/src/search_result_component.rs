use std::io::Cursor;

use bytes::Bytes;
use commons_error::*;
use hyper::StatusCode;
use image::imageops::FilterType;
use image::ImageFormat;
use log::*;
use serde::de::DeserializeOwned;
use serde::{de, Serialize};
use serde_derive::Deserialize;
use tokio::task;

use commons_error::{err_fwd, log_info, log_warn};
use commons_services::session_lib::valid_sid_get_session;
use commons_services::token_lib::SessionToken;
use commons_services::try_or_return;
use commons_services::x_request_id::{Follower, XRequestID};
use dkdto::cbor_type::CborType;
use dkdto::error_codes::INTERNAL_TECHNICAL_ERROR;
use dkdto::{ErrorSet, GetItemReply, WebType, WebTypeBuilder};
use doka_cli::async_request_client::{DocumentServerClientAsync, FileServerClientAsync};
use doka_cli::request_client::TokenType;

use crate::date_tools::{format_date, format_date_in_timezone};
use crate::kv_store::KvStore;
use crate::search_result_model::{
    GetItemReplyForSearchResult, HarborContext, MapToHarbor, SearchResultHarbor,
};

pub(crate) struct SearchResultComponent {
    pub session_token: SessionToken,
    pub follower: Follower,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct CborFile {
    pub file_data: Bytes,
}

impl SearchResultComponent {
    pub fn new(session_token: SessionToken, x_request_id: XRequestID) -> Self {
        Self {
            session_token,
            follower: Follower {
                x_request_id: x_request_id.new_if_null(),
                token_type: TokenType::None,
            },
        }
    }

    fn cbor_type_error<T: de::DeserializeOwned + Serialize>(
    ) -> impl Fn(&ErrorSet<'static>) -> CborType<T>
    where
        T: DeserializeOwned,
    {
        |e| {
            log_error!("💣 Error after try {:?}", e);
            CborType::from_errorset(e)
        }
    }

    /// 🌟 Read a file from the Doka API
    pub async fn get_file(&mut self, file_ref: &str) -> CborType<CborFile> {
        log_info!("🚀 Start the get_file API");

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await, /* => Result of an object or a static ErrorSet*/
            Self::cbor_type_error()
        );

        let micro_trans = "7cf98e6a";

        // let reduced_data = try_or_return!(
        //     Self::smart_fetch_file(&micro_trans, &file_ref).await,
        //     |_| CborType::from_errorset(&INTERNAL_TECHNICAL_ERROR)
        // );

        /* => anyhow::Result<Box<Vec<u8>>> */
        let Ok(reduced_data) = Self::smart_fetch_file(&micro_trans, &file_ref)
            .await
            .map_err(err_fwd!("Cannot fetch file, follower=[{}]", &self.follower))
        else {
            return CborType::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };

        let cbor_file = CborFile {
            file_data: Bytes::from(reduced_data.to_vec()),
        };

        log_info!("🏁 End get_file");

        CborType::from_item(StatusCode::OK.as_u16(), cbor_file)
    }

    /// 🌟 Search for items from the Doka API
    /// The search is based on a session token
    pub async fn search_result(&self) -> CborType<SearchResultHarbor> {
        log_info!("🚀 Start the search result API");

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
                        log_info!(
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
        log_info!("!!! Call the second API, for each items");

        for my_item in &get_item_reply.items {
            if let Some(file_ref) = my_item.file_ref.as_ref() {
                let file_ref_clone = file_ref.clone();
                let file_key = format!("{}-{}-{}", &sid, &micro_trans, &file_ref_clone);
                match file_store.read_from_nats(&file_key).await.unwrap() {
                    None => {
                        let _handle = tokio::spawn(async move {
                            log_info!("Smart fetch file for file ref : {}", &file_ref_clone);
                            let _reduced_data =
                                Self::smart_fetch_file(&micro_trans, &file_ref_clone)
                                    .await
                                    .unwrap();
                        });
                    }
                    Some(data) => {
                        log_info!("Data already in Nats. size {}", data.len())
                    }
                }
            }
        }

        // Transform the API data into something "front"
        log_info!("!!! Transform the API data into something front");
        let context = HarborContext {
            date_format_fn: format_date,
            datetime_format_fn: format_date_in_timezone,
        };
        let harbor_data: SearchResultHarbor = get_item_reply.map_to_harbor(&context);

        let ret = CborType::from_item(StatusCode::OK.as_u16(), harbor_data);

        log_info!("🏁 End search item ");

        ret
    }

    /// 🌟 Get an item from the Doka API
    pub async fn get_item(
        &self, /*, session_token: SessionToken, pattern: String*/
    ) -> CborType<GetItemReplyForSearchResult> {
        log_info!("🚀 Start the get_item API");

        // let query_name = "MY_ITEM";
        let micro_trans = "7cf98e6a";
        let item_id: i64 = 9;
        let sid = "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g";

        let document_bucket = "docs-60";
        let document_key = format!("{}-{}-{}", &sid, &micro_trans, item_id);

        let file_bucket = "files-60";

        // let habor_bucket = "harbor-60";
        // let query_key = format!("{}-{}-{}-{}", &sid, &micro_trans, &query_name, item_id);

        let server_host = "localhost"; // get_prop_value("server.host")?;
        let document_server_port: u16 = 30070; // get_prop_value("ds.port")?.parse()?;

        let document_store = KvStore::new(document_bucket, "0123456789ABCDEF");

        let get_item_reply = match document_store.read_from_nats(&document_key).await.unwrap() {
            None => {
                // Call the first API
                let client = DocumentServerClientAsync::new(&server_host, document_server_port);
                let data = match client.get_item(item_id, &sid).await {
                    Ok(reply) => {
                        log_info!(
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
                            let _reduced_data =
                                Self::smart_fetch_file(&micro_trans, &file_ref_clone)
                                    .await
                                    .unwrap();
                        });
                    }
                    Some(data) => {
                        log_info!("Data already in Nats. size {}", data.len())
                    }
                }
            }
        }

        // Transform the API data into something "front"
        let context = HarborContext {
            date_format_fn: format_date,
            datetime_format_fn: format_date_in_timezone,
        };
        let harbor_data: GetItemReplyForSearchResult = get_item_reply.map_to_harbor(&context);

        let ret = CborType::from_item(StatusCode::OK.as_u16(), harbor_data);
        log_info!("🏁 End get item ");

        ret
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
                log_info!("😎 File downloaded : {}", data.len());
                let r = kv_store
                    .store_to_nats(&file_key, data.to_vec().clone())
                    .await;
                data.to_vec()
            }
            Some(data) => {
                log_warn!("Big File already in Nats. size {}", data.len());
                data
            }
        };

        let file_reduced_key = format!("{}-{}-{}-REDUCED", &sid, &micro_trans, &file_ref);

        let reduced_data = match kv_store.read_from_nats(&file_reduced_key).await.unwrap() {
            None => {
                log_info!("About to resize. Original size {}", raw_data.len());
                // Resizing can be CPU blocking so we run a separate task
                let reduced_data =
                    task::spawn_blocking(move || Self::resize_jpeg(raw_data, 800, 600).unwrap())
                        .await
                        .expect("Task panicked");
                log_info!("Reduction done");

                let r = kv_store
                    .store_to_nats(&file_reduced_key, reduced_data.to_vec().clone())
                    .await;
                log_info!("--> Store resize to nats is done");
                reduced_data
            }
            Some(reduced_data) => {
                log_warn!("Reduced file already in Nats. size {}", reduced_data.len());
                reduced_data
            }
        };
        log_info!("End of smart fetch");
        Ok(Box::new(reduced_data))
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
}
