use serde::{Serialize, Deserialize};
use rocket::{Request, request};
use rocket::request::FromRequest;
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;


#[derive(Serialize, Deserialize, Debug)]
pub struct SecurityToken(pub String);

impl<'a, 'r> FromRequest<'a, 'r> for SecurityToken {
    type Error = ();
    fn from_request(my_request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let map = my_request.headers();
        let token_id = map.get_one("token").unwrap_or("");
        request::Outcome::Success(SecurityToken(token_id.to_string()))
    }
}

impl SecurityToken {
    pub fn is_valid(&self) -> bool {
        let cek = get_prop_value("cek");
        !self.0.is_empty() && DkEncrypt::decrypt_str(&self.0, &cek).is_ok()
    }

    pub fn take_value(self) -> String {
        self.0
    }
}


#[derive(Serialize, Deserialize, Debug)]
pub struct SessionToken(pub String);

impl<'a, 'r> FromRequest<'a, 'r> for SessionToken {
    type Error = ();
    fn from_request(my_request: &'a Request<'r>) -> request::Outcome<Self, Self::Error> {
        let map = my_request.headers();
        let token_id = map.get_one("sid").unwrap_or("");
        request::Outcome::Success(SessionToken(token_id.to_string()))
    }
}

impl SessionToken {
    pub fn is_valid(&self) -> bool {
        let cek = get_prop_value("cek");
        !self.0.is_empty() && DkEncrypt::decrypt_str(&self.0, &cek).is_ok()
    }

    pub fn take_value(self) -> String {
        self.0
    }
}
