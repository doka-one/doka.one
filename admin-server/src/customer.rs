use std::collections::HashMap;
use postgres::{Client, NoTls};
use rocket_contrib::json::Json;
use rs_uuid::iso::uuid_v4;
use commons_services::token_lib::SecurityToken;
use dkdto::{AddKeyRequest, CreateCustomerReply, CreateCustomerRequest, JsonErrorSet};
use dkdto::error_codes::{INTERNAL_DATABASE_ERROR, INVALID_PASSWORD, INVALID_REQUEST, INVALID_TOKEN, SUCCESS};
use commons_error::*;
use commons_pg::{CellValue, SQLChange, SQLConnection, SQLDataSet, SQLQueryBlock, SQLTransaction};
use commons_services::database_lib::open_transaction;
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use doka_cli::request_client::{KeyManagerClient, TokenType};
use rocket::*;

use log::*;
use rocket::http::RawStr;
use commons_services::x_request_id::{XRequestID, TwinId};
use dkdto::error_replies::ErrorReply;
use crate::dk_password::valid_password;
use crate::schema_cs::CS_SCHEMA;
use crate::schema_fs::FS_SCHEMA;

struct DbServerInfo {
    host: String,
    port : u16,
    db_name : String,
    db_user : String,
    password : String,
}

impl DbServerInfo {

    pub fn for_cs() -> Self {
        Self {
            host: get_prop_value("cs_db.hostname"),
            port: get_prop_value("cs_db.port").parse().unwrap(),
            db_name: get_prop_value("cs_db.name"),
            db_user: get_prop_value("cs_db.user"),
            password: get_prop_value("db.password"), // Careful, it's not cs_db
        }
    }

    pub fn for_fs() -> Self {
        Self {
            host: get_prop_value("fs_db.hostname"),
            port: get_prop_value("fs_db.port").parse().unwrap(),
            db_name: get_prop_value("fs_db.name"),
            db_user: get_prop_value("fs_db.user"),
            password: get_prop_value("db.password"), // Careful, it's not fs_db
        }
    }

}


///
/// Check if the customer code is not taken (true if it is not)
///
fn check_code_not_taken(mut trans : &mut SQLTransaction, customer_code : &str) -> anyhow::Result<bool> {
    let p_customer_code = CellValue::from_raw_string(customer_code.to_owned());
    let mut params = HashMap::new();
    params.insert("p_customer_code".to_owned(), p_customer_code);
    let sql_query = r#" SELECT 1 FROM dokaadmin.customer WHERE code = :p_customer_code"#.to_owned();

    let query = SQLQueryBlock {
        sql_query,
        params,
        start : 0,
        length : Some(1),
    };

    let sql_result : SQLDataSet =  query.execute(&mut trans).map_err(err_fwd!("Query failed, [{}]", &query.sql_query))?;
    Ok(sql_result.len() == 0)
}


fn generate_cs_schema_script(customer_code : &str) -> String {
    let template = CS_SCHEMA.to_string();
    let script = template.replace("{customer_schema}", format!("cs_{}", customer_code).as_str() );
    script
}

fn generate_fs_schema_script(customer_code : &str) -> String {
    let template = FS_SCHEMA.to_string();
    let script = template.replace("{customer_schema}", format!("fs_{}", customer_code).as_str() );
    script
}

fn warning_cs_schema(customer_code: &str) -> anyhow::Result<()> {
    // we don't drop the schema automatically, it could lead to user data loss.
    let dbi = DbServerInfo::for_cs();
    log_warn!("Please verify if the schema cs_{} is not in the database=[{}]", customer_code, dbi.db_name);
    Ok(())
}

fn warning_fs_schema(customer_code: &str) -> anyhow::Result<()> {
    // we don't drop the schema automatically, it could lead to user data loss.
    let dbi = DbServerInfo::for_fs();
    log_warn!("Please verify if the schema fs_{} is not in the database=[{}]", customer_code, dbi.db_name);
    Ok(())
}


fn set_removable_flag_customer_from_db( trans : &mut SQLTransaction, customer_code : &str ) -> anyhow::Result<bool> {
    let mut params = HashMap::new();
    params.insert("p_customer_code".to_owned(), CellValue::from_raw_string(customer_code.to_string()));

    let query = SQLChange {
        sql_query: r"UPDATE dokaadmin.customer SET is_removable = TRUE  WHERE code = :p_customer_code".to_string(),
        params,
        sequence_name: "".to_string(),
    };
    let nb = query.update(trans).map_err(err_fwd!("Query failed"))?;

    if nb == 0 {
        return Err(anyhow::anyhow!("We did not set any removable flag for any customer"));
    }

    Ok(true)
}


