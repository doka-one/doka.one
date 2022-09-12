use std::collections::HashMap;
use anyhow::anyhow;
use rocket_contrib::json::Json;
use log::*;
use rocket::http::RawStr;
use commons_error::*;
use commons_pg::{CellValue, SQLChange, SQLConnection, SQLDataSet, SQLQueryBlock, SQLTransaction};
use commons_services::database_lib::open_transaction;
use commons_services::property_name::COMMON_EDIBLE_KEY_PROPERTY;
use commons_services::token_lib::SecurityToken;
use commons_services::x_request_id::{Follower, XRequestID};
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::{AddKeyReply, AddKeyRequest, CustomerKeyReply, EntryReply, JsonErrorSet};
use dkdto::error_codes::{CUSTOMER_KEY_ALREADY_EXISTS, INTERNAL_DATABASE_ERROR, INVALID_REQUEST, SUCCESS};
use dkdto::error_replies::ErrorReply;
use doka_cli::request_client::TokenType;
use doka_cli::request_client::TokenType::Token;


pub(crate) struct KeyDelegate {
    pub security_token: SecurityToken,
    pub follower: Follower,
}

impl KeyDelegate {
    pub fn new(security_token: SecurityToken, x_request_id: XRequestID) -> Self {
        Self {
            security_token,
            follower: Follower {
                x_request_id: x_request_id.new_if_null(),
                token_type: TokenType::None,
            }
        }
    }

    ///
    /// ✨ Add a key for customer code [customer]
    ///
    pub fn add_key(&mut self, customer: Json<AddKeyRequest>) -> Json<AddKeyReply> {

        log_info!("🚀 Start add_key api, customer_code=[{}], follower=[{}]", &customer.customer_code, &self.follower);

        if ! self.security_token.is_valid() {
            log_error!("💣 Invalid security token, token=[{:?}], follower=[{}]", &self.security_token, &self.follower);
            return Json(AddKeyReply::invalid_token_error_reply())
        }

        self.follower.token_type = Token(self.security_token.0.clone());

        let internal_database_error_reply = Json(AddKeyReply::internal_database_error_reply());

        let mut r_cnx = SQLConnection::new();
        let r_trans = open_transaction(&mut r_cnx).map_err(err_fwd!("💣 Open transaction error, follower=[{}]", &self.follower));
        let Ok(mut trans) = r_trans else {
            return internal_database_error_reply;
        };


        // Verify if the key already exists for the customer code

        let Ok(entries)  = self.search_key_by_customer_code(&mut trans, Some(&customer.customer_code))
                                                      .map_err(err_fwd!("💣 Search failed, customer code=[{}], follower=[{}]",
                                                          &customer.customer_code, &self.follower)) else {
            return internal_database_error_reply;
        };

        if entries.contains_key(&customer.customer_code) {
            log_error!("💣 The customer code already exists, customer code=[{}], follower=[{}]", &customer.customer_code, &self.follower);
            return Json(AddKeyReply::from_error(CUSTOMER_KEY_ALREADY_EXISTS));
        }

        log_info!("😎 The customer code has no existing key in the system, customer_code=[{}], follower=[{}]", &customer.customer_code, &self.follower);

        let Ok(cek) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY)
            .map_err(err_fwd!("💣 Cannot read the cek, follower=[{}]", &self.follower)) else {
            return Json(AddKeyReply::invalid_common_edible_key());
        };

        let new_customer_key = DkEncrypt::generate_random_key();

        let Ok(enc_password) = DkEncrypt::encrypt_str(&new_customer_key, &cek)
                                        .map_err(err_fwd!("💣 Cannot encrypt the new key, follower=[{}]", &self.follower)) else {
            return Json(AddKeyReply::internal_technical_error_reply());
        };

