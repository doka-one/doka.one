use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};

use commons_error::*;
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;

use crate::COMMON_EDIBLE_KEY_PROPERTY;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SecurityToken(pub String);

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TokenHeader(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for SecurityToken
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("token")
            .and_then(|value| value.to_str().ok())
            .map(|value| SecurityToken(value.to_string()))
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "Missing or invalid token header".into(),
            ))?;

        Ok(token)
    }
}

impl SecurityToken {
    pub fn is_valid(&self) -> bool {
        let Ok(cek) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY).map_err(tr_fwd!()) else {
            return false;
        };
        !self.0.is_empty() && DkEncrypt::decrypt_str(&self.0, &cek).is_ok()
    }

    pub fn take_value(self) -> String {
        self.0
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct SessionToken(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for SessionToken
where
    S: Send + Sync,
{
    type Rejection = (StatusCode, String);

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("sid")
            .and_then(|value| value.to_str().ok())
            .map(|value| SessionToken(value.to_string()))
            .ok_or((
                StatusCode::UNAUTHORIZED,
                "Missing or invalid session token header".into(),
            ))?;

        Ok(token)
    }
}

impl SessionToken {
    pub fn is_valid(&self) -> bool {
        let Ok(cek) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY).map_err(tr_fwd!()) else {
            return false;
        };
        !self.0.is_empty() && DkEncrypt::decrypt_str(&self.0, &cek).is_ok()
    }

    pub fn take_value(self) -> String {
        self.0
    }
}
