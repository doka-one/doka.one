use std::fmt::Display;
use std::time::Duration;

use anyhow::anyhow;
use commons_error::*;
use log::*;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use reqwest::{multipart, Client, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::{de, Serialize};
use url::Url;

use dkdto::error_codes::{HTTP_CLIENT_ERROR, INTERNAL_TECHNICAL_ERROR, URL_PARSING_ERROR};
use dkdto::{
    AddItemReply, AddItemRequest, AddItemTagReply, AddItemTagRequest, AddKeyReply, AddKeyRequest, AddTagReply,
    AddTagRequest, CustomerKeyReply, DeleteFullTextRequest, FullTextReply, FullTextRequest, GetFileInfoReply,
    GetFileInfoShortReply, GetItemReply, GetTagReply, ListOfFileInfoReply, ListOfUploadInfoReply, MediaBytes,
    OpenSessionReply, OpenSessionRequest, SessionReply, SimpleMessage, TikaMeta, TikaParsing, UploadReply, WebResponse,
    WebTypeBuilder,
};

use crate::request_client::TokenType::{Sid, Token};
use crate::request_client::{CustomHeaders, TokenType};

const TIMEOUT: Duration = Duration::from_secs(60 * 60);
// const MAX_HTTP_RETRY: u32 = 5;
// const LAPS: u32 = 2_000;

pub struct KeyManagerClientAsync {
    server: WebServerAsync,
}

impl KeyManagerClientAsync {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self { server: WebServerAsync::new(server_name, port, "key-manager") }
    }

    ///
    /// It is not supposed to return an error, so let's return the Reply directly
    ///
    pub async fn add_key(&self, request: &AddKeyRequest, token: &TokenType) -> WebResponse<AddKeyReply> {
        //let url = format!("http://{}:{}/{}/key", &self.server.server_name, self.server.port, self.server.context);
        let url = self.server.build_url("key");

        let headers = CustomHeaders { token_type: token.clone(), x_request_id: None, cek: None };

        self.server.post_data_retry(&url, request, &headers).await
    }

    pub async fn get_key(&self, customer_code: &str, token: &str) -> WebResponse<CustomerKeyReply> {
        // http://localhost:{{PORT}}/key-manager/key/f1248fab
        let url = self.server.build_url_with_refcode("key", customer_code);
        self.server.get_data_retry(&url, &Token(token.to_string())).await
    }
}

#[derive(Clone)]
pub struct SessionManagerClientAsync {
    server: WebServerAsync,
}

impl SessionManagerClientAsync {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self { server: WebServerAsync::new(server_name, port, "session-manager") }
    }

    pub async fn open_session(
        &self,
        request: &OpenSessionRequest,
        token: &str,
        x_request_id: Option<u32>,
    ) -> WebResponse<OpenSessionReply> {
        //let url = format!("http://{}:{}/session-manager/session", &self.server.server_name, self.server.port);
        let url = self.server.build_url("session");

        let headers = CustomHeaders { token_type: Token(token.to_string()), x_request_id, cek: None };

        self.server.post_data_retry(&url, request, &headers).await
    }

    pub async fn get_session(&self, sid: &str, token: &str) -> WebResponse<SessionReply> {
        // let url = format!("http://{}:{}/session-manager/session/{}", &self.server.server_name, self.server.port,
        //                   Uri::percent_encode(sid) );
        let url = self.server.build_url_with_refcode("session", utf8_percent_encode(sid, NON_ALPHANUMERIC).to_string());
        self.server.get_data_retry(&url, &Token(token.to_string())).await
    }
}

///
/// Document Server
///
pub struct DocumentServerClientAsync {
    server: WebServerAsync,
}