pub (crate) fn set_removable_flag_customer_delegate(customer_code: &RawStr, security_token: SecurityToken) -> Json<JsonErrorSet> {
    // Check if the token is valid
    if !security_token.is_valid() {
        return  Json(JsonErrorSet::from(INVALID_TOKEN));
    }

    let token = security_token.take_value();

    log_info!("🚀 Start set_removable_flag_customer api, token={}", &token);

    let customer_code = match customer_code.percent_decode().map_err(err_fwd!("Invalid input parameter [{}]", customer_code) ) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return Json(JsonErrorSet::from(INVALID_REQUEST));
        }
    };


    let internal_database_error_reply = Json(JsonErrorSet::from(INTERNAL_DATABASE_ERROR));

    // | Open the transaction
    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    if set_removable_flag_customer_from_db(&mut trans, &customer_code).map_err(err_fwd!("")).is_err() {
        return internal_database_error_reply;
    }

    // Close the transaction
    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    log_info!("😎 Set removable flag with success");

    log_info!("🏁 End set_removable_flag_customer, token_id = {}", &token);

    Json(JsonErrorSet::from(SUCCESS))

}

///
/// Create a brand new customer with schema and all
///
#[post("/customer", format = "application/json", data = "<customer_request>")]
pub fn create_customer(customer_request: Json<CreateCustomerRequest>, security_token: SecurityToken, x_request_id: XRequestID) -> Json<CreateCustomerReply> {
    log_debug!("customer_request = [{:?}]", &customer_request);
    log_debug!("x_request_id = [{}]", &x_request_id);

    let x_request_id = x_request_id.new_if_null();
    log_debug!("x_request_id = [{}]", &x_request_id);

    log_info!("🚀 Start create_customer api, customer name=[{}], x_request_id=[{}]", &customer_request.customer_name, &x_request_id);

    // Check if the token is valid
    if !security_token.is_valid() {
        return Json(CreateCustomerReply::invalid_token_error_reply());
    }

    let token = security_token.take_value();

    let twin_id = TwinId {
        token_type: TokenType::Token(&token),
        x_request_id: x_request_id
    };

    log_info!("😎 Security token is valid, twin_id=[{}]", &twin_id);
    let internal_database_error_reply = Json(CreateCustomerReply::internal_database_error_reply());
    let internal_technical_error = Json(CreateCustomerReply::internal_technical_error_reply());

    // Check password validity

    // | length >= 8  + 1 symbol + 1 digit + 1 capital letter
    // | All chars are symbol OR [0-9, a-z, A-Z]
    if !valid_password(&customer_request.admin_password) {
        return Json(CreateCustomerReply::from_error(INVALID_PASSWORD));
    };

    log_info!("😎 User password is compliant, twin_id=[{}]", &twin_id);

    // Open the transaction
    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    // Generate the customer code
    let customer_code: String;
    loop {
        let big_code  = uuid_v4();
        let code_parts : Vec<&str> = big_code.split('-').collect();
        let customer_code_str = *code_parts.get(0).unwrap();

        // Verify if the customer code is unique in the table (loop)
        if check_code_not_taken(&mut trans, customer_code_str).unwrap() {
            customer_code = String::from(customer_code_str);
            break;
        }
    }

    log_info!("😎 Generated a free customer=[{}], twin_id=[{}]", &customer_code, &twin_id);

    // Create the schema

    fn run_cs_script(customer_code: &str) -> anyhow::Result<()> {
        // * Open a transaction on the cs database
        let dbi = DbServerInfo::for_cs();
        // "postgresql://denis:<password>@pg13:5432/cs_dev_1";
        let url = format!("postgresql://{}:{}@{}:{}/{}", dbi.db_user, dbi.password, dbi.host, dbi.port, dbi.db_name);

        let mut cs_cnx= Client::connect(&url, NoTls).map_err(err_fwd!("Cannot connect the CS database"))?;

        // Run the commands to create the tables & co
        let batch_script = generate_cs_schema_script(customer_code);

        cs_cnx.batch_execute(&batch_script).map_err(err_fwd!("CS batch script error"))?;

        Ok(())
    }

    if let Err(e) = run_cs_script(&customer_code) {
        log_error!("CS schema batch failed, error [{}]", e);
        trans.rollback();
        return internal_database_error_reply;
    }

    log_info!("😎 Created the CS schema, customer=[{}], twin_id=[{}]", &customer_code, &twin_id);

    fn run_fs_script(customer_code: &str) -> anyhow::Result<()> {
        // * Open a transaction on the cs database
        let dbi = DbServerInfo::for_fs();
        // "postgresql://denis:<password>@pg13:5432/fs_dev_1";
        let url = format!("postgresql://{}:{}@{}:{}/{}", dbi.db_user, dbi.password, dbi.host, dbi.port, dbi.db_name);

        let mut fs_cnx= Client::connect(&url, NoTls).map_err(err_fwd!("Cannot connect the FS database"))?;

        // Run the commands to create the tables & co
        let batch_script = generate_fs_schema_script(customer_code);

        fs_cnx.batch_execute(&batch_script).map_err(err_fwd!("FS batch script error"))?;

        Ok(())
    }

    if let Err(e) = run_fs_script(&customer_code) {
        log_error!("FS schema batch failed, error [{}]", e);
        trans.rollback();
        let _ = warning_cs_schema(&customer_code);
        return internal_database_error_reply;
    }

    log_info!("😎 Created the FS schema, customer=[{}], twin_id=[{}]", &customer_code, &twin_id);

    // Call the "key-manager" micro-service to create a secret master key
    let add_key_request = AddKeyRequest {
        customer_code : customer_code.clone(),
    };

    let km_host = get_prop_value("km.host");
    let km_port : u16 = get_prop_value("km.port").parse().unwrap();
    let kmc = KeyManagerClient::new( &km_host, km_port );
    let response = kmc.add_key(&add_key_request, /*TokenType::Token(&token)*/ twin_id.token_type );

    if ! response.success {
        log_error!("Key Manager failed with status [{:?}]", response.status);
        let _ = warning_cs_schema(&customer_code);
        let _ = warning_fs_schema(&customer_code);
        return internal_technical_error;
    }

    // Insert the customer in the table

    let mut params : HashMap<String, CellValue> = HashMap::new();
    params.insert("p_code".to_owned(), CellValue::from_raw_string(customer_code.clone()));
    params.insert("p_full_name".to_owned(), CellValue::from_raw_string(customer_request.customer_name.clone()));
    params.insert("p_default_language".to_owned(), CellValue::from_raw_string("ENG".to_owned()));
    params.insert("p_default_time_zone".to_owned(), CellValue::from_raw_string("Europe/Paris".to_owned()));

    let sql_insert = SQLChange {
        sql_query: r#"INSERT INTO dokaadmin.customer (code, full_name, default_language, default_time_zone)
                        VALUES (:p_code, :p_full_name, :p_default_language, :p_default_time_zone) "#.to_string(),
        params,
        sequence_name: "dokaadmin.customer_id_seq".to_string(),
    };

    let customer_id = match sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion of a new customer failed")) {
        Ok(x) => {x}
        Err(_) => {
            let _ = warning_cs_schema(&customer_code);
            let _ = warning_fs_schema(&customer_code);
            return internal_database_error_reply;
        }
    };

    log_info!("😎 Inserted new customer, customer id=[{}], twin_id=[{}]", customer_id, &twin_id);

    // Insert the admin user in the table

    // | Compute the hashed password
    let password_hash = DkEncrypt::hash_password(&customer_request.admin_password);

    let mut params : HashMap<String, CellValue> = HashMap::new();
    params.insert("p_login".to_owned(), CellValue::from_raw_string(customer_request.email.clone()));
    params.insert("p_full_name".to_owned(), CellValue::from_raw_string(customer_request.email.clone()));
    params.insert("p_password_hash".to_owned(), CellValue::from_raw_string(password_hash.clone()));
    params.insert("p_default_language".to_owned(), CellValue::from_raw_string("ENG".to_owned()));
    params.insert("p_default_time_zone".to_owned(), CellValue::from_raw_string("Europe/Paris".to_owned()));
    params.insert("p_admin".to_owned(), CellValue::from_raw_bool(true));
    params.insert("p_customer_id".to_owned(), CellValue::from_raw_int(customer_id));

    let sql_insert = SQLChange {
        sql_query: r#"INSERT INTO dokaadmin.appuser(
        login, full_name, password_hash, default_language, default_time_zone, admin, customer_id)
        VALUES (:p_login, :p_full_name, :p_password_hash, :p_default_language, :p_default_time_zone, :p_admin, :p_customer_id)"#.to_string(),
        params,
        sequence_name: "dokaadmin.appuser_id_seq".to_string(),
    };

    let user_id = match sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion of a new admin user failed")) {
        Ok(x) => {x}
        Err(_) => {
            let _ = warning_cs_schema(&customer_code);
            let _ = warning_fs_schema(&customer_code);
            return internal_database_error_reply;
        }
    };

    log_info!("😎 Inserted new user, user id=[{}], twin_id=[{}]", user_id, &twin_id);

    // Close the transaction
    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        let _ = warning_cs_schema(&customer_code);
        let _ = warning_fs_schema(&customer_code);
        return internal_database_error_reply;
    }

    log_info!("😎 Committed. Customer created with success, twin_id=[{}]", &twin_id);

    log_info!("🏁 End create_customer, twin_id=[{}]", &twin_id);

    Json(CreateCustomerReply {
        customer_code,
        customer_id,
        admin_user_id : user_id,
        status: JsonErrorSet::from(SUCCESS),
    })
}

