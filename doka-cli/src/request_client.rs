use std::fmt::Display;
use std::thread;
use std::time::Duration;

use anyhow::anyhow;
use log::warn;
use percent_encoding::{NON_ALPHANUMERIC, utf8_percent_encode};
use reqwest::blocking::{multipart, RequestBuilder};
use reqwest::StatusCode;
use serde::{de, Serialize};
use url::Url;

use commons_error::*;
use dkdto::{
    AddItemReply, AddItemRequest, AddItemTagReply, AddItemTagRequest, AddKeyReply, AddKeyRequest,
    AddTagReply, AddTagRequest, CreateCustomerReply, CreateCustomerRequest, CustomerKeyReply,
    DeleteFullTextRequest, ErrorMessage, FullTextReply, FullTextRequest, GetFileInfoReply,
    GetFileInfoShortReply, GetItemReply, GetTagReply, ListOfFileInfoReply, ListOfUploadInfoReply,
    LoginReply, LoginRequest, MediaBytes, OpenSessionReply, OpenSessionRequest, SessionReply,
    SimpleMessage, TikaMeta, TikaParsing, UploadReply, WebResponse, WebTypeBuilder,
};
use dkdto::error_codes::HTTP_CLIENT_ERROR;

use crate::request_client::TokenType::{Sid, Token};

const TIMEOUT: Duration = Duration::from_secs(60 * 60);
const MAX_HTTP_RETRY: u32 = 5;

#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
pub struct CustomHeaders {
    pub token_type: TokenType,
    pub x_request_id: Option<u32>,
    pub cek: Option<String>,
}
#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
pub enum TokenType {
    Token(String),
    Sid(String),
    None,
}
impl TokenType {
    pub fn value(&self) -> String {
        String::from(match self {
            Token(tok) => tok.as_str(),
            Sid(sid) => sid.as_str(),
            TokenType::None => "",
        })
    }
}
const LAPS: u32 = 2_000;

#[derive(Clone)]
struct WebServer {
    server_name: String,
    port: u16,
    context: String, // Ex : "document-server"
}

impl WebServer {
    pub fn new(server_name: &str, port: u16, context: &str) -> Self {
        Self {
            server_name: server_name.to_owned(),
            port,
            context: context.to_owned(),
        }
    }

    /// General retry routine
    fn retry<F, T>(&self, mut operation: F) -> anyhow::Result<T>
    where
        F: FnMut() -> anyhow::Result<T>,
    {
        let mut count: u32 = 0;
        loop {
            let operation_result = operation();
            if operation_result.is_ok() || count >= MAX_HTTP_RETRY {
                return operation_result;
            }
            let t = (LAPS) as u64;
            eprintln!("Wait for {} ms", t);
            thread::sleep(Duration::from_millis(t));
            log_warn!("Operation failed, attempt=[{}]", count);
            count += 1;
        }
    }

    fn get_data_retry<V: de::DeserializeOwned>(
        &self,
        url: &str,
        token: &TokenType,
    ) -> WebResponse<V> {
        let get_data = || -> anyhow::Result<WebResponse<V>> { self.get_data(url, token) };
        self.retry(get_data)
            .unwrap_or_else(|_| WebResponse::from_errorset(&HTTP_CLIENT_ERROR))
    }

    fn get_data<V: de::DeserializeOwned>(
        &self,
        url: &str,
        token: &TokenType,
    ) -> anyhow::Result<WebResponse<V>> {
        let request_builder = reqwest::blocking::Client::new()
            .get(Url::parse(url)?)
            .timeout(TIMEOUT);
        let request_builder_2 = Self::add_header(request_builder, &token);
        Self::send_request_builder(request_builder_2)
    }

    ///
    fn send_request_builder<V: de::DeserializeOwned>(
        request_builder: RequestBuilder,
    ) -> anyhow::Result<WebResponse<V>> {
        let wt = match request_builder.send() {
            Ok(v) => {
                dbg!(&v);
                let status_code = v.status();
                dbg!(&status_code);
                let wt = if status_code.as_u16() >= 300 {
                    Err(ErrorMessage {
                        http_error_code: status_code.as_u16(),
                        message: HTTP_CLIENT_ERROR.err_message.to_string(),
                    })
                } else {
                    let value: Result<V, reqwest::Error> = v.json(); // TODO
                    let v_value = value.unwrap();
                    WebResponse::from_item(status_code.as_u16(), v_value)
                };
                wt
            }
            Err(e) => {
                return Err(anyhow!("Http request failed: {}", e.to_string()));
            }
        };
        Ok(wt)
    }

