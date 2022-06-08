use std::fmt::Display;
use std::time::Duration;
use dkdto::{AddItemReply, AddItemRequest, AddKeyReply, AddKeyRequest, AddTagReply, AddTagRequest, CreateCustomerReply, CreateCustomerRequest, CustomerKeyReply, FullTextReply, FullTextRequest, GetItemReply, GetTagReply, JsonErrorSet, LoginReply, LoginRequest, OpenSessionReply, OpenSessionRequest, SessionReply, TikaMeta, TikaParsing, UploadReply};
use log::{error, warn};

use rocket::http::uri::Uri;
use serde::{de, Serialize};
use dkdto::error_codes::HTTP_CLIENT_ERROR;
use crate::request_client::TokenType::{Sid, Token};
use commons_error::*;

const TIMEOUT : Duration = Duration::from_secs(60 * 60);
const MAX_HTTP_RETRY: i32 = 5;

#[derive(Clone, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
pub struct CustomHeaders {
    token_type : TokenType,
    x_request_id : Option<u32>,
    cek : Option<String>,
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
            TokenType::Token(tok) => {tok.as_str()}
            TokenType::Sid(sid) => {sid.as_str()}
            TokenType::None => {""}
        })
    }
}

struct WebServer
{
    server_name : String,
    port : u16,
    context : String, // Ex : "document-server"
}

impl WebServer {

    pub fn new(server_name : &str, port : u16, context: &str) -> Self {
        Self {
            server_name : server_name.to_owned(),
            port,
            context : context.to_owned(),
        }
    }

    ///
    /// Get
    ///

    fn get_data<V : de::DeserializeOwned>( &self, url : &str, token : &TokenType ) -> anyhow::Result<V>
    {
        let request_builder = reqwest::blocking::Client::new().get(url).timeout(TIMEOUT);
        let request_builder_2 = match token {
            Token(token_value) => {
                request_builder.header("token", token_value.clone())
            }
            Sid(sid_value) => {
                request_builder.header("sid", sid_value.clone())
            }
            TokenType::None => {
                request_builder
            }
        };
        let reply: V = request_builder_2.send()?.json()?;
        Ok(reply)
    }

    fn get_data_retry<V : de::DeserializeOwned>( &self, url : &str, token : &TokenType ) -> anyhow::Result<V>
    {
        let mut r_reply;
        let mut count = 0;
        loop {
            r_reply = self.get_data(url, token);
            if r_reply.is_ok() || count >= MAX_HTTP_RETRY {
                break;
            }
            log_warn!("Url call failed, url=[{}], attempt=[{}]", url, count);
            count += 1;
        }
        let reply = r_reply?;
        Ok(reply)
    }

    ///
    /// Post
    ///

    /// Generic routine to post a message
    fn post_data< U: Serialize, V : de::DeserializeOwned>( &self, url : &str, request : &U, headers : &CustomHeaders) -> anyhow::Result<V>
    {
        let request_builder = reqwest::blocking::Client::new().post(url).timeout(TIMEOUT);
        let request_builder_2 = match &headers.token_type {
            TokenType::Token(token_value) => {
                request_builder.header("token", token_value.clone())
            }
            TokenType::Sid(sid_value) => {
                request_builder.header("sid", sid_value.clone())
            }
            TokenType::None => {
                request_builder
            }
        };

        let request_builder_3 = match headers.x_request_id {
            None => {request_builder_2}
            Some(x_request_id) => {
                request_builder_2.header("X-Request-ID", x_request_id)
            }
        };

        let reply: V = request_builder_3.json(request).send()?.json()?;
        Ok(reply)
    }


    fn post_data_retry< U: Serialize, V : de::DeserializeOwned>( &self, url : &str, request : &U, headers : &CustomHeaders ) -> anyhow::Result<V>
    {
        let mut r_reply;
        let mut count = 0;
        loop {
            r_reply = self.post_data(url, request, headers);
            if r_reply.is_ok() || count >= MAX_HTTP_RETRY {
                break;
            }
            log_warn!("Url call failed, url=[{}], attempt=[{}]", url, count);
            count += 1;
        }
        let reply = r_reply?;
        Ok(reply)
    }

    fn post_bytes_retry<V : de::DeserializeOwned>( &self, url : &str, request : &Vec<u8>, token : &TokenType ) -> anyhow::Result<V>
    {
        let mut r_reply;
        let mut count = 0;
        loop {
            let rr = request.clone();
            r_reply = self.post_bytes(url, rr, token);
            if r_reply.is_ok() || count >= MAX_HTTP_RETRY {
                break;
            }
            log_warn!("Url call failed, url=[{}], attempt=[{}]", url, count);
            count += 1;
        }
        let reply = r_reply?;
        Ok(reply)
    }

