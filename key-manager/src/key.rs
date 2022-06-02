use std::collections::HashMap;
use rocket_contrib::json::Json;
use log::*;
use commons_error::*;
use commons_pg::{CellValue, SQLChange, SQLConnection};
use commons_services::database_lib::open_transaction;
use commons_services::property_name::COMMON_EDIBLE_KEY_PROPERTY;
use commons_services::token_lib::SecurityToken;
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::{AddKeyReply, AddKeyRequest, JsonErrorSet};
use dkdto::error_codes::{CUSTOMER_KEY_ALREADY_EXISTS, INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR, INVALID_TOKEN, SUCCESS};
use dkdto::error_replies::ErrorReply;
use crate::search_key_by_customer_code;

pub fn add_key_delegate(customer: Json<AddKeyRequest>, security_token: SecurityToken) -> Json<AddKeyReply> {

    dbg!(&customer);

    // Check if the trace_id is valid
    if !security_token.is_valid() {
        return Json(AddKeyReply {
            success: false,
            status: JsonErrorSet::from(INVALID_TOKEN),
        });
    }
    let token = security_token.take_value();

    log_info!("🚀 Start add_key api, token_id={}", &token);

    let internal_database_error_reply = Json(AddKeyReply{ success : false, status: JsonErrorSet::from(INTERNAL_DATABASE_ERROR) });

    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    // Verify if the customer code exists in the system
    let entries  = match search_key_by_customer_code(&mut trans, Some(&customer.customer_code)) {
        Ok(x) => {x}
        Err(_) => {
            return internal_database_error_reply;
        }
    };

    if entries.contains_key(&customer.customer_code) {
        return Json(AddKeyReply{ success : false, status: JsonErrorSet::from(CUSTOMER_KEY_ALREADY_EXISTS) });
    }

    let Ok(cek) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY)
        .map_err(err_fwd!("💣 Cannot read the cek, token=[{}]", &token)) else {
        return Json(AddKeyReply::invalid_common_edible_key());
    };

    let new_customer_key = DkEncrypt::generate_random_key();
    dbg!(&new_customer_key);

    let internal_error_reply = Json(AddKeyReply{ success : false, status: JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR) });

    let enc_password = match DkEncrypt::encrypt_str(&new_customer_key, &cek) {
        Ok(v) => { v },
        Err(_) => { return internal_error_reply; },
    };

    let success = true;
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

    // TODO Handles the failure error !!!
    let _ = query.insert(&mut trans);

    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    if success {
        log_info!("😎 Customer key added with success");
    }

    let ret = AddKeyReply {
        success,
        status: JsonErrorSet::from(SUCCESS),
    };
    log_info!("🏁 End dd_key, token_id = {}, success={}", &token, success);
    Json(ret)
}