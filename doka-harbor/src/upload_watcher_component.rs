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
use dkdto::api_error::ApiError;
use dkdto::cbor_type::CborType;
use dkdto::error_codes::{INTERNAL_TECHNICAL_ERROR, INVALID_TOKEN};
use dkdto::web_types::{GetItemReply, WebType, WebTypeBuilder};
use doka_cli::async_request_client::{DocumentServerClientAsync, FileServerClientAsync};
use doka_cli::request_client::TokenType;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct ListOfUploadWatchInfoHarbor {
    pub uploading_files: Vec<UploadWatchInfoHarbor>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct UploadWatchInfoHarbor {
    pub file_name: String,
    pub file_ref: String,
    pub percent_of_completion: u32,
}

#[derive(Clone)]
pub(crate) struct UploadWatcherComponent {
    pub session_token: SessionToken,
    pub follower: Follower,
}

impl UploadWatcherComponent {
    pub fn new(session_token: SessionToken, x_request_id: XRequestID) -> Self {
        Self {
            session_token,
            follower: Follower { x_request_id: x_request_id.new_if_null(), token_type: TokenType::None },
        }
    }

    fn cbor_type_error<T: de::DeserializeOwned + Serialize>() -> impl Fn(&ApiError<'static>) -> CborType<T>
    where
        T: DeserializeOwned,
    {
        |e| {
            log_error!("💣 Error after try {:?}", e);
            CborType::from_api_error(e)
        }
    }

    /// 🌟 Look up the file in the Nats store and return it as a CborType
    pub async fn upload_watch_cbor(&self) -> CborType<ListOfUploadWatchInfoHarbor> {
        match self.upload_watch().await {
            Ok(harbor_data) => CborType::from_item(StatusCode::OK.as_u16(), harbor_data),
            Err(error_set) => {
                log_error!("💣 Error in upload_watch_cbor, error_set=[{:?}]", error_set);
                // CborType::from_api_error(error_set.clone())
                // TODO find a way to convert the error_set to a CborType
                panic!()
            }
        }
    }

    pub async fn upload_watch(&self) -> Result<ListOfUploadWatchInfoHarbor, &ApiError> {
        log_info!("🚀 Start the upload_watch API");

        // Call the doka API

        let micro_trans = "7cf98e6a";
        let search_filters = "NONE";
        let sid =
            "no7sunaJVabyGe3-_LkD9inQmrlQYaKhl3v3JCaK4zFiweZSK_YisP6SKEtj3UaIBjO8y1yvOyHFJwHZFRi3EndsOorrVgfENrJu8g";

        let search_key = format!("{}-{}-{}", &sid, &micro_trans, search_filters);
        let file_store = KvStore::new(FILE_BUCKET, "0123456789ABCDEF");

        let server_host = "localhost"; // get_prop_value("server.host")?;
        let file_server_port: u16 = 30080; // get_prop_value("ds.port")?.parse()?;

        // Call the loading API to be aware of the file upload progress
        log_info!("Call the loading API to be aware of the file upload progress");
        let client = FileServerClientAsync::new(&server_host, file_server_port);
        let loading_reply = match client.loading(sid).await {
            Ok(loading_reply) => {
                log_info!("Loading reply: {:?}", loading_reply);
                loading_reply
            }
            Err(e) => {
                log_error!("💣 Error in loading API: {:?}", e);
                return Err(&INTERNAL_TECHNICAL_ERROR);
            }
        };

        let mut uploading_files = Vec::new();
        for loading in loading_reply.list_of_upload_info {
            let harbor_data = UploadWatchInfoHarbor {
                file_name: loading.item_info.to_string(),
                file_ref: loading.file_reference.to_string(),
                percent_of_completion: (loading.encrypted_count as u32 * 100 / loading.total_part as u32) as u32,
            };
            uploading_files.push(harbor_data);
        }

        let list_of_upload_watch_info_harbor = ListOfUploadWatchInfoHarbor { uploading_files };

        // Transform the API data into something "front"
        log_info!("Transform the API data into something front");
        // let context = HarborContext {
        //     date_format_fn: format_date,
        //     datetime_format_fn: format_date_in_timezone,
        // };
        // let harbor_data: SearchResultHarbor = get_item_reply.map_to_harbor(&context);

        log_info!("🏁 End the upload_watch API");

        Ok(list_of_upload_watch_info_harbor)
    }
}
