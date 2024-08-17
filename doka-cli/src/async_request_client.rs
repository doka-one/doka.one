use crate::request_client::TokenType::{Sid, Token};
use crate::request_client::{CustomHeaders, TokenType};
use anyhow::anyhow;
use commons_error::*;
use dkdto::error_codes::HTTP_CLIENT_ERROR;
use dkdto::{OpenSessionReply, OpenSessionRequest, SimpleMessage, WebResponse, WebTypeBuilder};
use log::warn;
use reqwest::{Client, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::{de, Serialize};
use std::fmt::Display;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use tokio::time::sleep;
use url::Url;

const TIMEOUT: Duration = Duration::from_secs(60 * 60);
const MAX_HTTP_RETRY: u32 = 5;
const LAPS: u32 = 2_000;

#[derive(Clone)]
pub struct SessionManagerClientAsync {
    server: WebServerAsync,
}

impl SessionManagerClientAsync {
    pub fn new(server_name: &str, port: u16) -> Self {
        Self {
            server: WebServerAsync::new(server_name, port, "session-manager"),
        }
    }

    pub async fn open_session(
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

        self.server.post_data_retry(&url, request, &headers).await
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
        Self {
            server_name: server_name.to_owned(),
            port,
            context: context.to_owned(),
        }
    }

    async fn retry<F, T>(&self, mut operation: F) -> anyhow::Result<T>
    where
        // F: FnMut() -> anyhow::Result<T>,
        F: FnMut() -> Pin<Box<dyn Future<Output = anyhow::Result<T>> + Send>>,
    {
        let mut count: u32 = 0;
        loop {
            let operation_result = operation().await;
            if operation_result.is_ok() || count >= MAX_HTTP_RETRY {
                return operation_result;
            }
            let t = LAPS as u64;
            eprintln!("Wait for {} ms", t);
            sleep(Duration::from_millis(t)).await;
            log_warn!("Operation failed, attempt=[{}]", count);
            count += 1;
        }
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

        match /*self.retry(post_data).await*/ ret {
            Ok(response) => response,
            Err(_) => WebResponse::from_errorset(&HTTP_CLIENT_ERROR),
        }
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
                        Err(e) => {
                            return Err(anyhow!(
                                "Failed to parse error response: {}",
                                e.to_string()
                            ))
                        }
                    }
                } else {
                    let value: Result<V, reqwest::Error> = v.json().await;
                    match value {
                        Ok(v_value) => WebResponse::from_item(status_code.as_u16(), v_value),
                        Err(e) => {
                            return Err(anyhow!(
                                "Failed to parse successful response: {}",
                                e.to_string()
                            ))
                        }
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

    fn add_header(
        request_builder: reqwest::blocking::RequestBuilder,
        token: &TokenType,
    ) -> reqwest::blocking::RequestBuilder {
        match token {
            Token(token_value) => request_builder.header("token", token_value.clone()),
            Sid(sid_value) => request_builder.header("sid", sid_value.clone()),
            TokenType::None => request_builder,
        }
    }
}