impl DocumentServerClientAsync {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self { server: WebServerAsync::new(server_name, port, "document-server") }
    }

    pub fn read_info(&self) -> (String, u16) {
        (self.server.server_name.clone(), self.server.port)
    }

    pub async fn create_item(&self, request: &AddItemRequest, sid: &str) -> WebResponse<AddItemReply> {
        // let url = format!("http://{}:{}/document-server/item", &self.server.server_name, self.server.port);
        let url = self.server.build_url("item");

        let headers = CustomHeaders { token_type: TokenType::Sid(sid.to_string()), x_request_id: None, cek: None };

        self.server.post_data_retry(&url, request, &headers).await
    }

    ///
    ///
    ///
    pub async fn get_item(&self, item_id: i64, sid: &str) -> WebResponse<GetItemReply> {
        // let url = format!("http://{}:{}/document-server/item/{}", &self.server.server_name, self.server.port,
        //                   item_id );
        let url = self.server.build_url_with_refcode("item", item_id);
        let reply: WebResponse<GetItemReply> = self.server.get_data_retry(&url, &Sid(sid.to_string())).await;
        reply
    }

    ///
    ///
    ///
    pub async fn update_item_tag(
        &self,
        item_id: i64,
        request: &AddItemTagRequest,
        sid: &str,
    ) -> WebResponse<AddItemTagReply> {
        // http://{}:{}/document-server/item/<item_id>/tags

        let end_point = format!("item/{0}/tags", item_id);
        let url = self.server.build_url(&end_point);

        let headers = CustomHeaders { token_type: Sid(sid.to_string()), x_request_id: None, cek: None };

        self.server.post_data_retry(&url, request, &headers).await
    }

    ///
    /// TODO perform URL escaping
    ///
    pub async fn delete_item_tag(&self, item_id: i64, tag_names: &[String], sid: &str) -> WebResponse<SimpleMessage> {
        // http://{}:{}/document-server/item/<item_id>/tags?tag_names=<tag_names>
        let end_point = format!("item/{0}/tags?tag_names={1}", item_id, tag_names.join(","));
        let url = self.server.build_url(&end_point);
        self.server.delete_data_retry(&url, &Sid(sid.to_owned())).await
    }

    ///
    /// TODO might be merged with get_item
    ///
    pub async fn search_item(&self, sid: &str) -> WebResponse<GetItemReply> {
        // let url = format!("http://{}:{}/document-server/item/", &self.server.server_name, self.server.port,
        //                   item_id );
        let url = self.server.build_url("item");
        self.server.get_data_retry(&url, &Sid(sid.to_string())).await
    }

    ///
    ///
    ///
    pub async fn create_tag(&self, request: &AddTagRequest, sid: &str) -> WebResponse<AddTagReply> {
        // let url = format!("http://{}:{}/document-server/tag", &self.server.server_name, self.server.port);
        let url = self.server.build_url("tag");

        let headers = CustomHeaders { token_type: TokenType::Sid(sid.to_string()), x_request_id: None, cek: None };

        self.server.post_data_retry(&url, request, &headers).await
    }

    ///
    ///
    ///
    pub async fn get_all_tag(&self, sid: &str) -> WebResponse<GetTagReply> {
        //let url = format!("http://{}:{}/document-server/tag", &self.server.server_name, self.server.port);
        let url = self.server.build_url("tag");
        self.server.get_data_retry(&url, &Sid(sid.to_string())).await
    }

    ///
    ///
    ///
    pub async fn delete_tag(&self, tag_id: i64, sid: &str) -> WebResponse<SimpleMessage> {
        self.server.delete_for_url(tag_id, "tag", &Sid(sid.to_owned())).await
    }

    ///
    ///
    ///
    pub async fn fulltext_indexing(
        &self,
        raw_text: &str,
        file_name: &str,
        file_ref: &str,
        sid: &str,
    ) -> WebResponse<FullTextReply> {
        let request = FullTextRequest {
            file_name: file_name.to_owned(),
            file_ref: file_ref.to_owned(),
            raw_text: raw_text.to_owned(),
        };
        let url = self.server.build_url("fulltext_indexing");
        let headers = CustomHeaders { token_type: TokenType::Sid(sid.to_string()), x_request_id: None, cek: None };
        self.server.post_data_retry(&url, &request, &headers).await
    }

    ///
    ///
    ///
    pub async fn delete_text_indexing(&self, file_ref: &str, sid: &str) -> WebResponse<SimpleMessage> {
        let request = DeleteFullTextRequest { file_ref: file_ref.to_owned() };
        let url = self.server.build_url("delete_text_indexing");
        let headers = CustomHeaders { token_type: TokenType::Sid(sid.to_string()), x_request_id: None, cek: None };
        self.server.post_data_retry(&url, &request, &headers).await
    }
}