fn drop_schema_from_db( trans : &mut SQLTransaction, customer_code : &str ) -> anyhow::Result<bool> {
    let query = SQLChange {
        sql_query: format!( r"DROP SCHEMA cs_{} CASCADE", customer_code ),
        params : Default::default(),
        sequence_name: "".to_string(),
    };
    let _ = query.batch(trans).map_err(err_fwd!("Dropping the schema failed, customer_code=[{}]", customer_code))?;

    Ok(true)
}

fn search_customer( trans : &mut SQLTransaction, customer_code : &str ) -> anyhow::Result<i64> {
    let mut params = HashMap::new();
    params.insert("p_customer_code".to_owned(), CellValue::from_raw_string(customer_code.to_string()));

    let query = SQLQueryBlock {
        sql_query: "SELECT id FROM dokaadmin.customer WHERE code = :p_customer_code AND is_removable = TRUE".to_string(),
        start: 0,
        length: None,
        params
    };
    let mut data_set = query.execute(trans).map_err(err_fwd!("Query failed"))?;

    if data_set.len() == 0 {
        return Err(anyhow::anyhow!("Customer code not found"));
    }
    let _ = data_set.next();
    let customer_id = data_set.get_int("id").unwrap_or(0i64);
    Ok(customer_id)
}

fn delete_user_from_db( trans : &mut SQLTransaction, customer_code : &str ) -> anyhow::Result<bool> {
    let mut params = HashMap::new();
    params.insert("p_customer_code".to_owned(), CellValue::from_raw_string(customer_code.to_string()));

    let query = SQLChange {
        sql_query: r"DELETE FROM dokaadmin.appuser WHERE customer_id IN
        (SELECT id FROM dokaadmin.customer WHERE code = :p_customer_code AND is_removable = TRUE)".to_string(),
        params,
        sequence_name: "".to_string(),
    };
    let nb_delete = query.delete(trans).map_err(err_fwd!("Query failed"))?;

    if nb_delete == 0 {
        return Err(anyhow::anyhow!("We did not delete any user for the customer"));
    }

    Ok(true)
}