    /// Generic routine to post a binary content
    fn post_bytes<V : de::DeserializeOwned>( &self, url : &str, request : Vec<u8>, token : &TokenType ) -> anyhow::Result<V>
    {
        let request_builder = reqwest::blocking::Client::new().post(url).timeout(TIMEOUT);
        let request_builder_2 = match token {
            Token(token_value) => {
                request_builder.header("token", token_value.clone())
            }
            Sid(sid_value) => {
                request_builder.header("sid", sid_value.clone())
            }
            TokenType::None => {
                request_builder
            }
        };
        let reply: V = request_builder_2.body(request).send()?.json()?;
        Ok(reply)
    }

    ///
    /// Put
    ///

    ///
    /// This PUT is for the TikaServer only, so no security token
    ///
    fn put_bytes<V : de::DeserializeOwned>(&self, url : &str, request : Vec<u8>) -> anyhow::Result<V>
    {
        let request_builder = reqwest::blocking::Client::new().put(url).timeout(TIMEOUT);
        let r2 = request_builder.header("Accept", "application/json");
        let reply: V = r2.body(request).send()?.json()?;
        Ok(reply)
    }

    fn put_bytes_retry<V : de::DeserializeOwned>(&self, url : &str, request : &Vec<u8>) -> anyhow::Result<V>
    {
        let mut r_reply;
        let mut count = 0;
        loop {
            let rr = request.clone();
            r_reply = self.put_bytes(url, rr);
            if r_reply.is_ok() || count >= MAX_HTTP_RETRY {
                break;
            }
            log_warn!("Url call failed, url=[{}], attempt=[{}]", url, count);
            count += 1;
        }
        let reply = r_reply?;
        Ok(reply)
    }

    ///
    /// Patch
    ///

    fn patch_data<V : de::DeserializeOwned>( &self, url : &str, token : &str ) -> anyhow::Result<V>
    {
        let reply: V = reqwest::blocking::Client::new()
            .patch(url)
            .timeout(TIMEOUT)
            .header("token", token.clone())
            .send()?.json()?;
        Ok(reply)
    }

    fn patch_data_retry<V : de::DeserializeOwned>( &self, url : &str, token : &str ) -> anyhow::Result<V>
    {
        let mut r_reply;
        let mut count = 0;
        loop {
            r_reply = self.patch_data(url, token);
            if r_reply.is_ok() || count >= MAX_HTTP_RETRY {
                break;
            }
            log_warn!("Url call failed, url=[{}], attempt=[{}]", url, count);
            count += 1;
        }
        let reply = r_reply?;
        Ok(reply)
    }

    ///
    /// Delete
    ///

    fn delete_data<V : de::DeserializeOwned>( &self, url : &str, token : &str ) -> anyhow::Result<V>
    {
        let reply: V = reqwest::blocking::Client::new()
            .delete(url)
            .timeout(TIMEOUT)
            .header("token", token.clone())
            .send()?.json()?;
        Ok(reply)
    }

    fn delete_data_retry<V : de::DeserializeOwned>( &self, url : &str, token : &str ) -> anyhow::Result<V>
    {
        let mut r_reply;
        let mut count = 0;
        loop {
            r_reply = self.delete_data(url, token);
            if r_reply.is_ok() || count >= MAX_HTTP_RETRY {
                break;
            }
            log_warn!("Url call failed, url=[{}], attempt=[{}]", url, count);
            count += 1;
        }
        let reply = r_reply?;
        Ok(reply)
    }


    ///
    /// Generic implementation of a delete action
    /// url_path : ex : admin-server/tag
    /// refcode : "eb65e" or 125
    ///
    fn delete_for_url<T>(&self, refcode: T, end_point: &str, token : &str) -> JsonErrorSet
    where T : Display {
        // let url = format!("http://{}:{}/{}/{}", &self.server.server_name, self.server.port, end_point,
        //                   refcode);

        let url = self.build_url_with_refcode(end_point, refcode);

        let reply : JsonErrorSet = match self.delete_data_retry(&url, token) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return  JsonErrorSet::from(HTTP_CLIENT_ERROR);
            },
        };
        reply
    }

    ///
    /// end_point , ex : "key", "tag"
    ///
    fn build_url(&self, end_point : &str) -> String {
        format!("http://{}:{}/{}/{}", &self.server_name, self.port, self.context, end_point)
    }

    ///
    /// end_point , ex : "key", "tag"
    ///
    fn build_url_with_refcode<T>(&self, end_point : &str, ref_code : T) -> String
    where T : Display
    {
        format!("http://{}:{}/{}/{}/{}", &self.server_name, self.port, self.context, end_point, ref_code)
    }

}

