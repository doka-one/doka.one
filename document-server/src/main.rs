#![feature(proc_macro_hygiene, decl_macro)]
#![feature(let_else)]

mod item;
mod tag;
mod fulltext;
mod ft_tokenizer;
mod language;
mod item_query;

use std::path::Path;
use std::process::exit;
use rocket::config::Environment;
use rocket_contrib::templates::Template;
use rocket::{Config, routes};
use commons_pg::init_db_pool;
use commons_services::read_cek_and_store;
use dkconfig::conf_reader::read_config;
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use log::{error,info};
use rocket_contrib::json::Json;
use rocket::{get};
use commons_error::*;
use commons_services::property_name::{COMMON_EDIBLE_KEY_PROPERTY, LOG_CONFIG_FILE_PROPERTY, SERVER_PORT_PROPERTY};
use commons_services::token_lib::SessionToken;
use commons_services::x_request_id::XRequestID;
use dkdto::GetItemReply;
use crate::item::ItemDelegate;

///
/// ✨ Find all the items at page [start_page]
///
#[get("/item?<start_page>&<page_size>")]
pub fn get_all_item(start_page : Option<u32>, page_size : Option<u32>, session_token: SessionToken) -> Json<GetItemReply> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.get_all_item(start_page, page_size)
}


///
/// ✨  Find a item from its item id
///
#[get("/item/<item_id>")]
pub (crate) fn get_item(item_id: i64, session_token: SessionToken) -> Json<GetItemReply> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.get_item(item_id)
}

fn main() {

    const PROGRAM_NAME: &str = "Document Server";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "document-server";
    const VAR_NAME: &str = "DOKA_ENV";

    // Read the application config's file
    println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);

    let props = read_config(PROJECT_CODE, VAR_NAME);

    dbg!(&props);
    set_prop_values(props);

    let Ok(port) = get_prop_value(SERVER_PORT_PROPERTY).unwrap_or("".to_string()).parse::<u16>() else {
        eprintln!("💣 Cannot read the server port");
        exit(-56);
    };

    dbg!(port);

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

    log_info!("🚀 Start {}", PROGRAM_NAME);

    // Read the CEK
    log_info!("😎 Read Common Edible Key");
    read_cek_and_store();

    let cek = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY);
    dbg!(&cek);

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
            get_all_item,
            get_item,
            item::add_item,
            tag::get_all_tag,
            tag::add_tag,
            tag::delete_tag,
            fulltext::fulltext_indexing,
        ])
        .attach(Template::fairing())
        .launch();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