    /// Returns the media type and the binary content and the status code
    fn get_binary_data(
        &self,
        url: &str,
        token: &TokenType,
    ) -> anyhow::Result<WebResponse<MediaBytes>> {
        let request_builder = reqwest::blocking::Client::new()
            .get(Url::parse(url)?)
            .timeout(TIMEOUT);

        let request_builder_2 = match token {
            Token(token_value) => request_builder.header("token", token_value.clone()),
            Sid(sid_value) => request_builder.header("sid", sid_value.clone()),
            TokenType::None => request_builder,
        };

        let response = request_builder_2.send()?;
        let status_code = response.status();
        let mime_type = response
            .headers()
            .get("content-type")
            .ok_or(anyhow!("No content-type"))?
            .to_str()?;
        let mb = MediaBytes {
            media_type: mime_type.to_string(),
            data: response.bytes()?,
        };
        Ok(WebResponse::from_item(status_code.as_u16(), mb))
    }

    fn get_binary_data_retry(&self, url: &str, token: &TokenType) -> WebResponse<MediaBytes> {
        let get_binary_data =
            || -> anyhow::Result<WebResponse<MediaBytes>> { self.get_binary_data(url, token) };
        self.retry(get_binary_data)
            .unwrap_or_else(|_| WebResponse::from_errorset(&HTTP_CLIENT_ERROR))
    }

    ///
    /// Post
    ///

    /// Generic routine to post a message
    fn post_data<U: Serialize, V: de::DeserializeOwned>(
        &self,
        url: &str,
        request: &U,
        headers: &CustomHeaders,
    ) -> anyhow::Result<WebResponse<V>> {
        let request_builder = reqwest::blocking::Client::new()
            .post(Url::parse(url)?)
            .timeout(TIMEOUT);
        let request_builder_2 = match &headers.token_type {
            Token(token_value) => request_builder.header("token", token_value.clone()),
            Sid(sid_value) => request_builder.header("sid", sid_value.clone()),
            TokenType::None => request_builder,
        };

        let request_builder_3 = match headers.x_request_id {
            None => request_builder_2,
            Some(x_request_id) => request_builder_2.header("X-Request-ID", x_request_id),
        };

        let request_builder_4 = request_builder_3.json(request);

        Self::send_request_builder(request_builder_4)
    }

    fn post_data_retry<U: Serialize, V: de::DeserializeOwned>(
        &self,
        url: &str,
        request: &U,
        headers: &CustomHeaders,
    ) -> WebResponse<V> {
        let post_data =
            || -> anyhow::Result<WebResponse<V>> { self.post_data(url, request, headers) };
        self.retry(post_data)
            .unwrap_or_else(|_| WebResponse::from_errorset(&HTTP_CLIENT_ERROR))
    }

    fn post_bytes_retry<V: de::DeserializeOwned>(
        &self,
        url: &str,
        request: &Vec<u8>,
        token: &TokenType,
    ) -> WebResponse<V> {
        let post_bytes = || -> anyhow::Result<WebResponse<V>> {
            let rr = request.clone();
            self.post_bytes(url, rr, token)
        };
        match self.retry(post_bytes) {
            Ok(v) => v,
            Err(e) => WebResponse::from_simple(
                StatusCode::BAD_REQUEST.as_u16(),
                SimpleMessage {
                    message: e.to_string(),
                },
            ),
        }
    }

    /// Generic routine to post a binary content
    fn post_bytes<V: de::DeserializeOwned>(
        &self,
        url: &str,
        request: Vec<u8>,
        token: &TokenType,
    ) -> anyhow::Result<WebResponse<V>> {
        let request_builder = reqwest::blocking::Client::new()
            .post(Url::parse(url)?)
            .timeout(TIMEOUT);
        let form = multipart::Form::new().part(
            "data",
            multipart::Part::bytes(request).file_name("111-Bright_Snow.jpg"),
        );

        dbg!(&url);
        dbg!(&token);
        let request_builder_2 = Self::add_header(request_builder, &token);
        Self::send_request_builder(request_builder_2.multipart(form))
    }

