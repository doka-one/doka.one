use std::cmp::min;
use serde::{Serialize, Deserialize};
use std::fmt::{Display, Formatter};
use rand::Rng;
use rocket::{Request, request};
use rocket::request::FromRequest;
use commons_error::*;
use doka_cli::request_client::{TokenType};

#[derive(Serialize, Deserialize, Debug,Copy,Clone)]
pub struct XRequestID(Option<u32>);

impl XRequestID {
    pub fn new() -> Self {
        XRequestID(Some(Self::generate()))
    }
    pub fn from_value(val : Option<u32>) -> Self {
        XRequestID(val)
    }
    pub fn value(&self) -> Option<u32> {
        self.0
    }

    /// Regenerate a x_request_id if none
    pub fn new_if_null(&self) -> Self {
        let t_value = match self.0 {
            Some(t) => {t}
            None => {Self::generate()}
        };
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


impl<'a, 'r> FromRequest<'a, 'r> for XRequestID {
    type Error = ();
    fn from_request(my_request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        //dbg!(my_request);
        let map = my_request.headers();

        let x_request_id = map.get_one("X-Request-ID").map(|t|
            t.parse().map_err(err_fwd!("Cannot parse the x_request_id from the header,set default to 0")).unwrap_or(0u32) );

        request::Outcome::Success(XRequestID(x_request_id))
    }
}

#[derive(Debug,Clone)]
pub struct Follower {
    pub token_type : TokenType,
    pub x_request_id: XRequestID,
}

impl Display for Follower {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let tt = match &self.token_type {
            TokenType::Token(tok) => {
                let limit = min(tok.len(), 22);
                format!("T:{}...", &tok[..limit])
            }
            TokenType::Sid(sid) => {
                let limit = min(sid.len(), 22);
                format!("S:{}...", &sid[..limit])
            }
            TokenType::None => {"".to_string()}
        };
        write!(f, "({} / {})", self.x_request_id, tt)
    }
}