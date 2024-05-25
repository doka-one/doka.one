#![feature(proc_macro_hygiene, decl_macro)]
#![feature(try_trait_v2)]

use std::path::Path;
use std::process::exit;

use log::{error, info};
use rocket::{Config, routes};
use rocket::{delete, get, post};
use rocket::config::Environment;
use rocket_contrib::json::Json;
use rocket_contrib::templates::Template;

use commons_error::*;
use commons_pg::init_db_pool;
use commons_services::property_name::{COMMON_EDIBLE_KEY_PROPERTY, LOG_CONFIG_FILE_PROPERTY, SERVER_PORT_PROPERTY};
use commons_services::read_cek_and_store;
use commons_services::token_lib::SessionToken;
use commons_services::x_request_id::XRequestID;
use dkconfig::conf_reader::{read_config, read_doka_env};
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use dkdto::{ AddItemReply, AddItemRequest, AddItemTagReply, AddItemTagRequest, AddTagReply, AddTagRequest, DeleteFullTextRequest, DeleteTagsRequest, FullTextReply, FullTextRequest, GetItemReply, GetTagReply, QueryFilters, SimpleMessage, WebType};

use crate::fulltext::FullTextDelegate;
use crate::item::ItemDelegate;
use crate::tag::TagDelegate;

mod item;
mod tag;
mod fulltext;
mod ft_tokenizer;
mod language;
mod filter_ast;
mod char_lib;
mod filter_lexer;
mod filter_normalizer;


///  deprecated
/// ✨ Find all the items at page [start_page]
/// **NORM
///
#[get("/item?<start_page>&<page_size>")]
pub fn get_all_item(start_page : Option<u32>, page_size : Option<u32>, session_token: SessionToken) -> WebType<GetItemReply> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.get_all_item(start_page, page_size)
}

///
/// ✨ Find all the items at page [start_page]
/// **NORM
///
#[get("/search?<start_page>&<page_size>&<filters>")]
pub fn search_item(start_page : Option<u32>, page_size : Option<u32>, filters: QueryFilters, session_token: SessionToken) -> WebType<GetItemReply> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));

    delegate.search_item(start_page, page_size, filters)

    //dbg!(&filters);s
    // let lexems = filter_lexem_parser::lex(&filters.0);
    // let filter_tokens : Box<FilterExpression> = parse_expression(&lexems).unwrap();
    //
    // // Verify the the attributes are existing tag in doka and the type complies with the filter condition
    //
    //
    //
    //
    // let s = to_sql_form(&filter_tokens.deref()).unwrap();
    //
    // println!("sql = {:}", &s);
    // WebType::from_errorset(INTERNAL_DATABASE_ERROR)
}

///
/// ✨  Find a item from its item id
/// **NORM
///
#[get("/item/<item_id>")]
pub (crate) fn get_item(item_id: i64, session_token: SessionToken) -> WebType<GetItemReply> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.get_item(item_id)
}

///
/// ✨ Create an item and all its tags
///     A tag can be existing or not
/// **NORM
///
#[post("/item", format = "application/json", data = "<add_item_request>")]
pub (crate) fn add_item(add_item_request: Json<AddItemRequest>, session_token: SessionToken) -> WebType<AddItemReply> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.add_item(add_item_request)
}

///
/// ✨ Update tags on an existing item
///     Tags can be already existing in the system.
///
///
#[post("/item/<item_id>/tags", format = "application/json", data = "<add_item_tag_request>")]
pub (crate) fn update_item_tag(item_id: i64,add_item_tag_request: Json<AddItemTagRequest>, session_token: SessionToken) -> WebType<AddItemTagReply> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.update_item_tag(item_id,add_item_tag_request)
}


///
/// ✨ Update tags on an existing item
///     Tags can be already existing in the system.
///
///  DELETE /api/documents/{item_id}/tags?tag_names=tag1,tag2,tag3
///
#[delete("/item/<item_id>/tags?<tag_names>")]
pub (crate) fn delete_item_tag(item_id: i64, tag_names: DeleteTagsRequest, session_token: SessionToken) -> WebType<SimpleMessage> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.delete_item_tag(item_id, tag_names)
}

type Type = GetTagReply;

///
/// ✨ Find all the existing tags by pages
/// **NORM
///
#[get("/tag?<start_page>&<page_size>")]
pub (crate) fn get_all_tag(start_page : Option<u32>, page_size : Option<u32>, session_token: SessionToken) -> WebType<Type> {
    let delegate = TagDelegate::new(session_token, XRequestID::from_value(None));
    delegate.get_all_tag(start_page, page_size)
}


///
/// ✨ Delete a tag
/// **NORM
///
#[delete("/tag/<tag_id>")]
pub (crate) fn delete_tag(tag_id: i64, session_token: SessionToken) -> WebType<SimpleMessage> {
    let delegate = TagDelegate::new(session_token, XRequestID::from_value(None));
    delegate.delete_tag(tag_id)
}

///
/// ✨ Create a new tag
/// **NORM
///
#[post("/tag", format = "application/json", data = "<add_tag_request>")]
pub (crate) fn add_tag(add_tag_request: Json<AddTagRequest>, session_token: SessionToken) -> WebType<AddTagReply> {
    let delegate = TagDelegate::new(session_token, XRequestID::from_value(None));
    delegate.add_tag(add_tag_request)
}

///
/// ✨ Parse the raw text data and create the document parts
/// Used from file-server
/// **NORM
///
#[post("/fulltext_indexing", format = "application/json", data = "<raw_text_request>")]
pub (crate) fn fulltext_indexing(raw_text_request: Json<FullTextRequest>, session_token: SessionToken, x_request_id: XRequestID) -> WebType<FullTextReply> {
    let delegate = FullTextDelegate::new(session_token, x_request_id);
    delegate.fulltext_indexing(raw_text_request)
}

///
/// ✨ Delete the information linked to the document full text indexing information
/// Used from file-server
/// **NORM
///
#[post("/delete_text_indexing", format = "application/json", data = "<delete_text_request>")]
pub (crate) fn delete_text_indexing(delete_text_request: Json<DeleteFullTextRequest>, session_token: SessionToken, x_request_id: XRequestID) -> WebType<SimpleMessage> {
    let delegate = FullTextDelegate::new(session_token, x_request_id);
    delegate.delete_text_indexing(delete_text_request)
}

fn main() {

    const PROGRAM_NAME: &str = "Document Server";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "document-server";
    const VAR_NAME: &str = "DOKA_ENV";

    // Read the application config's file
    println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);

    let props = read_config(PROJECT_CODE, &read_doka_env(&VAR_NAME));

    set_prop_values(props);

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

    log_info!("🚀 Start {}", PROGRAM_NAME);

    // Read the CEK
    log_info!("😎 Read Common Edible Key");
    read_cek_and_store();

    let Ok(cek) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY) else {
        panic!("💣 Cannot read the cek properties");
    };
    log_info!("😎 The CEK was correctly read : [{}]", format!("{}...", &cek[0..5]));

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
            search_item,
            get_item,
            add_item,
            update_item_tag,
            delete_item_tag,
            get_all_tag,
            add_tag,
            delete_tag,
            fulltext_indexing,
            delete_text_indexing,
        ])
        .attach(Template::fairing())
        .launch();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