pub struct KeyManagerClient {
    server : WebServer,
}

impl KeyManagerClient {

    pub fn new(server_name : &str, port : u16) -> Self {
        Self {
            server : WebServer::new(server_name, port, "key-manager"),
        }
    }

    ///
    /// It is not supposed to return an error, so let's return the Reply directly
    ///
    pub fn add_key(&self, request : &AddKeyRequest, token : &TokenType) -> AddKeyReply {
        //let url = format!("http://{}:{}/{}/key", &self.server.server_name, self.server.port, self.server.context);
        let url = self.server.build_url("key");

        let headers = CustomHeaders {
            token_type: token.clone(),
            x_request_id: None,
            cek: None
        };

        let reply : AddKeyReply = match self.server.post_data_retry(&url, request, &headers) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return AddKeyReply {
                    success : false,  status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                };
            },
        };
        reply
    }


    pub fn get_key(&self, customer_code: &str, token : &str ) -> CustomerKeyReply {
        // http://localhost:{{PORT}}/key-manager/key/f1248fab
        let url = self.server.build_url_with_refcode("key", customer_code);

        let reply : CustomerKeyReply = match self.server.get_data_retry(&url, &Token(token.to_string())) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return CustomerKeyReply {
                    keys: Default::default(),
                    status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                };
            },
        };
        reply
    }

}


///
///
///
pub struct SessionManagerClient {
    server : WebServer,
}

impl SessionManagerClient {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServer::new(server_name, port, "session-manager"),
        }
    }

    pub fn open_session(&self, request : &OpenSessionRequest, token : &str, x_request_id: Option<u32>) -> OpenSessionReply {
        //let url = format!("http://{}:{}/session-manager/session", &self.server.server_name, self.server.port);
        let url = self.server.build_url("session");

        let headers = CustomHeaders {
            token_type: TokenType::Token(token.to_string()),
            x_request_id,
            cek: None
        };

        let reply : OpenSessionReply = match self.server.post_data_retry(&url, request, &headers) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return OpenSessionReply {
                    session_id : "".to_string(),
                    status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                };
            },
        };
        reply
    }


    pub fn get_session(&self, sid : &str, token : &str) -> SessionReply {

        // let url = format!("http://{}:{}/session-manager/session/{}", &self.server.server_name, self.server.port,
        //                   Uri::percent_encode(sid) );
        let url = self.server.build_url_with_refcode("session", Uri::percent_encode(sid) );

        let reply : SessionReply = match self.server.get_data_retry(&url, &Token(token.to_string())) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return SessionReply {
                    sessions: vec![],
                    status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                };
            },
        };
        reply
    }

}



///
///
///
pub struct AdminServerClient {
    server : WebServer,
}

impl AdminServerClient {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServer::new(server_name, port, "admin-server"),
        }
    }

    pub fn create_customer(&self, request : &CreateCustomerRequest, token: &str) -> CreateCustomerReply {
        // let url = format!("http://{}:{}/admin-server/customer", &self.server.server_name, self.server.port);
        let url = self.server.build_url("customer");

        let headers = CustomHeaders {
            token_type: TokenType::Token(token.to_string()),
            x_request_id: None,
            cek: None
        };

        let reply : CreateCustomerReply = match self.server.post_data_retry(&url, request, &headers) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return CreateCustomerReply {
                    customer_id: 0,
                    customer_code: "".to_string(),
                    admin_user_id: 0,
                    status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                };
            },
        };
        reply
    }

    pub fn customer_removable(&self, customer_code : &str, token : &str) -> JsonErrorSet {
        // let url = format!("http://{}:{}/admin-server/customer/removable/{}", &self.server.server_name, self.server.port,
        //                   Uri::percent_encode(customer_code) );
        let url = self.server.build_url_with_refcode("customer/removable", Uri::percent_encode(customer_code)  );

        let reply : JsonErrorSet = match self.server.patch_data_retry(&url, token) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return  JsonErrorSet::from(HTTP_CLIENT_ERROR);
            },
        };
        reply
    }


    pub fn delete_customer(&self, customer_code : &str, token : &str) -> JsonErrorSet {
        self.server.delete_for_url(customer_code, "customer", token)
    }


    pub fn login(&self, request : &LoginRequest) -> LoginReply {
        // let url = format!("http://{}:{}/admin-server/login", &self.server.server_name, self.server.port);
        let url = self.server.build_url("login" );

        let headers = CustomHeaders {
            token_type: TokenType::None,
            x_request_id: None,
            cek: None
        };

        let reply : LoginReply = match self.server.post_data_retry(&url, request, &headers) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return LoginReply {
                    session_id : "".to_string(),
                    status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                };
            },
        };
        reply
    }
}


