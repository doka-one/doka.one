use std::collections::HashMap;
use postgres::{Client, NoTls};
use rocket_contrib::json::Json;
use rs_uuid::iso::uuid_v4;
use commons_services::token_lib::SecurityToken;
use dkdto::{AddKeyRequest, CreateCustomerReply, CreateCustomerRequest, JsonErrorSet};
use dkdto::error_codes::{INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR, INVALID_PASSWORD, INVALID_TOKEN, SUCCESS};
use commons_error::*;
use commons_pg::{CellValue, SQLChange, SQLConnection, SQLDataSet, SQLQueryBlock, SQLTransaction};
use commons_services::database_lib::open_transaction;
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use doka_cli::request_client::KeyManagerClient;
use doka_cli::request_client::TokenType::Token;
use rocket::*;

use log::*;
use crate::{CS_SCHEMA, FS_SCHEMA, valid_password};

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

///
/// Create a brand new customer with schema and all
///
#[post("/customer", format = "application/json", data = "<customer_request>")]
pub fn create_customer(customer_request: Json<CreateCustomerRequest>, security_token: SecurityToken) -> Json<CreateCustomerReply> {
    dbg!(&customer_request);

    // Check if the token is valid
    if !security_token.is_valid() {
        return Json(CreateCustomerReply {
            customer_code: "".to_string(),
            customer_id : 0,
            admin_user_id : 0,
            status: JsonErrorSet::from(INVALID_TOKEN),
        });
    }

    let token = security_token.take_value();

    log_info!("🚀 Start create_customer api, token={}", &token);

    let internal_database_error_reply = Json(CreateCustomerReply {
        customer_code: "".to_string(),
        customer_id : 0,
        admin_user_id : 0,
        status : JsonErrorSet::from(INTERNAL_DATABASE_ERROR) });

    let internal_technical_error = Json(CreateCustomerReply {
        customer_code: "".to_string(),
        customer_id : 0,
        admin_user_id : 0,
        status : JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR) });

    // Check password validity

    // | length >= 8  + 1 symbol + 1 digit + 1 capital letter
    // | All chars are symbol OR [0-9, a-z, A-Z]
    if !valid_password(&customer_request.admin_password) {
        return Json(CreateCustomerReply {
            customer_code: "".to_string(),
            customer_id : 0,
            admin_user_id : 0,
            status : JsonErrorSet::from(INVALID_PASSWORD) });
    };

    log_info!("User password is compliant");

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

    log_info!("Generated a free customer =, [{}]", &customer_code);

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

    // Call the "key-manager" micro-service to create a secret master key
    let add_key_request = AddKeyRequest {
        customer_code : customer_code.clone(),
    };

    let km_host = get_prop_value("km.host");
    let km_port : u16 = get_prop_value("km.port").parse().unwrap();
    let kmc = KeyManagerClient::new( &km_host, km_port );
    let response = kmc.add_key(&add_key_request, Token(&token) );

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

    // dbg!(customer_id);

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

    // dbg!(user_id);

    // Close the transaction
    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        let _ = warning_cs_schema(&customer_code);
        let _ = warning_fs_schema(&customer_code);
        return internal_database_error_reply;
    }

    log_info!("😎 Customer created with success");

    log_info!("🏁 End create_customer, token_id = {}", &token);

    Json(CreateCustomerReply {
        customer_code,
        customer_id,
        admin_user_id : user_id,
        status: JsonErrorSet::from(SUCCESS),
    })
}