/// File Server

pub struct FileServerClientAsync {
    server: WebServerAsync,
}

impl FileServerClientAsync {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self { server: WebServerAsync::new(server_name, port, "file-server") }
    }

    pub async fn upload(&self, item_info: &str, request: Vec<u8>, sid: &str) -> WebResponse<UploadReply> {
        // let url = format!("http://{}:{}/file-server/upload/{}", &self.server.server_name, self.server.port);
        let url = self.server.build_url_with_refcode("upload2", item_info);

        self.server
            .post_bytes(&url, request, &Sid(sid.to_owned()))
            .await
            .map_err(|e| e.into_owned())
    }

    pub async fn download(&self, file_reference: &str, sid: &str) -> WebResponse<MediaBytes> /*WebResponse<( String, bytes::Bytes, StatusCode )>*/
    {
        // http://localhost:{{PORT}}/file-server/download/47cef2c4-188d-43ed-895d-fe29440633da
        let url = self.server.build_url_with_refcode("download", file_reference);

        self.server.get_binary_data(&url, &Sid(sid.to_string())).await.unwrap_or_else(|e| {
            println!("😎 Cannot download the binary content");
            // log_error!("Cannot download the binary content");
            WebResponse::from_api_error(&INTERNAL_TECHNICAL_ERROR)
        })
    }

    pub async fn info(&self, file_ref: &str, sid: &str) -> WebResponse<GetFileInfoReply> {
        // let url = format!("http://{}:{}/file-server/info/{}", &self.server.server_name, self.server.port);
        let url = self.server.build_url_with_refcode("info", &file_ref);
        // let url = self.server.build_url("info/1ABH234");
        self.server.get_data_retry(&url, &Sid(sid.to_string())).await
    }

    pub async fn stats(&self, file_ref: &str, sid: &str) -> WebResponse<GetFileInfoShortReply> {
        // let url = format!("http://{}:{}/file-server/stats/{}", &self.server.server_name, self.server.port);
        let url = self.server.build_url_with_refcode("stats", &file_ref);
        // let url = self.server.build_url("stats/1ABH234");
        self.server.get_data_retry(&url, &Sid(sid.to_string())).await
    }

    pub async fn loading(&self, sid: &str) -> WebResponse<ListOfUploadInfoReply> {
        // let url = format!("http://{}:{}/file-server/loading/{}", &self.server.server_name, self.server.port);
        let url = self.server.build_url("loading");
        self.server.get_data_retry(&url, &Sid(sid.to_string())).await
    }

    pub async fn list(&self, pattern: &str, sid: &str) -> WebResponse<ListOfFileInfoReply> {
        // let url = format!("http://{}:{}/file-server/stats/{}", &self.server.server_name, self.server.port);
        let url = self.server.build_url_with_refcode("list", &pattern);
        // let url = self.server.build_url("stats/1ABH234");
        self.server.get_data_retry(&url, &Sid(sid.to_string())).await
    }
}

///
/// Tika Server
///
pub struct TikaServerClientAsync {
    server: WebServerAsync,
}