fn delete_customer_from_db( trans : &mut SQLTransaction, customer_code : &str ) -> anyhow::Result<bool> {
    let mut params = HashMap::new();
    params.insert("p_customer_code".to_owned(), CellValue::from_raw_string(customer_code.to_string()));

    let query = SQLChange {
        sql_query: r"DELETE FROM dokaadmin.customer WHERE code = :p_customer_code AND is_removable = TRUE".to_string(),
        params,
        sequence_name: "".to_string(),
    };
    let nb = query.delete(trans).map_err(err_fwd!("Query failed"))?;

    if nb == 0 {
        return Err(anyhow::anyhow!("We did not delete any customer"));
    }

    Ok(true)
}



///
/// Delete a customer with schema and all
///
#[delete("/customer/<customer_code>")]
pub fn delete_customer(customer_code: &RawStr, security_token: SecurityToken) -> Json<JsonErrorSet> {

    // Check if the token is valid
    if !security_token.is_valid() {
        return Json(JsonErrorSet::from(INVALID_TOKEN));
    }

    let token = security_token.take_value();

    log_info!("🚀 Start delete_customer api, token={}", &token);

    let customer_code = match customer_code.percent_decode().map_err(err_fwd!("Invalid input parameter [{}]", customer_code) ) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return Json(JsonErrorSet::from(INVALID_REQUEST));
        }
    };

    let internal_database_error_reply = Json(JsonErrorSet::from(INTERNAL_DATABASE_ERROR));

    // | Open the transaction
    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    // Check if the customer is removable (flag is_removable)

    let _customer_id = match search_customer(&mut trans, &customer_code) {
        Ok(x) => x,
        Err(_) => { return internal_database_error_reply; },
    };

    // Clear the customer table and user

    // TODO Look if we display the "e" in the fwd!
    if delete_user_from_db(&mut trans, &customer_code).map_err(err_fwd!("")).is_err() {
        return internal_database_error_reply;
    }

    if delete_customer_from_db(&mut trans, &customer_code).map_err(err_fwd!("")).is_err() {
        return internal_database_error_reply;
    }

    // Remove the db schema

    if drop_schema_from_db(&mut trans, &customer_code).map_err(err_fwd!("")).is_err() {
        return internal_database_error_reply;
    }


    // Close the transaction
    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    log_info!("😎 Customer delete created with success");

    log_info!("🏁 End delete_customer, token_id = {}", &token);

    Json(JsonErrorSet::from(SUCCESS))
}