    ///
    /// Put
    ///

    ///
    /// This PUT is for the TikaServer only, so no security token
    ///
    fn put_bytes<V: de::DeserializeOwned>(&self, url: &str, request: Vec<u8>) -> anyhow::Result<V> {
        let request_builder = reqwest::blocking::Client::new()
            .put(Url::parse(url)?)
            .timeout(TIMEOUT);
        let r2 = request_builder.header("Accept", "application/json");
        let reply: V = r2.body(request).send()?.json()?;
        Ok(reply)
    }

    fn put_bytes_retry<V: de::DeserializeOwned>(
        &self,
        url: &str,
        request: &Vec<u8>,
    ) -> anyhow::Result<V> {
        let put_bytes = || -> anyhow::Result<V> {
            let rr = request.clone();
            self.put_bytes(url, rr)
        };
        self.retry(put_bytes)
    }

    ///
    /// Patch
    ///

    fn patch_data<V: de::DeserializeOwned>(
        &self,
        url: &str,
        token: &TokenType,
    ) -> anyhow::Result<WebResponse<V>> {
        let request_builder = reqwest::blocking::Client::new()
            .patch(Url::parse(url)?)
            .timeout(TIMEOUT);
        Self::send_request_builder(Self::add_header(request_builder, &token))
    }

    fn patch_data_retry<V: de::DeserializeOwned>(
        &self,
        url: &str,
        token: &TokenType,
    ) -> WebResponse<V> {
        let patch_data = || -> anyhow::Result<WebResponse<V>> { self.patch_data(url, &token) };
        self.retry(patch_data)
            .unwrap_or_else(|_| WebResponse::from_errorset(&HTTP_CLIENT_ERROR))
    }

    ///
    /// Delete
    ///
    fn delete_data<V: de::DeserializeOwned>(
        &self,
        url: &str,
        token: &TokenType,
    ) -> anyhow::Result<WebResponse<V>> {
        let request_builder = reqwest::blocking::Client::new()
            .delete(Url::parse(url)?)
            .timeout(TIMEOUT);

        Self::send_request_builder(Self::add_header(request_builder, &token))
    }

    fn delete_data_retry<V: de::DeserializeOwned>(
        &self,
        url: &str,
        token: &TokenType,
    ) -> WebResponse<V> {
        let delete_data = || -> anyhow::Result<WebResponse<V>> { self.delete_data(url, token) };
        self.retry(delete_data)
            .unwrap_or_else(|_| WebResponse::from_errorset(&HTTP_CLIENT_ERROR))
    }

    ///
    /// Generic implementation of a delete action
    /// url_path : ex : admin-server/tag
    /// refcode : "eb65e" or 125
    ///
    fn delete_for_url<T>(
        &self,
        refcode: T,
        end_point: &str,
        token: &TokenType,
    ) -> WebResponse<SimpleMessage>
    where
        T: Display,
    {
        // let url = format!("http://{}:{}/{}/{}", &self.server.server_name, self.server.port, end_point,
        //                   refcode);
        let url = self.build_url_with_refcode(end_point, refcode);
        self.delete_data_retry(&url, &token)
    }

    ///
    /// end_point , ex : "key", "tag"
    ///
    fn build_url(&self, end_point: &str) -> String {
        format!(
            "http://{}:{}/{}/{}",
            &self.server_name, self.port, self.context, end_point
        )
    }

    ///
    /// end_point , ex : "key", "tag"
    ///
    fn build_url_with_refcode<T>(&self, end_point: &str, ref_code: T) -> String
    where
        T: Display,
    {
        format!(
            "http://{}:{}/{}/{}/{}",
            &self.server_name, self.port, self.context, end_point, ref_code
        )
    }

    fn add_header(request_builder: RequestBuilder, token: &TokenType) -> RequestBuilder {
        match token {
            Token(token_value) => request_builder.header("token", token_value.clone()),
            Sid(sid_value) => request_builder.header("sid", sid_value.clone()),
            TokenType::None => request_builder,
        }
    }
}

pub struct KeyManagerClient {
    server: WebServer,
}