///
/// Document Server
///
pub struct DocumentServerClient {
    server : WebServer,
}

impl DocumentServerClient {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServer::new(server_name, port, "document-server"),
        }
    }

    pub fn create_item(&self, request : &AddItemRequest, sid: &str) -> AddItemReply {
        // let url = format!("http://{}:{}/document-server/item", &self.server.server_name, self.server.port);
        let url = self.server.build_url("item");

        let headers = CustomHeaders {
            token_type: TokenType::Sid(sid.to_string()),
            x_request_id: None,
            cek: None
        };

        let reply : AddItemReply = match self.server.post_data_retry(&url, request, &headers) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return AddItemReply {
                    item_id: 0,
                    name: "".to_string(),
                    created: "".to_string(),
                    last_modified: None,
                    status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                };
            },
        };
        reply
    }


    ///
    ///
    ///
    pub fn get_item(&self, item_id : i64, sid : &str) -> GetItemReply {

        // let url = format!("http://{}:{}/document-server/item/{}", &self.server.server_name, self.server.port,
        //                   item_id );
        let url = self.server.build_url_with_refcode("item", item_id);

        let reply : GetItemReply = match self.server.get_data_retry(&url, &Sid(sid.to_string())) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return GetItemReply {
                    items: vec![],
                    status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                };
            },
        };
        reply
    }


    ///
    ///
    ///
    pub fn create_tag(&self, request : &AddTagRequest, sid: &str) -> AddTagReply {
        // let url = format!("http://{}:{}/document-server/tag", &self.server.server_name, self.server.port);
        let url = self.server.build_url("tag");

        let headers = CustomHeaders {
            token_type: TokenType::Sid(sid.to_string()),
            x_request_id: None,
            cek: None
        };

        let reply : AddTagReply = match self.server.post_data_retry(&url, request, &headers) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return AddTagReply {
                    tag_id: 0,
                    status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                };
            },
        };
        reply
    }


    ///
    ///
    ///
    pub fn get_all_tag(&self, sid: &str) -> GetTagReply {
        //let url = format!("http://{}:{}/document-server/tag", &self.server.server_name, self.server.port);
        let url = self.server.build_url("tag");

        let reply : GetTagReply = match self.server.get_data_retry(&url, &Sid(sid.to_string())) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return GetTagReply {
                    tags: vec![],
                    status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                };
            },
        };
        reply
    }

    ///
    ///
    ///
    pub fn delete_tag(&self, tag_id : i64, token : &str) -> JsonErrorSet {
        self.server.delete_for_url(tag_id, "tag", token)
    }

    ///
    ///
    ///
    pub fn fulltext_indexing(&self, raw_text: &str, file_name: &str, file_ref: &str, sid: &str) -> FullTextReply {
        let request = FullTextRequest {
            file_name: file_name.to_owned(),
            file_ref: file_ref.to_owned(),
            raw_text: raw_text.to_owned(),
        };

        let url = self.server.build_url("fulltext_indexing");

        let headers = CustomHeaders {
            token_type: TokenType::Sid(sid.to_string()),
            x_request_id: None,
            cek: None
        };

        let reply : FullTextReply = match self.server.post_data_retry(&url, &request, &headers) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return FullTextReply {
                    // item_id: 0,
                    part_count: 0,
                    status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                };
            },
        };
        reply
    }

}



///
/// Document Server
///
pub struct TikaServerClient {
    server : WebServer,
}

