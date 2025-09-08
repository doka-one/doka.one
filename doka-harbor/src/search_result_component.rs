use bytes::Bytes;
use commons_error::*;
use hyper::StatusCode;
use image::imageops::FilterType;
use image::ImageFormat;
use log::*;
use serde::de::DeserializeOwned;
use serde::{de, Serialize};
use serde_derive::Deserialize;
use std::io::Cursor;
use std::ops::Deref;
use tokio::task;

use crate::buckets::{DOC_BUCKET, FILE_BUCKET};
use crate::date_tools::{format_date, format_date_in_timezone};
use crate::kv_store::KvStore;
use crate::search_result_model::{GetItemReplyForSearchResult, HarborContext, MapToHarbor, SearchResultHarbor};
use commons_error::{err_fwd, log_info, log_warn};
use commons_services::session_lib::valid_sid_get_session;
use commons_services::token_lib::SessionToken;
use commons_services::try_or_return;
use commons_services::x_request_id::{Follower, XRequestID};
use dkdto::cbor_type::CborType;
use dkdto::error_codes::{INTERNAL_TECHNICAL_ERROR, INVALID_TOKEN};
use dkdto::{ErrorSet, GetItemReply, WebType, WebTypeBuilder};
use doka_cli::async_request_client::{DocumentServerClientAsync, FileServerClientAsync};
use doka_cli::request_client::TokenType;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct CborFile {
    pub file_data: Bytes,
}

#[derive(Clone)]
pub(crate) struct SearchResultComponent {
    pub session_token: SessionToken,
    pub follower: Follower,
}

impl SearchResultComponent {
    pub fn new(session_token: SessionToken, x_request_id: XRequestID) -> Self {
        Self {
            session_token,
            follower: Follower { x_request_id: x_request_id.new_if_null(), token_type: TokenType::None },
        }
    }