impl TikaServerClientAsync {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self { server: WebServerAsync::new(server_name, port, "") }
    }

    pub async fn parse_data(&self, request: &Vec<u8>) -> anyhow::Result<TikaParsing> {
        // curl -T birdy_tickets.pdf  http://localhost:9998/tika/text --header "Accept: application/json"
        let url = self.server.build_url("tika/text");
        let reply: TikaParsing = self.server.put_bytes_retry(&url, &request).await?;
        Ok(reply)
    }

    pub async fn parse_data_json(&self, request: &Vec<u8>) -> anyhow::Result<serde_json::Value> {
        // curl -T birdy_tickets.pdf  http://localhost:9998/tika/text --header "Accept: application/json"
        let url = self.server.build_url("tika/text");
        let reply: serde_json::Value = self.server.put_bytes_retry(&url, &request).await?;
        Ok(reply)
    }

    // pub fn parse_data_as_string(&self, request : &Vec<u8>) -> anyhow::Result<String> {
    //     // curl -T birdy_tickets.pdf  http://localhost:9998/tika/text --header "Accept: application/json"
    //     let url = self.server.build_url("tika/text");
    //     let reply : String = self.server.put_bytes_as_string_retry(&url, &request)?;
    //     Ok(reply)
    // }

    ///
    /// Read meta information from the utf8 text request
    ///
    pub async fn read_meta(&self, request: &str) -> anyhow::Result<TikaMeta> {
        // curl -T birdy_tickets.pdf  http://localhost:9998/meta --header "Accept: application/json"
        let url = self.server.build_url("meta");

        let bytes = request.as_bytes().to_vec();
        let reply: TikaMeta = self.server.put_bytes_retry(&url, &bytes).await?;
        Ok(reply)
    }
}

#[derive(Clone)]
struct WebServerAsync {
    server_name: String,
    port: u16,
    context: String, // Ex : "document-server"
}

impl WebServerAsync {
    pub fn new(server_name: &str, port: u16, context: &str) -> Self {
        Self { server_name: server_name.to_owned(), port, context: context.to_owned() }
    }

    // async fn retry<F, T>(&self, mut operation: F) -> anyhow::Result<T>
    // where
    //     // F: FnMut() -> anyhow::Result<T>,
    //     F: FnMut() -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send>>,
    // {
    //     let mut count: u32 = 0;
    //     loop {
    //         let operation_result = operation().await;
    //         if operation_result.is_ok() || count >= MAX_HTTP_RETRY {
    //             return operation_result;
    //         }
    //         let t = LAPS as u64;
    //         eprintln!("Wait for {} ms", t);
    //         sleep(Duration::from_millis(t)).await;
    //         log_warn!("Operation failed, attempt=[{}]", count);
    //         count += 1;
    //     }
    // }

    async fn get_data_retry<V: de::DeserializeOwned>(&self, url: &str, token: &TokenType) -> WebResponse<V> {
        // let get_data = || -> anyhow::Result<WebResponse<V>> { self.get_data(url, token) };
        /*        self.retry(get_data)
        .unwrap_or_else(|_| WebResponse::from_api_error(&HTTP_CLIENT_ERROR)).await*/
        // FIXME we bypass the retry routine because we cannot figure out the right signature
        //          for the retry
        self.get_data(&url, &token).await?
    }

    async fn get_data<V: de::DeserializeOwned>(&self, url: &str, token: &TokenType) -> anyhow::Result<WebResponse<V>> {
        let client = Client::new();
        let url = Url::parse(url)?;
        let request_builder = client.get(url).timeout(TIMEOUT);

        let request_builder = match &token {
            Token(token_value) => request_builder.header("token", token_value.clone()),
            Sid(sid_value) => request_builder.header("sid", sid_value.clone()),
            TokenType::None => request_builder,
        };

        Self::send_request_builder(request_builder).await
    }