        let sql_insert = r#"INSERT INTO keymanager.customer_keys(
                            customer_code, ciphered_key)
                            VALUES (:p_customer_code, :p_ciphered_key)"#;


        let mut params : HashMap<String, CellValue> = HashMap::new();
        params.insert("p_customer_code".to_owned(), CellValue::from_raw_string(customer.customer_code.to_owned()));
        params.insert("p_ciphered_key".to_owned(), CellValue::from_raw_string(enc_password));

        let query = SQLChange {
            sql_query :  sql_insert.to_string(),
            params,
            sequence_name : "keymanager.customer_keys_id_seq".to_string(),
        };

        let Ok(key_id) = query.insert(&mut trans).map_err( err_fwd!("💣 Cannot insert the key, follower=[{}]", &self.follower)) else {
            return internal_database_error_reply;
        };

        if trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower)).is_err() {
            return internal_database_error_reply;
        }

        let ret = AddKeyReply {
            success : true,
            status: JsonErrorSet::from(SUCCESS),
        };

        log_info!("😎 Committed. Key created with success, key id=[{}], follower=[{}]", key_id, &self.follower);

        log_info!("🏁 End add_key, follower=[{}]", &self.follower);

        Json(ret)
    }


    // Search the keys for a customer_code
    // If the customer code is not present, returns all the keys
    fn search_key_by_customer_code(&self, mut trans : &mut SQLTransaction, customer_code : Option<&str>) -> anyhow::Result<HashMap<String, EntryReply>> {
        let p_customer_code = CellValue::from_opt_str(customer_code);

        let mut params = HashMap::new();
        params.insert("p_customer_code".to_owned(), p_customer_code);

        let query = SQLQueryBlock {
            sql_query : r"SELECT id, customer_code, ciphered_key FROM keymanager.customer_keys
                    WHERE customer_code = :p_customer_code OR :p_customer_code IS NULL ".to_string(),
            start : 0,
            length : None,
            params,
        };

        let mut sql_result : SQLDataSet =  query.execute(&mut trans).map_err(err_fwd!("Query failed, [{}]", &query.sql_query))?;

        let mut entries= HashMap::new();
        while sql_result.next() {
            let id : i64 = sql_result.get_int("id").ok_or(anyhow!("Wrong column: id"))?;
            let customer_code: String = sql_result.get_string("customer_code").ok_or(anyhow!("Wrong column: customer_code"))?;
            let ciphered_key: String = sql_result.get_string("ciphered_key").ok_or(anyhow!("Wrong column: ciphered_key"))?;

            let key_info = EntryReply {
                key_id : id,
                customer_code,
                ciphered_key,
                active: true,
            };

            let _ = &entries.insert(key_info.customer_code.clone(), key_info);
        }

        Ok(entries)
    }


    ///
    /// ✨ Read the key for a specific customer code [customer_code]
    ///
    pub fn read_key(&mut self, customer_code: &RawStr) -> Json<CustomerKeyReply> {

        log_info!("🚀 Start read_key api, customer_code=[{}], follower=[{}]", customer_code, &self.follower);

        // Check if the token is valid
        if ! self.security_token.is_valid() {
            log_error!("💣 Invalid security token, token=[{:?}], follower=[{}]", &self.security_token, &self.follower);
            return Json(CustomerKeyReply::invalid_token_error_reply())
        }

        self.follower.token_type = TokenType::Token(self.security_token.0.clone());

        let Ok(customer_code) = customer_code.percent_decode()
                        .map_err(err_fwd!("💣 Invalid input parameter, customer_code=[{}], follower=[{}]", customer_code, &self.follower) ) else {
            return Json( CustomerKeyReply::from_error(INVALID_REQUEST) )
        };

        // customer key to return.
        let customer_key_reply = self.read_entries(Some(&customer_code));

        if customer_key_reply.status.error_code != 0 {
            log_error!("💣 We were not able to read the entries for the customer_code=[{}], follower=[{}]", customer_code, &self.follower);
            return Json(customer_key_reply);
        }

        log_info!("😎 Key read with success, number of keys=[{}], follower=[{}]", customer_key_reply.keys.len(), &self.follower);

        log_info!("🏁 End read_key api, customer_code=[{}], follower=[{}]", customer_code, &self.follower);

        Json(customer_key_reply)

    }

    ///
    /// ✨ Read all the keys
    ///
    pub fn key_list(&mut self) -> Json<CustomerKeyReply> {
        log_info!("🚀 Start key list api, follower=[{}]", &self.follower);

        // Check if the token is valid
        if ! self.security_token.is_valid() {
            log_error!("💣 Invalid security token, token=[{:?}], follower=[{}]", &self.security_token, &self.follower);
            return Json(CustomerKeyReply::invalid_token_error_reply())
        }

        self.follower.token_type = TokenType::Token(self.security_token.0.clone());

        // List of customer keys to return.
        let customer_key_reply = self.read_entries(None);

        if customer_key_reply.status.error_code != 0 {
            log_error!("💣 We were not able to read the entries, follower=[{}]", &self.follower);
            return Json(customer_key_reply);
        }

        log_info!("😎 Key read with success, number of keys=[{}], follower=[{}]", customer_key_reply.keys.len(), &self.follower);

        log_info!("🏁 End key list api, follower=[{}]", &self.follower);
        Json(customer_key_reply)
    }


    // Read the list of users from the DB
    fn read_entries( &self, customer_code : Option<&str> ) -> CustomerKeyReply {

        let internal_database_error_reply = CustomerKeyReply{ keys: HashMap::new(), status: JsonErrorSet::from(INTERNAL_DATABASE_ERROR) };

        let mut r_cnx = SQLConnection::new();
        let r_trans = open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error, follower=[{}]", &self.follower));
        let Ok(mut trans) = r_trans else  {
            return internal_database_error_reply;
        };

        let Ok(entries) = self.search_key_by_customer_code(&mut trans, customer_code)
                                    .map_err(err_fwd!("Key search failed, follower=[{}]", &self.follower)) else {
            return internal_database_error_reply;
        };

        if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
            return internal_database_error_reply;
        }

        log_info!("Number of key found, number of keys=[{}], follower=[{}]", entries.len(), &self.follower);

        CustomerKeyReply{ keys: entries, status: JsonErrorSet::from(SUCCESS) }

    }


}


