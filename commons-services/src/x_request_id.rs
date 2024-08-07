use crate::token_lib::SessionToken;
use axum::async_trait;
use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::request::Parts;
use axum::http::StatusCode;
use commons_error::*;
use doka_cli::request_client::TokenType;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::cmp::min;
use std::fmt::{Display, Formatter};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
pub struct XRequestID(Option<u32>);

impl XRequestID {
    pub fn new() -> Self {
        XRequestID(Some(Self::generate()))
    }
    pub fn from_value(val: Option<u32>) -> Self {
        XRequestID(val)
    }
    pub fn value(&self) -> Option<u32> {
        self.0
    }

    /// Regenerate a x_request_id if none
    pub fn new_if_null(&self) -> Self {
        let t_value = self.0.unwrap_or_else(|| Self::generate());
        XRequestID(Some(t_value))
    }

    fn generate() -> u32 {
        let mut rng = rand::thread_rng();
        rng.gen_range(0..1_000_000)
    }
}

impl Display for XRequestID {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(t) => {
                // TODO format with thousand-group separator, Ex : 987_789
                write!(f, "{}", t)
            }
            None => {
                write!(f, "None")
            }
        }
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for XRequestID
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let x_request_id = parts
            .headers
            .get("X-Request-ID")
            .and_then(|value| {
                u32::from_str(
                    value
                        .to_str()
                        .map_err(err_fwd!(
                            "⛔ Cannot parse the x_request_id from the header,set default to 0"
                        ))
                        .unwrap_or("0"),
                )
                .ok()
            })
            .map(|value| XRequestID(Some(value)));

        let xri = x_request_id.unwrap_or_else(|| XRequestID(None));
        Ok(xri)
    }
}

#[derive(Debug, Clone)]
pub struct Follower {
    pub token_type: TokenType,
    pub x_request_id: XRequestID,
}

impl Display for Follower {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let tt = match &self.token_type {
            TokenType::Token(tok) => {
                let limit = min(tok.len() - 2, 22);
                format!("T:{}...", &tok[..limit])
            }
            TokenType::Sid(sid) => {
                let limit = min(sid.len() - 2, 22);
                format!("S:{}...", &sid[..limit])
            }
            TokenType::None => "".to_string(),
        };
        write!(f, "(X:{} / {})", self.x_request_id, tt)
    }
}