    async fn post_data_retry<U: Serialize, V: DeserializeOwned>(
        &self,
        url: &str,
        request: &U,
        headers: &CustomHeaders,
    ) -> WebResponse<V> {
        // let post_data = || async { self.post_data(url, request, headers).await };

        let ret = self.post_data(url, request, headers).await;

        // Wrap the post_data call in a closure that returns a boxed future
        // let post_data = || {
        //     let fut = self.post_data(url, request, headers);
        //     Box::pin(fut) as Pin<Box<dyn Future<Output = anyhow::Result<WebResponse<V>>> + Send>>
        // };

        // FIXME we bypass the retry routine because we cannot figure out the right signature
        //          for the retry

        ret.unwrap_or_else(|_| WebResponse::from_api_error(&HTTP_CLIENT_ERROR))
    }

    async fn post_data<U: Serialize, V: de::DeserializeOwned>(
        &self,
        url: &str,
        request: &U,
        headers: &CustomHeaders,
    ) -> anyhow::Result<WebResponse<V>> {
        let client = Client::new();
        let url = Url::parse(url)?;
        let request_builder = client.post(url).timeout(TIMEOUT);

        let request_builder = match &headers.token_type {
            Token(token_value) => request_builder.header("token", token_value.clone()),
            Sid(sid_value) => request_builder.header("sid", sid_value.clone()),
            TokenType::None => request_builder,
        };

        let request_builder = match headers.x_request_id {
            None => request_builder,
            Some(x_request_id) => request_builder.header("X-Request-ID", x_request_id),
        };

        let request_builder = request_builder.json(request);
        Self::send_request_builder(request_builder).await
    }

    /// Generic routine to post a binary content
    async fn post_bytes<V: de::DeserializeOwned>(
        &self,
        url: &str,
        request: Vec<u8>,
        token: &TokenType,
    ) -> WebResponse<V> {
        let client = Client::new();

        let my_url = match Url::parse(url) {
            Ok(parsed_url) => parsed_url, // Parsed URL is valid
            Err(_) => {
                return WebResponse::from_api_error(&URL_PARSING_ERROR); // Return an error response on failure
            }
        };

        let request_builder = client.post(my_url).timeout(TIMEOUT);

        let form =
            multipart::Form::new().part("data", multipart::Part::bytes(request).file_name("111-Bright_Snow.jpg"));

        let request_builder_2 = Self::add_header(request_builder, &token);
        Self::send_request_builder(request_builder_2.multipart(form)).await?
    }

    /// Returns the media type and the binary content and the status code
    async fn get_binary_data(&self, url: &str, token: &TokenType) -> anyhow::Result<WebResponse<MediaBytes>> {
        let client = Client::new();
        let my_url = Url::parse(url).map_err(tr_fwd!())?;
        let request_builder = client.get(my_url).timeout(TIMEOUT);

        //dbg!(&token);
        let request_builder_2 = match token {
            Token(token_value) => request_builder.header("token", token_value.clone()),
            Sid(sid_value) => request_builder.header("sid", sid_value.clone()),
            TokenType::None => request_builder,
        };

        println!("About to request the binary data");
        let response = request_builder_2.send().await.map_err(tr_fwd!())?;
        let status_code = response.status();
        let mime_type = response.headers().get("content-type").ok_or(anyhow!("No content-type"))?.to_str()?;
        let mb = MediaBytes { media_type: mime_type.to_string(), data: response.bytes().await.map_err(tr_fwd!())? };
        Ok(WebResponse::from_item(status_code.as_u16(), mb))
    }

    ///
    /// Put
    ///

    ///
    /// This PUT is for the TikaServer only, so no security token
    ///
    async fn put_bytes<V: de::DeserializeOwned>(&self, url: &str, request: Vec<u8>) -> anyhow::Result<V> {
        let client = Client::new();
        let url = Url::parse(url)?;
        let request_builder = client.put(url).timeout(TIMEOUT);
        let request_builder = request_builder.header("Accept", "application/json");
        let request_builder = request_builder.body(request);
        let r: WebResponse<V> = Self::send_request_builder(request_builder).await?;

        // TODO handle the error correctly
        match r {
            Ok(v) => Ok(v),
            Err(e) => Err(anyhow!(e)),
        }
    }