impl KeyManagerClient {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServer::new(server_name, port, "key-manager"),
        }
    }

    ///
    /// It is not supposed to return an error, so let's return the Reply directly
    ///
    pub fn add_key(&self, request: &AddKeyRequest, token: &TokenType) -> WebResponse<AddKeyReply> {
        //let url = format!("http://{}:{}/{}/key", &self.server.server_name, self.server.port, self.server.context);
        let url = self.server.build_url("key");

        let headers = CustomHeaders {
            token_type: token.clone(),
            x_request_id: None,
            cek: None,
        };

        self.server.post_data_retry(&url, request, &headers)
    }

    pub fn get_key(&self, customer_code: &str, token: &str) -> WebResponse<CustomerKeyReply> {
        // http://localhost:{{PORT}}/key-manager/key/f1248fab
        let url = self.server.build_url_with_refcode("key", customer_code);
        self.server.get_data_retry(&url, &Token(token.to_string()))
    }
}

#[derive(Clone)]
pub struct SessionManagerClient {
    server: WebServer,
}

impl SessionManagerClient {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServer::new(server_name, port, "session-manager"),
        }
    }

    pub fn open_session(
        &self,
        request: &OpenSessionRequest,
        token: &str,
        x_request_id: Option<u32>,
    ) -> WebResponse<OpenSessionReply> {
        //let url = format!("http://{}:{}/session-manager/session", &self.server.server_name, self.server.port);
        let url = self.server.build_url("session");

        let headers = CustomHeaders {
            token_type: Token(token.to_string()),
            x_request_id,
            cek: None,
        };

        self.server.post_data_retry(&url, request, &headers)
    }

    pub fn get_session(&self, sid: &str, token: &str) -> WebResponse<SessionReply> {
        // let url = format!("http://{}:{}/session-manager/session/{}", &self.server.server_name, self.server.port,
        //                   Uri::percent_encode(sid) );
        let url = self.server.build_url_with_refcode(
            "session",
            utf8_percent_encode(sid, NON_ALPHANUMERIC).to_string(),
        );
        self.server.get_data_retry(&url, &Token(token.to_string()))
    }
}

///
///
///
pub struct AdminServerClient {
    server: WebServer,
}

impl AdminServerClient {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServer::new(server_name, port, "admin-server"),
        }
    }

    pub fn create_customer(
        &self,
        request: &CreateCustomerRequest,
        token: &str,
    ) -> WebResponse<CreateCustomerReply> {
        // let url = format!("http://{}:{}/admin-server/customer", &self.server.server_name, self.server.port);
        let url = self.server.build_url("customer");

        let headers = CustomHeaders {
            token_type: Token(token.to_string()),
            x_request_id: None,
            cek: None,
        };

        self.server.post_data_retry(&url, request, &headers)
    }

    pub fn customer_removable(
        &self,
        customer_code: &str,
        token: &str,
    ) -> WebResponse<SimpleMessage> {
        // let url = format!("http://{}:{}/admin-server/customer/removable/{}", &self.server.server_name, self.server.port,
        //                   Uri::percent_encode(customer_code) );
        let url = self.server.build_url_with_refcode(
            "customer/removable",
            utf8_percent_encode(customer_code, NON_ALPHANUMERIC).to_string(),
        );
        self.server.patch_data_retry(&url, &Token(token.to_owned()))
    }

    pub fn delete_customer(&self, customer_code: &str, token: &str) -> WebResponse<SimpleMessage> {
        self.server
            .delete_for_url(customer_code, "customer", &Token(token.to_owned()))
    }

    pub fn login(&self, request: &LoginRequest) -> WebResponse<LoginReply> {
        // let url = format!("http://{}:{}/admin-server/login", &self.server.server_name, self.server.port);
        let url = self.server.build_url("login");

        let headers = CustomHeaders {
            token_type: TokenType::None,
            x_request_id: None,
            cek: None,
        };

        self.server.post_data_retry(&url, request, &headers)
    }
}

///
/// Document Server
///
pub struct DocumentServerClient {
    server: WebServer,
}