impl TikaServerClient {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServer::new(server_name, port, ""),
        }
    }

    pub fn parse_data(&self, request : &Vec<u8>) -> anyhow::Result<TikaParsing> {
        // curl -T birdy_tickets.pdf  http://localhost:9998/tika/text --header "Accept: application/json"
        let url = self.server.build_url("tika/text");
        let reply : TikaParsing = self.server.put_bytes_retry(&url, &request)?;
        Ok(reply)
    }

    ///
    /// Read meta information from the utf8 text request
    ///
    pub fn read_meta(&self, request : &str) ->  anyhow::Result<TikaMeta> {
        // curl -T birdy_tickets.pdf  http://localhost:9998/tika/text --header "Accept: application/json"
        let url = self.server.build_url("meta");

        let bytes = request.as_bytes().to_vec();
        let reply : TikaMeta = self.server.put_bytes_retry(&url, &bytes)?;
        Ok(reply)
    }

}

///
/// File Server
///
pub struct FileServerClient {
    server : WebServer,
}

impl FileServerClient {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServer::new(server_name, port, "file-server"),
        }
    }

    pub fn upload(&self, request: &Vec<u8>, sid: &str) -> UploadReply {
        // let url = format!("http://{}:{}/file-server/tag", &self.server.server_name, self.server.port);
        let url = self.server.build_url("upload");

        let reply : UploadReply = match self.server.post_bytes_retry(&url, request, &Sid(sid.to_string())) {
            Ok(x) => x,
            Err(e) => {
                log_error!("Technical error, [{}]", e);
                return UploadReply {
                    file_ref: "".to_string(),
                    size: 0,
                    status : JsonErrorSet::from(HTTP_CLIENT_ERROR),
                    block_count: 0
                };
            },
        };
        reply
    }
}

#[cfg(test)]
mod test
{
    use dkdto::{TikaParsing, UploadReply};
    use crate::request_client::{FileServerClient, TikaServerClient};

    fn put_data( url : &str, request : Vec<u8>) -> anyhow::Result<TikaParsing>
    {
        let request_builder = reqwest::blocking::Client::new().put(url);
        let r2 = request_builder.header("Accept", "application/json");
        let reply: TikaParsing = r2.body(request).send()?.json()?;
        Ok(reply)
    }

    #[test]
    fn test_put_basic() -> anyhow::Result<()> {
        let byte_buf: Vec<u8> = std::fs::read("C:/Users/denis/wks-poc/tika/Gandi_order.pdf")?;
        // curl -T birdy_tickets.pdf  http://localhost:9998/tika/text --header "Accept: application/json"
        let s = put_data("http://localhost:40010/tika/text", byte_buf )?;
        dbg!(&s);

        Ok(())
    }

    #[test]
    fn test_put_from_client() -> anyhow::Result<()> {
        let byte_buf: Vec<u8> = std::fs::read("C:/Users/denis/wks-poc/tika/Gandi_order.pdf")?;
        let client = TikaServerClient::new("localhost", 40010);
        let s = client.parse_data(&byte_buf)?;
        let _ = dbg!(&s);
        println!( "Extracted Text : [{}]", &s.x_tika_content);

        let meta = client.read_meta(&s.x_tika_content)?;

        println!( "Language : [{}]", &meta.language);

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
        println!( "Extracted Text : [{}]", &s.x_tika_content);

        let meta = client.read_meta(&s.x_tika_content)?;

        println!( "Language : [{}]", &meta.language);

        Ok(())
    }

    #[test]
    fn test_post_bytes_basic() -> anyhow::Result<()> {
        let byte_buf = std::fs::read("C:/Users/denis/wks-poc/tika/content.en.txt")?;

        let url = "http://localhost:30080/file-server/upload";

        let request_builder = reqwest::blocking::Client::new().post(url)
                            .header("Accept", "application/json")
                            .header("sid", "jPA93edZEA8pzJz5LvjnA1qEpWqNPf2Wsio_N9oHvQOKWDo3SwtS4hdqL2MOIb9x");
        let reply = request_builder.body(byte_buf).send()?.text()?;

        let _ = dbg!(reply);
        Ok(())
    }

    #[test]
    fn test_post_bin_from_client() -> anyhow::Result<()> {
        let byte_buf: Vec<u8> = std::fs::read("C:/Users/denis/wks-poc/tika/big_planet.pdf")?;
        let client = FileServerClient::new("localhost", 30080);
        let reply = client.upload(&byte_buf, "jPA93edZEA8pzJz5LvjnA1qEpWqNPf2Wsio_N9oHvQOKWDo3SwtS4hdqL2MOIb9x");
        let _ = dbg!(&s);
        println!( "Reply : [{:?}]", reply );

        // let meta = client.read_meta(&s.x_tika_content)?;
        //
        // println!( "Language : [{}]", &meta.language);

        Ok(())
    }

}