    async fn put_bytes_retry<V: de::DeserializeOwned>(&self, url: &str, request: &Vec<u8>) -> anyhow::Result<V> {
        let clone_request = request.clone(); // TODO find a way not to clone the array
        self.put_bytes(url, clone_request).await
    }

    async fn send_request_builder<V: DeserializeOwned>(
        request_builder: RequestBuilder,
    ) -> anyhow::Result<WebResponse<V>> {
        let response = match request_builder.send().await {
            Ok(v) => {
                let status_code = v.status();
                if status_code.as_u16() >= 300 {
                    let value: Result<SimpleMessage, reqwest::Error> = v.json().await;
                    match value {
                        Ok(v_value) => WebResponse::from_simple(status_code.as_u16(), v_value),
                        Err(e) => return Err(anyhow!("Failed to parse error response: {}", e.to_string())),
                    }
                } else {
                    let value: Result<V, reqwest::Error> = v.json().await;
                    match value {
                        Ok(v_value) => WebResponse::from_item(status_code.as_u16(), v_value),
                        Err(e) => return Err(anyhow!("Failed to parse successful response: {}", e.to_string())),
                    }
                }
            }
            Err(e) => {
                return Err(anyhow!("Http request failed: {}", e.to_string()));
            }
        };
        Ok(response)
    }

    ///
    /// Delete
    ///
    async fn delete_data<V: de::DeserializeOwned>(
        &self,
        url: &str,
        token: &TokenType,
    ) -> anyhow::Result<WebResponse<V>> {
        let client = Client::new();
        let url = Url::parse(url)?;
        let request_builder = client.delete(url).timeout(TIMEOUT);
        Self::send_request_builder(Self::add_header(request_builder, &token)).await
    }

    async fn delete_data_retry<V: de::DeserializeOwned>(&self, url: &str, token: &TokenType) -> WebResponse<V> {
        self.delete_data(url, token).await.unwrap_or_else(|_| WebResponse::from_api_error(&HTTP_CLIENT_ERROR))

        // let delete_data = || -> anyhow::Result<WebResponse<V>> { self.delete_data(url, token) };
        // self.retry(delete_data)
        //     .unwrap_or_else(|_| WebResponse::from_api_error(&HTTP_CLIENT_ERROR))
    }

    ///
    /// Generic implementation of a delete action
    /// url_path : ex : admin-server/tag
    /// refcode : "eb65e" or 125
    ///
    async fn delete_for_url<T>(&self, refcode: T, end_point: &str, token: &TokenType) -> WebResponse<SimpleMessage>
    where
        T: Display,
    {
        // let url = format!("http://{}:{}/{}/{}", &self.server.server_name, self.server.port, end_point,
        //                   refcode);
        let url = self.build_url_with_refcode(end_point, refcode);
        self.delete_data_retry(&url, &token).await
    }

    ///
    /// end_point , ex : "key", "tag"
    ///
    fn build_url(&self, end_point: &str) -> String {
        format!("http://{}:{}/{}/{}", &self.server_name, self.port, self.context, end_point)
    }

    ///
    /// end_point , ex : "key", "tag"
    ///
    fn build_url_with_refcode<T>(&self, end_point: &str, ref_code: T) -> String
    where
        T: Display,
    {
        format!("http://{}:{}/{}/{}/{}", &self.server_name, self.port, self.context, end_point, ref_code)
    }

    fn add_header(request_builder: RequestBuilder, token: &TokenType) -> RequestBuilder {
        match token {
            Token(token_value) => request_builder.header("token", token_value.clone()),
            Sid(sid_value) => request_builder.header("sid", sid_value.clone()),
            TokenType::None => request_builder,
        }
    }
}
