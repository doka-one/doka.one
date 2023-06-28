#![feature(proc_macro_hygiene, decl_macro)]
#![feature(let_else)]


mod file_delegate;

use std::collections::HashMap;
use std::path::Path;
use std::process::exit;
use rocket::config::Environment;
use rocket_contrib::templates::Template;
use rocket::{Config, Data, Request, Response, routes};
use commons_pg::{init_db_pool, };
use commons_services::read_cek_and_store;
use dkconfig::conf_reader::{read_config, read_doka_env};
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use log::*;
use commons_error::*;
use rocket::{post,get};
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{ContentType, Method, RawStr, Status};
use rocket::response::{Content};
use rocket_contrib::json::Json;

use commons_services::property_name::{ LOG_CONFIG_FILE_PROPERTY, SERVER_PORT_PROPERTY,};
use commons_services::token_lib::SessionToken;
use commons_services::x_request_id::XRequestID;
use dkdto::{GetFileInfoReply, GetFileInfoShortReply, UploadReply};

use crate::file_delegate::FileDelegate;


///
/// ✨  Upload the binary content of a file
///
#[post("/upload", data = "<file_data>")]
pub fn upload(file_data: Data, session_token : SessionToken) -> Json<UploadReply> {
    let mut delegate = FileDelegate::new(session_token, XRequestID::from_value(None));
    delegate.upload(file_data)
}

///
/// ✨  Upload the binary content of a file v2
/// item_info : Base64Url encoded information representing a value from the possible target item (for instance, its filename)
///
#[post("/upload2/<item_info>", data = "<file_data>")]
pub fn upload2(item_info: &RawStr, file_data: Data, session_token : SessionToken) -> Json<UploadReply> {
    let mut delegate = FileDelegate::new(session_token, XRequestID::from_value(None));
    delegate.upload2(item_info, file_data)
}


///
/// ✨ Get the information about the composition of a file [file_ref]
///
#[get("/info/<file_ref>")]
pub fn file_info(file_ref: &RawStr, session_token : SessionToken) -> Json<GetFileInfoReply> {
    let mut delegate = FileDelegate::new(session_token, XRequestID::from_value(None));
    delegate.file_info(file_ref)

}

///
/// ✨ Get the information about the loading status of a file [file_ref]
///
#[get("/stats/<file_ref>")]
pub fn file_stats(file_ref: &RawStr, session_token : SessionToken) -> Json<GetFileInfoShortReply> {
    let mut delegate = FileDelegate::new(session_token, XRequestID::from_value(None));
    delegate.file_stats(file_ref)
}

///
/// ✨  Download the binary content of a file
///
#[get("/download/<file_ref>")]
pub fn download(file_ref: &RawStr, session_token : SessionToken) -> Content<Vec<u8>> {
    let mut delegate = FileDelegate::new(session_token, XRequestID::from_value(None));
    delegate.download(file_ref)
}

#[derive(Debug)]
pub struct CORS;

impl Fairing for CORS {
    fn info(&self) -> Info {
        Info {
            name: "Add CORS headers to responses",
            kind: Kind::Response
        }
    }

    fn on_response(&self, request: &Request, response: &mut Response) {
        info!("On Response [{}]", &request );
        info!("On Response [{}]", &response.status() );

        let _ = response.status();
        // dbg!(&s);

        if request.method() == Method::Options {
            response.set_status(Status::Ok);
        }

        response.adjoin_header(ContentType::JSON );
        response.adjoin_raw_header("Access-Control-Allow-Methods", "POST, GET, OPTIONS, PATCH, DELETE");
        response.adjoin_raw_header("Access-Control-Allow-Origin", "*");
        response.adjoin_raw_header("Access-Control-Allow-Credentials", "true");
        response.adjoin_raw_header("Access-Control-Allow-Headers", "*");
    }
}