impl DocumentServerClient {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServer::new(server_name, port, "document-server"),
        }
    }

    pub fn create_item(&self, request: &AddItemRequest, sid: &str) -> WebResponse<AddItemReply> {
        // let url = format!("http://{}:{}/document-server/item", &self.server.server_name, self.server.port);
        let url = self.server.build_url("item");

        let headers = CustomHeaders {
            token_type: TokenType::Sid(sid.to_string()),
            x_request_id: None,
            cek: None,
        };

        self.server.post_data_retry(&url, request, &headers)
    }

    ///
    ///
    ///
    pub fn get_item(&self, item_id: i64, sid: &str) -> WebResponse<GetItemReply> {
        // let url = format!("http://{}:{}/document-server/item/{}", &self.server.server_name, self.server.port,
        //                   item_id );
        let url = self.server.build_url_with_refcode("item", item_id);
        let reply: WebResponse<GetItemReply> =
            self.server.get_data_retry(&url, &Sid(sid.to_string()));
        reply
    }

    ///
    ///
    ///
    pub fn update_item_tag(
        &self,
        item_id: i64,
        request: &AddItemTagRequest,
        sid: &str,
    ) -> WebResponse<AddItemTagReply> {
        // http://{}:{}/document-server/item/<item_id>/tags

        let end_point = format!("item/{0}/tags", item_id);
        let url = self.server.build_url(&end_point);

        let headers = CustomHeaders {
            token_type: Sid(sid.to_string()),
            x_request_id: None,
            cek: None,
        };

        self.server.post_data_retry(&url, request, &headers)
    }

    ///
    /// TODO perform URL escaping
    ///
    pub fn delete_item_tag(
        &self,
        item_id: i64,
        tag_names: &[String],
        sid: &str,
    ) -> WebResponse<SimpleMessage> {
        // http://{}:{}/document-server/item/<item_id>/tags?tag_names=<tag_names>
        let end_point = format!("item/{0}/tags?tag_names={1}", item_id, tag_names.join(","));
        let url = self.server.build_url(&end_point);
        self.server.delete_data_retry(&url, &Sid(sid.to_owned()))
    }

    ///
    /// TODO might be merged with get_item
    ///
    pub fn search_item(&self, sid: &str) -> WebResponse<GetItemReply> {
        // let url = format!("http://{}:{}/document-server/item/", &self.server.server_name, self.server.port,
        //                   item_id );
        let url = self.server.build_url("item");
        self.server.get_data_retry(&url, &Sid(sid.to_string()))
    }

    ///
    ///
    ///
    pub fn create_tag(&self, request: &AddTagRequest, sid: &str) -> WebResponse<AddTagReply> {
        // let url = format!("http://{}:{}/document-server/tag", &self.server.server_name, self.server.port);
        let url = self.server.build_url("tag");

        let headers = CustomHeaders {
            token_type: TokenType::Sid(sid.to_string()),
            x_request_id: None,
            cek: None,
        };

        self.server.post_data_retry(&url, request, &headers)
    }

    ///
    ///
    ///
    pub fn get_all_tag(&self, sid: &str) -> WebResponse<GetTagReply> {
        //let url = format!("http://{}:{}/document-server/tag", &self.server.server_name, self.server.port);
        let url = self.server.build_url("tag");
        self.server.get_data_retry(&url, &Sid(sid.to_string()))
    }

    ///
    ///
    ///
    pub fn delete_tag(&self, tag_id: i64, sid: &str) -> WebResponse<SimpleMessage> {
        self.server
            .delete_for_url(tag_id, "tag", &Sid(sid.to_owned()))
    }

    ///
    ///
    ///
    pub fn fulltext_indexing(
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
        let headers = CustomHeaders {
            token_type: TokenType::Sid(sid.to_string()),
            x_request_id: None,
            cek: None,
        };
        self.server.post_data_retry(&url, &request, &headers)
    }

    ///
    ///
    ///
    pub fn delete_text_indexing(&self, file_ref: &str, sid: &str) -> WebResponse<SimpleMessage> {
        let request = DeleteFullTextRequest {
            file_ref: file_ref.to_owned(),
        };
        let url = self.server.build_url("delete_text_indexing");
        let headers = CustomHeaders {
            token_type: TokenType::Sid(sid.to_string()),
            x_request_id: None,
            cek: None,
        };
        self.server.post_data_retry(&url, &request, &headers)
    }
}

///
/// Document Server
///
pub struct TikaServerClient {
    server: WebServer,
}

