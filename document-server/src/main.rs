#![feature(proc_macro_hygiene, decl_macro)]

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
use commons_error::*;

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

    log_info!("🚀 Start {}", PROGRAM_NAME);

    // Read the CEK
    log_info!("😎 Read Common Edible Key");
    read_cek_and_store();

    let cek = get_prop_value("cek");
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
            item::get_all_item,
            item::get_item,
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
