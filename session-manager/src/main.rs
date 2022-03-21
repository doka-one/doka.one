#![feature(proc_macro_hygiene, decl_macro)]

use std::path::Path;
use std::process::exit;
use log::*;
use rocket::*;
use rocket_contrib::json::Json;
use dkconfig::conf_reader::{read_config};
use std::collections::HashMap;
use chrono::{Utc};
use rocket::http::RawStr;
use commons_error::*;
use rocket_contrib::templates::Template;
use std::time::SystemTime;
use rocket::config::Environment;
use commons_pg::{SQLConnection, SQLChange, CellValue, SQLQueryBlock, SQLDataSet, SQLTransaction, init_db_pool};
use commons_services::read_cek_and_store;
use commons_services::token_lib::SecurityToken;
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use dkcrypto::dk_crypto::DkEncrypt;

use dkdto::{EntrySession, OpenSessionReply, OpenSessionRequest, SessionReply, JsonErrorSet};
use dkdto::error_codes::{INTERNAL_DATABASE_ERROR, INVALID_REQUEST, INVALID_TOKEN, SESSION_CANNOT_BE_RENEWED, SESSION_NOT_FOUND, SESSION_TIMED_OUT, SUCCESS};

///
/// Find a session from its sid
///
#[get("/session/<session_id>")]
fn read_session(session_id: &RawStr, security_token: SecurityToken) -> Json<SessionReply> {

    // Check if the token is valid
    if ! security_token.is_valid() {
        log_error!("Invalid token {:?}", &security_token);
        return Json(SessionReply { sessions : vec![], status: JsonErrorSet::from(INVALID_TOKEN) } )
    }
    let token = security_token.take_value();

    log_info!("🚀 Start read_session api, trace_id=[{:?}]", token);

    let session_id = match session_id.percent_decode().map_err(err_fwd!("Invalid input parameter, [{}]", session_id) ) {
        Ok(s) => s.to_string(),
        Err(_) => {
            return Json(SessionReply { sessions : vec![], status: JsonErrorSet::from(INVALID_REQUEST) } )
        }
    };

    // Open Db connection

    let internal_database_error_reply = Json(SessionReply{ sessions : vec![], status: JsonErrorSet::from(INTERNAL_DATABASE_ERROR) });

    let mut cnx = match SQLConnection::new().map_err(err_fwd!("New Db connection failed")){
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    let mut trans = match cnx.sql_transaction().map_err(err_fwd!("Error transaction")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    // Query the sessions to find the right one
    let sessions = match search_session_by_sid(&mut trans, Some(&session_id) ) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    // Customer key to return
    let mut session_reply = SessionReply{ sessions, status: JsonErrorSet::from(SUCCESS) };

    // Check if the session was found
    if session_reply.sessions.is_empty() {
        return Json(SessionReply { sessions : vec![], status: JsonErrorSet::from(SESSION_NOT_FOUND) } )
    }

    // ... then check the end date
    let session = session_reply.sessions.get_mut(0).unwrap();

    if session.termination_time_gmt.is_some() {
        return Json(SessionReply { sessions : vec![], status: JsonErrorSet::from(SESSION_TIMED_OUT) } )
    }

    // Update the session renew_time_gmt

    let r_update = update_renew_time(&mut trans, &session_id);

    if r_update.is_err() {
        trans.rollback();
        return Json(SessionReply { sessions : vec![], status: JsonErrorSet::from(SESSION_CANNOT_BE_RENEWED) } )
    }

    session.renew_time_gmt = Some(Utc::now().to_string());

    // End the transaction
    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    log_info!("🏁 End read_session api, trace_id=[{:?}]", token);

    Json(session_reply)

}


///
///
///
fn search_session_by_sid(mut trans : &mut SQLTransaction, session_id: Option<&str>) -> anyhow::Result<Vec<EntrySession>> {
    let p_sid = CellValue::from_opt_str(session_id);

    let mut params = HashMap::new();
    params.insert("p_sid".to_owned(), p_sid);

    let query = SQLQueryBlock {
        sql_query : r"SELECT id, customer_code, customer_id, user_name, user_id, session_id, start_time_gmt, renew_time_gmt, termination_time_gmt
                    FROM dokasys.sessions
                    WHERE session_id = :p_sid OR :p_sid IS NULL ".to_string(),
        start : 0,
        length : Some(1),
        params,
    };

    let mut sql_result : SQLDataSet =  query.execute(&mut trans).map_err(err_fwd!("Query failed, [{}]", &query.sql_query))?;

    let mut sessions = vec![];
    while sql_result.next() {
        let id : i64 = sql_result.get_int("id").unwrap_or(0i64);
        let customer_code: String = sql_result.get_string("customer_code").unwrap_or("".to_owned());
        let customer_id: i64 = sql_result.get_int("customer_id").unwrap_or(0i64);
        let user_name: String = sql_result.get_string("user_name").unwrap_or("".to_owned());
        let user_id: i64 = sql_result.get_int("user_id").unwrap_or(0i64);
        let session_id: String = sql_result.get_string("session_id").unwrap_or("".to_owned());
        let start_time_gmt  = sql_result.get_timestamp_as_datetime("start_time_gmt")
            .ok_or(anyhow::anyhow!(""))
            .map_err(err_fwd!("Cannot read the start time"))?;

        let renew_time_gmt = sql_result.get_timestamp_as_datetime("renew_time_gmt").as_ref().map( |x| x.to_string() );
        let termination_time_gmt = sql_result.get_timestamp_as_datetime("termination_time_gmt").as_ref().map( |x| x.to_string() );

        let session_info = EntrySession {
            id,
            customer_code,
            user_name,
            customer_id,
            user_id,
            session_id,
            start_time_gmt : start_time_gmt.to_string(),
            renew_time_gmt,
            termination_time_gmt,
        };

        let _ = &sessions.push(session_info);

    }

    Ok(sessions)
}


///
///
///
fn update_renew_time(mut trans : &mut SQLTransaction, session_id: &str) -> anyhow::Result<bool> {
    let p_sid = CellValue::from_raw_string(session_id.to_owned());

    let mut params = HashMap::new();
    params.insert("p_session_id".to_owned(), p_sid);

    let sql_update = r#"UPDATE dokasys.SESSIONS
                             SET renew_time_gmt = ( NOW() at time zone 'UTC'  )
                             WHERE session_id = :p_session_id "#;

    let query = SQLChange {
        sql_query :  sql_update.to_string(),
        params,
        sequence_name : "".to_string(),
    };

    let _ = query.update(&mut trans).map_err( err_fwd!("Cannot update the session"))?;

    Ok(true)
}

///
/// Open a new session for the group and user
///
#[post("/session", format = "application/json", data = "<session>")]
fn open_session(session: Json<OpenSessionRequest>, security_token: SecurityToken) -> Json<OpenSessionReply> {

    dbg!(&session);

    // Check if the token is valid
    if !security_token.is_valid() {
        return Json(OpenSessionReply {
            session_id: "".to_string(),
            status : JsonErrorSet::from(INVALID_TOKEN),
        });
    }
    let token = security_token.take_value();

    log_info!("🚀 Start open_session api, token_id={}", &token);

    let internal_database_error_reply = Json(OpenSessionReply{ session_id: "".to_string(), status : JsonErrorSet::from(INTERNAL_DATABASE_ERROR) });

    let mut cnx = match SQLConnection::new().map_err(err_fwd!("Connection issue"))
    {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    let mut trans = match cnx.sql_transaction().map_err(err_fwd!("Transaction issue"))
    {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    let new_customer_key = DkEncrypt::generate_random_key();
    dbg!(&new_customer_key);

    let sql_insert = r#"INSERT INTO dokasys.SESSIONS
                            (customer_code, customer_id, user_name, user_id, session_id, start_time_gmt)
                            VALUES (:p_customer_code, :p_customer_id, :p_user_name, :p_user_id, :p_session_id, :p_start_time_gmt)"#;

    let current_datetime = SystemTime::now();
    let session_id = session.session_id.to_owned();

    let mut params : HashMap<String, CellValue> = HashMap::new();
    params.insert("p_customer_code".to_owned(), CellValue::from_raw_string(session.customer_code.to_owned()));
    params.insert("p_customer_id".to_owned(), CellValue::from_raw_int(session.customer_id));
    params.insert("p_user_name".to_owned(), CellValue::from_raw_string(session.user_name.to_owned()));
    params.insert("p_user_id".to_owned(), CellValue::from_raw_int(session.user_id));
    params.insert("p_session_id".to_owned(), CellValue::from_raw_string(session_id.clone()));
    params.insert("p_start_time_gmt".to_owned(), CellValue::from_raw_systemtime(current_datetime));

    let query = SQLChange {
        sql_query :  sql_insert.to_string(),
        params,
        sequence_name : "dokasys.sessions_id_seq".to_string(),
    };

    let _ = match query.insert(&mut trans).map_err( err_fwd!("Cannot insert the session")) {
        Ok(v) => { v },
        Err(_) => { return internal_database_error_reply; },
    };

    if trans.commit().map_err(err_fwd!("Commit failed")).is_err() {
        return internal_database_error_reply;
    }

    log_info!("😎 Customer key added with success");

    let ret = OpenSessionReply {
        session_id,
        status : JsonErrorSet::from(SUCCESS),
    };
    log_info!("🏁 End open_session, token_id = {}", &token);
    Json(ret)
}

///
///
///
fn main() {

    const PROGRAM_NAME: &str = "Session Manager";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "session-manager";
    const VAR_NAME: &str = "DOKA_ENV";

    // Read the application config's file
    println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);

    let props = read_config(PROJECT_CODE, VAR_NAME);

    dbg!(&props);
    set_prop_values(props);

    let port = get_prop_value("server.port").parse::<u16>().unwrap();
    dbg!(port);

    let log_config: String = get_prop_value("log4rs.config");

    let log_config_path = Path::new(&log_config);

    // Read the global properties
    println!("😎 Read log properties from {:?}", &log_config_path);

    match log4rs::init_file(&log_config_path, Default::default()) {
        Err(e) => {
            eprintln!("{:?} {:?}", &log_config_path, e);
            exit(-59);
        }
        Ok(_) => {}
    }

    // Read the CEK
    log_info!("😎 Read Common Edible Key");
    read_cek_and_store();

    let new_prop = get_prop_value("cek");
    dbg!(&new_prop);

    // Init DB pool
    let (connect_string, db_pool_size) = match get_prop_pg_connect_string()
                    .map_err(err_fwd!("Cannot read the database connection information")) {
        Ok(x) => x,
        Err(e) => {
            log_error!("{:?}", e);
            exit(-64);
        }
    };

    init_db_pool(&connect_string, db_pool_size);

    log_info!("🚀 Start {}", PROGRAM_NAME);

    let mut my_config = Config::new(Environment::Production);
    my_config.set_port(port);

    let base_url = format!("/{}", PROJECT_CODE);

    let _ = rocket::custom(my_config)
        .mount(&base_url, routes![open_session, read_session])
        .attach(Template::fairing())
        .launch();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