impl TikaServerClient {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServer::new(server_name, port, ""),
        }
    }

    pub fn parse_data(&self, request: &Vec<u8>) -> anyhow::Result<TikaParsing> {
        // curl -T birdy_tickets.pdf  http://localhost:9998/tika/text --header "Accept: application/json"
        let url = self.server.build_url("tika/text");
        let reply: TikaParsing = self.server.put_bytes_retry(&url, &request)?;
        Ok(reply)
    }

    pub fn parse_data_json(&self, request: &Vec<u8>) -> anyhow::Result<serde_json::Value> {
        // curl -T birdy_tickets.pdf  http://localhost:9998/tika/text --header "Accept: application/json"
        let url = self.server.build_url("tika/text");
        let reply: serde_json::Value = self.server.put_bytes_retry(&url, &request)?;
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
    pub fn read_meta(&self, request: &str) -> anyhow::Result<TikaMeta> {
        // curl -T birdy_tickets.pdf  http://localhost:9998/meta --header "Accept: application/json"
        let url = self.server.build_url("meta");

        let bytes = request.as_bytes().to_vec();
        let reply: TikaMeta = self.server.put_bytes_retry(&url, &bytes)?;
        Ok(reply)
    }
}

///
/// File Server
///
pub struct FileServerClient {
    server: WebServer,
}

impl FileServerClient {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServer::new(server_name, port, "file-server"),
        }
    }

    pub fn upload(
        &self,
        item_info: &str,
        request: &Vec<u8>,
        sid: &str,
    ) -> WebResponse<UploadReply> {
        // let url = format!("http://{}:{}/file-server/upload/{}", &self.server.server_name, self.server.port);
        let url = self.server.build_url_with_refcode("upload2", item_info);
        // let url = self.server.build_url("upload2/1ABH234");
        self.server
            .post_bytes_retry(&url, request, &Sid(sid.to_string()))
    }

    pub fn download(&self, file_reference: &str, sid: &str) -> WebResponse<MediaBytes> /*WebResponse<( String, bytes::Bytes, StatusCode )>*/
    {
        // http://localhost:{{PORT}}/file-server/download/47cef2c4-188d-43ed-895d-fe29440633da
        let url = self
            .server
            .build_url_with_refcode("download", file_reference);
        self.server
            .get_binary_data_retry(&url, &Sid(sid.to_string()))
    }

    pub fn info(&self, file_ref: &str, sid: &str) -> WebResponse<GetFileInfoReply> {
        // let url = format!("http://{}:{}/file-server/info/{}", &self.server.server_name, self.server.port);
        let url = self.server.build_url_with_refcode("info", &file_ref);
        // let url = self.server.build_url("info/1ABH234");
        self.server.get_data_retry(&url, &Sid(sid.to_string()))
    }

    pub fn stats(&self, file_ref: &str, sid: &str) -> WebResponse<GetFileInfoShortReply> {
        // let url = format!("http://{}:{}/file-server/stats/{}", &self.server.server_name, self.server.port);
        let url = self.server.build_url_with_refcode("stats", &file_ref);
        // let url = self.server.build_url("stats/1ABH234");
        self.server.get_data_retry(&url, &Sid(sid.to_string()))
    }

    pub fn loading(&self, sid: &str) -> WebResponse<ListOfUploadInfoReply> {
        // let url = format!("http://{}:{}/file-server/loading/{}", &self.server.server_name, self.server.port);
        let url = self.server.build_url("loading");
        self.server.get_data_retry(&url, &Sid(sid.to_string()))
    }

    pub fn list(&self, pattern: &str, sid: &str) -> WebResponse<ListOfFileInfoReply> {
        // let url = format!("http://{}:{}/file-server/stats/{}", &self.server.server_name, self.server.port);
        let url = self.server.build_url_with_refcode("list", &pattern);
        // let url = self.server.build_url("stats/1ABH234");
        self.server.get_data_retry(&url, &Sid(sid.to_string()))
    }
}

#[cfg(test)]
mod test {
    use url::Url;

    use dkdto::TikaParsing;

    use crate::request_client::{DocumentServerClient, TikaServerClient};

    fn put_data(url: &str, request: Vec<u8>) -> anyhow::Result<TikaParsing> {
        let request_builder = reqwest::blocking::Client::new().put(Url::parse(url)?);
        let r2 = request_builder.header("Accept", "application/json");
        let reply: TikaParsing = r2.body(request).send()?.json()?;
        Ok(reply)
    }