    fn cbor_type_error<T: de::DeserializeOwned + Serialize>() -> impl Fn(&ErrorSet<'static>) -> CborType<T>
    where
        T: DeserializeOwned,
    {
        |e| {
            log_error!("💣 Error after try {:?}", e);
            CborType::from_errorset(e)
        }
    }

    fn web_type_error<T: de::DeserializeOwned + Serialize>() -> impl Fn(&ErrorSet<'static>) -> WebType<T>
    where
        T: DeserializeOwned,
    {
        |e| {
            log_error!("💣 Error after try {:?}", e);
            WebType::from_errorset(e)
        }
    }

    /// 🌟 Read the original file from the Doka API or the storage
    pub async fn view_file(&mut self, file_ref: &str) -> CborType<CborFile> {
        log_info!("🚀 Start the view_file API");

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::cbor_type_error()
        );

        let micro_trans = "7cf98e6a";

        let Ok(reduced_data) = self.smart_fetch_original_file(&micro_trans, &file_ref).await.map_err(err_fwd!(
            "Cannot fetch file, file_ref=[{}], follower=[{}]",
            &file_ref,
            &self.follower
        )) else {
            return CborType::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };

        let cbor_file = CborFile { file_data: Bytes::from(reduced_data.to_vec()) };

        log_info!("🏁 End the view_file API");

        CborType::from_item(StatusCode::OK.as_u16(), cbor_file)
    }

    /// 🌟 Read the original file from the Doka API or the storage
    pub async fn view_file_json(&mut self, file_ref: &str) -> WebType<CborFile> {
        log_info!("🚀 Start the view_file API");

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        let micro_trans = "7cf98e6a";

        let Ok(reduced_data) = self.smart_fetch_original_file(&micro_trans, &file_ref).await.map_err(err_fwd!(
            "Cannot fetch file, file_ref=[{}], follower=[{}]",
            &file_ref,
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };

        let cbor_file = CborFile { file_data: Bytes::from(reduced_data.to_vec()) };

        log_info!("🏁 End the view_file API");

        WebType::from_item(StatusCode::OK.as_u16(), cbor_file)
    }

    /// 🌟 Read a file from the Doka API
    pub async fn get_file(&mut self, file_ref: &str) -> Result<Bytes, &ErrorSet> {
        log_info!("🚀 Start the get_file API");

        // TODO check the session token
        fn my_type_error<T: de::DeserializeOwned + Serialize>(
        ) -> impl Fn(&ErrorSet<'static>) -> Result<T, &'static ErrorSet<'static>>
        where
            T: DeserializeOwned,
        {
            |e| {
                log_error!("💣 Error after try {:?}", e);
                Err(&INVALID_TOKEN)
            }
        }

        // let entry_session = try_or_return!(
        //     valid_sid_get_session(&self.session_token, &mut self.follower).await,
        //     my_type_error()
        // );

        let micro_trans = "7cf98e6a";

        let Ok(reduced_data) = self
            .smart_fetch_reduced_file(&micro_trans, &file_ref)
            .await
            .map_err(err_fwd!("Cannot fetch file, follower=[{}]", &self.follower))
        else {
            return Err(&INTERNAL_TECHNICAL_ERROR);
        };

        let file_data = Bytes::from(reduced_data.to_vec());

        log_info!("🏁 End the get_file API");

        Ok(file_data)
    }

    /// 🌟 Read a file from the Doka API
    pub async fn get_file_cbor(&mut self, file_ref: &str) -> CborType<CborFile> {
        log_info!("🚀 Start the get_file API");

        let r_file_data = self.get_file(file_ref).await;

        match r_file_data {
            Ok(file_data) => {
                let cbor_file = CborFile { file_data };
                log_info!("🏁 End the get_file API");
                CborType::from_item(StatusCode::OK.as_u16(), cbor_file)
            }
            Err(error_set) => {
                log_error!("💣 Error in get_file_cbor, error_set=[{:?}]", error_set);
                // CborType::from_errorset(error_set.clone())
                // TODO find a way to convert the error_set to a CborType
                panic!()
            }
        }

        // log_info!("🏁 End the get_file API");

        // CborType::from_item(StatusCode::OK.as_u16(), cbor_file)
    }

    /// 🌟 Read a file from the Doka API
    pub async fn get_file_json(&mut self, file_ref: &str) -> WebType<CborFile> {
        log_info!("🚀 Start the get_file API");

        let r_file_data = self.get_file(file_ref).await;

        match r_file_data {
            Ok(file_data) => {
                let cbor_file = CborFile { file_data };
                log_info!("🏁 End the get_file API");
                WebType::from_item(StatusCode::OK.as_u16(), cbor_file)
            }
            Err(error_set) => {
                log_error!("💣 Error in get_file_cbor, error_set=[{:?}]", error_set);
                // CborType::from_errorset(error_set.clone())
                // TODO find a way to convert the error_set to a CborType
                panic!()
            }
        }

        // log_info!("🏁 End the get_file API");

        // CborType::from_item(StatusCode::OK.as_u16(), cbor_file)
    }

    /// 🌟 Search for the entities from the Doka API
    pub async fn search_result(&self) -> Result<SearchResultHarbor, &ErrorSet> {
        log_info!("🚀 Start the search_result API");

        // Call the doka API

        let micro_trans = "7cf98e6a";
        let search_filters = "NONE";
        let sid =
            "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g";

        let search_key = format!("{}-{}-{}", &sid, &micro_trans, search_filters);
        let file_store = KvStore::new(FILE_BUCKET, "0123456789ABCDEF");

        let server_host = "localhost"; // get_prop_value("server.host")?;
        let document_server_port: u16 = 30070; // get_prop_value("ds.port")?.parse()?;

        let document_store = KvStore::new(DOC_BUCKET, "0123456789ABCDEF");

        let Ok(o_original_file) = document_store
            .read_from_nats(&search_key)
            .await
            .map_err(err_fwd!("Cannot fetch the original file, follower=[{}]", &self.follower))
        else {
            return Err(&INTERNAL_TECHNICAL_ERROR);
        };

        let get_item_reply = match o_original_file {
            None => {
                // Call the first API
                let client = DocumentServerClientAsync::new(&server_host, document_server_port);

                let Ok(get_item_reply) = client
                    .search_item(&sid)
                    .await
                    .map_err(err_fwd!("💣 Cannot fetch the original file, follower=[{}]", &self.follower))
                else {
                    return Err(&INTERNAL_TECHNICAL_ERROR);
                };

                log_info!("😎 Item successfully fetch from API, count : {} ", get_item_reply.items.len());

                // Store the API data, in JSON format, in the storage
                let binary_json = serde_json::to_string(&get_item_reply).unwrap().into_bytes();
                let _ = document_store
                    .store_to_nats(&search_key, binary_json)
                    .await
                    .map_err(err_fwd!("💣 Cannot store the original file, follower=[{}]", &self.follower));
                get_item_reply
            }
            Some(binary_json) => {
                serde_json::from_str::<GetItemReply>(&(String::from_utf8(binary_json).unwrap())).unwrap()
            }
        };

        // Call the second API, for each items
        log_info!("Call the file API, for each items, in parallel");

        for my_entity in &get_item_reply.items {
            if let Some(file_ref) = my_entity.file_ref.as_ref() {
                let file_ref_clone = file_ref.clone();
                let file_key = format!("{}-{}-{}", &sid, &micro_trans, &file_ref_clone);
                match file_store.read_from_nats(&file_key).await.unwrap() {
                    None => {
                        let self_clone = self.clone();
                        let _handle = tokio::spawn(async move {
                            log_info!("Smart fetch file for file ref : {}", &file_ref_clone);
                            let _reduced_data =
                                self_clone.smart_fetch_reduced_file(&micro_trans, &file_ref_clone).await.unwrap();
                        });
                    }
                    Some(data) => {
                        log_info!("Data already in Nats. size {}", data.len())
                    }
                }
            }
        }

        // Transform the API data into something "front"
        log_info!("Transform the API data into something front");
        let context = HarborContext { date_format_fn: format_date, datetime_format_fn: format_date_in_timezone };
        let harbor_data: SearchResultHarbor = get_item_reply.map_to_harbor(&context);

        log_info!("🏁 End the search_result API");

        Ok(harbor_data)
    }

    /// 🌟 Search for the entities from the Doka API
    /// - The search is based on a session token
    pub async fn search_result_cbor(&self) -> CborType<SearchResultHarbor> {
        let r = self.search_result().await;
        match r {
            Ok(harbor_data) => CborType::from_item(StatusCode::OK.as_u16(), harbor_data),
            Err(error_set) => {
                log_error!("💣 Error in search_result_cbor, error_set=[{:?}]", error_set);
                // CborType::from_errorset(error_set.clone())
                // TODO find a way to convert the error_set to a CborType
                panic!()
            }
        }
    }

    /// 🌟 Search for the entities from the Doka API
    /// - The search is based on a session token
    pub async fn search_result_json(&self) -> WebType<SearchResultHarbor> {
        let r = self.search_result().await;
        match r {
            Ok(harbor_data) => WebType::from_item(StatusCode::OK.as_u16(), harbor_data),
            Err(error_set) => {
                log_error!("💣 Error in search_result_cbor, error_set=[{:?}]", error_set);
                // CborType::from_errorset(error_set.clone())
                // TODO find a way to convert the error_set to a CborType
                panic!()
            }
        }
    }

    async fn smart_fetch_original_file(&self, micro_trans: &str, file_ref: &str) -> anyhow::Result<Box<Vec<u8>>> {
        let server_host = "localhost";
        let sid =
            "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g";
        let file_key = format!("{}-{}-{}", &sid, &micro_trans, &file_ref);

        let kv_store = KvStore::new(FILE_BUCKET, "0123456789ABCDEF");

        let o_raw_data = kv_store.read_from_nats(&file_key).await.map_err(err_fwd!("Cannot fetch file"))?;

        let raw_data = match o_raw_data {
            None => {
                let fs_client = FileServerClientAsync::new(server_host, 30080);
                let r = fs_client.download(&file_ref, &sid).await;
                let data = r.unwrap().data;
                log_info!("😎 File downloaded : {}", data.len());
                let _ = kv_store.store_to_nats(&file_key, data.to_vec().clone()).await.map_err(err_fwd!(
                    "Cannot store file, file_ref : [{}], follower=[{}]",
                    &file_ref,
                    &self.follower
                ))?;
                data.to_vec()
            }
            Some(data) => {
                log_warn!("Big File already in Nats. size {}", data.len());
                data
            }
        };

        Ok(Box::new(raw_data))
    }

    /// Get a reduced binary content from either
    /// * the Doka API after reducing the raw image
    /// * the storage   
    async fn smart_fetch_reduced_file(&self, micro_trans: &str, file_ref: &str) -> anyhow::Result<Box<Vec<u8>>> {
        let sid =
            "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g";

        let kv_store = KvStore::new(FILE_BUCKET, "0123456789ABCDEF");
        let file_reduced_key = format!("{}-{}-{}-REDUCED", &sid, &micro_trans, &file_ref);

        let reduced_data = match kv_store.read_from_nats(&file_reduced_key).await.map_err(err_fwd!(
            "Cannot fetch file, file_ref=[{}], follower=[{}]",
            &file_ref,
            &self.follower
        ))? {
            None => {
                let raw_data = self.smart_fetch_original_file(micro_trans, file_ref).await?.to_vec();

                log_info!("About to resize. Original size {}", raw_data.len());
                // Resizing can be CPU blocking so we run a separate task
                let follower_clone = self.follower.clone();
                let reduced_data = task::spawn_blocking(move || match Self::resize_jpeg(raw_data, 800, 600) {
                    Ok(reduced_data) => reduced_data,
                    Err(e) => {
                        log_error!("Cannot resize, follower=[{}], error=[{}]", &follower_clone, e);
                        vec![]
                    }
                })
                .await
                .expect("Task panicked");
                log_info!("Reduction done");

                let r = kv_store.store_to_nats(&file_reduced_key, reduced_data.to_vec().clone()).await;
                log_info!("Store resize to nats is done");
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

    fn resize_jpeg(raw_data: Vec<u8>, new_width: u32, new_height: u32) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
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