fn main() {

    const PROGRAM_NAME: &str = "File Server";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "file-server";
    const VAR_NAME: &str = "DOKA_ENV";

    // Read the application config's file
    println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);

    let props = read_config(PROJECT_CODE, &read_doka_env(&VAR_NAME));
    set_prop_values(props);

    log_info!("🚀 Start {}", PROGRAM_NAME);

    let Ok(port) = get_prop_value(SERVER_PORT_PROPERTY).unwrap_or("".to_string()).parse::<u16>() else {
        eprintln!("💣 Cannot read the server port");
        exit(-56);
    };

    let Ok(log_config) = get_prop_value(LOG_CONFIG_FILE_PROPERTY) else {
        eprintln!("💣 Cannot read the log4rs config");
        exit(-57);
    };
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

    // let new_prop = get_prop_value(CUSTOMER_EDIBLE_KEY_PROPERTY);
    // dbg!(&new_prop);

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

    let mut my_config = Config::new(Environment::Production);
    my_config.set_port(port);

    let base_url = format!("/{}", PROJECT_CODE);



    let _ = rocket::custom(my_config)
        .mount(&base_url, routes![
            upload,
            upload2,
            file_info,
            file_stats,
            download,
        ])
        .attach(CORS)
        .attach(Template::fairing())
        .launch();

    log_info!("🏁 End {}", PROGRAM_NAME);
}

#[cfg(test)]
mod test {
    // use std::path::Path;
    // use std::process::exit;
    // use commons_pg::{init_db_pool, SQLConnection, SQLTransaction};
    // use commons_services::database_lib::open_transaction;
    // use dkconfig::conf_reader::read_config;
    // use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
    // //use crate::{insert_document_part, parse_content, select_tsvector};
    // use log::{error,info};
    // use commons_error::*;
    // use commons_services::property_name::LOG_CONFIG_FILE_PROPERTY;
    //
    // fn init_test() {
    //     const PROGRAM_NAME: &str = "Test File Server";
    //
    //     println!("😎 Init {}", PROGRAM_NAME);
    //
    //     const PROJECT_CODE: &str = "file-server";
    //     const VAR_NAME: &str = "DOKA_ENV";
    //
    //     // Read the application config's file
    //     println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);
    //
    //     let props = read_config(PROJECT_CODE, VAR_NAME);
    //     set_prop_values(props);
    //
    //     log_info!("🚀 Start {}", PROGRAM_NAME);
    //
    //     let log_config: String = get_prop_value(LOG_CONFIG_FILE_PROPERTY).unwrap();
    //     let log_config_path = Path::new(&log_config);
    //
    //     // Read the global properties
    //     println!("😎 Read log properties from {:?}", &log_config_path);
    //
    //     match log4rs::init_file(&log_config_path, Default::default()) {
    //         Err(e) => {
    //             eprintln!("{:?} {:?}", &log_config_path, e);
    //             exit(-59);
    //         }
    //         Ok(_) => {}
    //     }
    //
    //
    //     // Init DB pool
    //     let (connect_string, db_pool_size) = match get_prop_pg_connect_string()
    //         .map_err(err_fwd!("Cannot read the database connection information")) {
    //         Ok(x) => x,
    //         Err(e) => {
    //             log_error!("{:?}", e);
    //             exit(-64);
    //         }
    //     };
    //
    //     init_db_pool(&connect_string, db_pool_size);
    // }
    //
    // #[test]
    // fn test_parse_content() -> anyhow::Result<()> {
    //     init_test();
    //     let mem_file: Vec<u8> = std::fs::read("C:/Users/denis/wks-poc/tika/big_planet.pdf")?;
    //     let ret = parse_content("0f373b54-5dbb-4c75-98e7-98fd141593dc", mem_file, "f1248fab", "MY_SID")?;
    //     Ok(())
    // }
    //
    //
    // #[test]
    // fn test_compute_tsvector() -> anyhow::Result<()> {
    //     init_test();
    //     let mut r_cnx = SQLConnection::new();
    //     let mut trans = open_transaction(&mut r_cnx)?;
    //     let ret = select_tsvector(&mut trans,Some("french"), "Planète Phase formation cœurs planétaires Phase formation noyaux telluriques moderne")?;
    //     assert_eq!("'coeur':4 'format':3,7 'modern':10 'noyal':8 'phas':2,6 'planet':1 'planetair':5 'tellur':9", ret);
    //     Ok(())
    // }
    //
    // #[test]
    // fn test_insert_document() -> anyhow::Result<()> {
    //     init_test();
    //     let mut r_cnx = SQLConnection::new();
    //     let mut trans = open_transaction(&mut r_cnx)?;
    //
    //     let id = insert_document_part(&mut trans, "0f373b54-5dbb-4c75-98e7-98fd141593dc", 107,
    //                                  "Phase formation cœurs planétaires Phase formation noyaux telluriques moderne",
    //                                  "french", "f1248fab" )?;
    //
    //     trans.commit();
    //     log_info!("ID = [{}]", id);
    //     assert!(id > 0);
    //     Ok(())
    // }

}