    #[test]
    fn test_put_basic() -> anyhow::Result<()> {
        let byte_buf: Vec<u8> = std::fs::read("C:/Users/denis/wks-poc/tika/Gandi_order.pdf")?;
        // curl -T birdy_tickets.pdf  http://localhost:9998/tika/text --header "Accept: application/json"
        let s = put_data("http://localhost:40010/tika/text", byte_buf)?;
        dbg!(&s);

        Ok(())
    }

    #[test]
    fn test_put_from_client() -> anyhow::Result<()> {
        let byte_buf: Vec<u8> = std::fs::read("C:/Users/denis/wks-poc/tika/Gandi_order.pdf")?;
        let client = TikaServerClient::new("localhost", 40010);
        let s = client.parse_data(&byte_buf)?;
        let _ = dbg!(&s);
        println!("Extracted Text : [{}]", &s.x_tika_content);

        let meta = client.read_meta(&s.x_tika_content)?;

        println!("Language : [{}]", &meta.language);

        Ok(())
    }

    #[test]
    fn test_read_meta_from_client() -> anyhow::Result<()> {
        let text = std::fs::read_to_string("C:/Users/denis/wks-poc/tika/content.en.txt")?;
        let client = TikaServerClient::new("localhost", 40010);
        let s = client.read_meta(&text);

        let _ = dbg!(s);
        Ok(())
    }

    #[test]
    fn test_put_big_from_client() -> anyhow::Result<()> {
        let byte_buf: Vec<u8> = std::fs::read("C:/Users/denis/wks-poc/tika/big_planet.pdf")?;
        let client = TikaServerClient::new("localhost", 40010);
        let s = client.parse_data(&byte_buf)?;
        let _ = dbg!(&s);
        println!("Extracted Text : [{}]", &s.x_tika_content);

        let meta = client.read_meta(&s.x_tika_content)?;

        println!("Language : [{}]", &meta.language);

        Ok(())
    }

    #[test]
    fn test_post_bytes_basic() -> anyhow::Result<()> {
        let byte_buf = std::fs::read("/home/denis/Dropbox/Upload/111-Bright_Snow.jpg")?;

        let url = "http://localhost:30080/file-server/upload2/very_dicky";

        // let request_builder = reqwest::blocking::Client::new()
        //     .post(Url::parse(url)?)
        //     .header("Content-Type", "multipart/form-data")
        //     //.header("Accept", "application/json")
        //     .header(
        //         "sid",
        //         "9ARks93f49KdpZ3sPnPYpSRZUOk9shmbQVZKn9If6RQmwi25yGtCN3vCis4JnYxGO46Hf07hDEZc9LFPRW5ncPFCeO-14VyW-Hdq-Q",
        //     );

        use reqwest::blocking::multipart;

        let form = multipart::Form::new().part(
            "data",
            multipart::Part::bytes(byte_buf).file_name("111-Bright_Snow.jpg"),
        );

        let request_builder = reqwest::blocking::Client::new()
            .post(Url::parse(url)?)
            .header(
                "sid",
                "9ARks93f49KdpZ3sPnPYpSRZUOk9shmbQVZKn9If6RQmwi25yGtCN3vCis4JnYxGO46Hf07hDEZc9LFPRW5ncPFCeO-14VyW-Hdq-Q",
            )
            .multipart(form);

        let reply = request_builder.send()?.text()?;

        let _ = dbg!(reply);
        Ok(())
    }

    #[test]
    fn test_post_bin_from_client() -> anyhow::Result<()> {
        Ok(())
    }

    #[test]
    fn test_get_item() -> anyhow::Result<()> {
        println!("Start the test");
        let client = DocumentServerClient::new("localhost", 30070);
        let reply = client.get_item(5, "4Sw3Z9etp4C8RoSSxwU7fLJrBfDStLmgFXUmhaHQf4iLwHV2CRiQsaSdFQG6W3YeYL_54TmorxT9crTZp1UIOvBysHkpVVrkr8J3OA");
        let _ = dbg!(&reply);
        println!("Reply : [{:?}]", reply);
        Ok(())
    }

    #[test]
    fn url_escaping() {
        use url::Url;
        let original_url = "https://example.com/api/endpoint?param=value with spaces&p2=with,coma";

        // Parse the URL
        let url = Url::parse(original_url).expect("Invalid URL");

        // Get the encoded URL as a string
        let encoded_url = url.as_str();

        println!("Original URL: {}", original_url);
        println!("Encoded URL: {}", encoded_url);
    }
}
