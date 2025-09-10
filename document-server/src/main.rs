use std::net::SocketAddr;
use std::process::exit;

use axum::extract::{Path, Query};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use log::{error, info};
use serde_derive::{Deserialize, Serialize};

use commons_error::*;
use commons_pg::sql_transaction_async::init_db_pool_async;
use commons_services::read_cek_and_store;
use commons_services::token_lib::SessionToken;
use commons_services::x_request_id::XRequestID;
use dkconfig::conf_reader::{read_config, read_doka_env};
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use dkconfig::property_name::{COMMON_EDIBLE_KEY_PROPERTY, LOG_CONFIG_FILE_PROPERTY, SERVER_PORT_PROPERTY};
use dkdto::web_types::{
    AddItemReply, AddItemRequest, AddItemTagReply, AddItemTagRequest, AddTagReply, AddTagRequest,
    DeleteFullTextRequest, FullTextReply, FullTextRequest, GetItemReply, GetTagReply, SimpleMessage, WebType,
    WebTypeBuilder, WebTypeWithContext,
};

use crate::fulltext::FullTextDelegate;
use crate::item::ItemDelegate;
use crate::tag::TagDelegate;

mod char_lib;
mod engine;
mod filter;
mod ft_tokenizer;
mod fulltext;
mod item;
mod language;
mod tag;

#[derive(Serialize, Deserialize)]
pub struct PageQuery {
    pub start_page: Option<u32>,
    pub page_size: Option<u32>,
}

///  deprecated
/// 🌟 Find all the items at page [start_page]
/// **NORM
///
///#[get("/item?<start_page>&<page_size>")]
pub async fn get_all_item(Query(page): Query<PageQuery>, session_token: SessionToken) -> WebType<GetItemReply> {
    //
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.get_all_item(page.start_page, page.page_size).await
}

#[derive(Serialize, Deserialize)]
pub struct SearchQuery {
    pub start_page: Option<u32>,
    pub page_size: Option<u32>,
    pub filters: Option<String>,
}

///
/// 🌟 Find all the items at page [start_page]
/// **NORM
///
/// #[get("/search?<start_page>&<page_size>&<filters>")]
pub async fn search_item(
    Query(page): Query<SearchQuery>,
    session_token: SessionToken,
) -> WebTypeWithContext<GetItemReply> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));

    delegate.search_item(page.start_page, page.page_size, page.filters).await
}

///
/// 🌟  Find a item from its item id
/// **NORM
///
/// #[get("/item/<item_id>")]
pub(crate) async fn get_item(Path(item_id): Path<i64>, session_token: SessionToken) -> WebType<GetItemReply> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.get_item(item_id).await
}

///
/// 🌟 Create an item and all its tags
///     A tag can be existing or not
/// **NORM
///
/// #[post("/item", format = "application/json", data = "<add_item_request>")]
pub(crate) async fn add_item(
    session_token: SessionToken,
    add_item_request: Json<AddItemRequest>,
) -> WebType<AddItemReply> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.add_item(add_item_request).await
}

///
/// 🌟 Update tags on an existing item
///     Tags can be already existing in the system.
///
/// ```
/// #[post(
///     "/item/<item_id>/tags",
///     format = "application/json",
///     data = "<add_item_tag_request>"
/// )]
/// ```
pub(crate) async fn update_item_tag(
    session_token: SessionToken,
    Path(item_id): Path<i64>,
    add_item_tag_request: Json<AddItemTagRequest>,
) -> WebType<AddItemTagReply> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.update_item_tag(item_id, add_item_tag_request).await
}

#[derive(Serialize, Deserialize)]
pub struct DeleteTagsQuery {
    pub names: Vec<String>,
}

///
/// 🌟 Update tags on an existing item
///     Tags can be already existing in the system.
///
///  DELETE /api/documents/{item_id}/tags?tag_names=tag1,tag2,tag3
///
/// #[delete("/item/<item_id>/tags?<tag_names>")]
pub(crate) async fn delete_item_tag(
    session_token: SessionToken,
    Path(item_id): Path<i64>,
    Query(tag_names): Query<DeleteTagsQuery>,
) -> WebType<SimpleMessage> {
    let delegate = ItemDelegate::new(session_token, XRequestID::from_value(None));
    delegate.delete_item_tag(item_id, tag_names.names).await
}

type Type = GetTagReply;

///
/// 🌟 Find all the existing tags by pages
/// **NORM
///
/// #[get("/tag?<start_page>&<page_size>")]
pub(crate) async fn get_all_tag(Query(page): Query<PageQuery>, session_token: SessionToken) -> WebType<Type> {
    let delegate = TagDelegate::new(session_token, XRequestID::from_value(None));
    delegate.get_all_tag(page.start_page, page.page_size).await
}

///
/// 🌟 Delete a tag
/// **NORM
///
/// #[delete("/tag/<tag_id>")]
pub(crate) async fn delete_tag(session_token: SessionToken, Path(tag_id): Path<i64>) -> WebType<SimpleMessage> {
    let delegate = TagDelegate::new(session_token, XRequestID::from_value(None));
    delegate.delete_tag(tag_id).await
}

///
/// 🌟 Create a new tag
/// **NORM
///
/// #[post("/tag", format = "application/json", data = "<add_tag_request>")]
pub(crate) async fn add_tag(session_token: SessionToken, add_tag_request: Json<AddTagRequest>) -> WebType<AddTagReply> {
    let delegate = TagDelegate::new(session_token, XRequestID::from_value(None));
    delegate.add_tag(add_tag_request).await
}

///
/// 🌟 Parse the raw text data and create the document parts
/// Used from file-server
/// **NORM
///
/// ```
/// #[post(
///    "/fulltext_indexing",
///    format = "application/json",
///    data = "<raw_text_request>"
/// )]
/// ```
pub(crate) async fn fulltext_indexing(
    session_token: SessionToken,
    x_request_id: XRequestID,
    raw_text_request: Json<FullTextRequest>,
) -> WebType<FullTextReply> {
    log_info!(">>> Hey!!!");
    let delegate = FullTextDelegate::new(session_token, x_request_id);
    delegate.fulltext_indexing(raw_text_request).await
}

/// 🌟 Delete the information linked to the document full text indexing information
/// Used from file-server
/// **NORM
///
/// ```
/// #[post(
///    "/delete_text_indexing",
///    format = "application/json",
///    data = "<delete_text_request>"
/// )]
pub(crate) async fn delete_text_indexing(
    session_token: SessionToken,
    x_request_id: XRequestID,
    delete_text_request: Json<DeleteFullTextRequest>,
) -> WebType<SimpleMessage> {
    let delegate = FullTextDelegate::new(session_token, x_request_id);
    delegate.delete_text_indexing(delete_text_request).await
}

#[tokio::main]
async fn main() {
    const PROGRAM_NAME: &str = "Document Server";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "document-server";
    const VAR_NAME: &str = "DOKA_ENV";

    // Read the application config's file
    println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);

    let props = read_config(PROJECT_CODE, &read_doka_env(&VAR_NAME), &Some("DOKA_CLUSTER_PROFILE".to_string()));

    set_prop_values(props);

    let Ok(port) = get_prop_value(SERVER_PORT_PROPERTY).unwrap_or("".to_string()).parse::<u16>() else {
        eprintln!("💣 Cannot read the server port");
        exit(-56);
    };

    let Ok(log_config) = get_prop_value(LOG_CONFIG_FILE_PROPERTY) else {
        eprintln!("💣 Cannot read the log4rs config");
        exit(-57);
    };

    let log_config_path = std::path::Path::new(&log_config);

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

    let Ok(cek) = get_prop_value(COMMON_EDIBLE_KEY_PROPERTY) else {
        panic!("💣 Cannot read the cek properties");
    };
    log_info!("😎 The CEK was correctly read : [{}]", format!("{}...", &cek[0..5]));

    // Init DB pool
    let (connect_string, db_pool_size) =
        match get_prop_pg_connect_string().map_err(err_fwd!("Cannot read the database connection information")) {
            Ok(x) => x,
            Err(e) => {
                log_error!("{:?}", e);
                exit(-64);
            }
        };

    let _ = init_db_pool_async(&connect_string, db_pool_size).await;

    log_info!("🚀 Start {} on port {}", PROGRAM_NAME, port);

    // Build our application with some routes
    let base_url = format!("/{}", PROJECT_CODE);

    let key_routes = Router::new()
        .route("/item", get(get_all_item))
        .route("/search", get(search_item))
        .route("/item/:item_id", get(get_item))
        .route("/item", post(add_item))
        .route("/item/:item_id/tags", post(update_item_tag))
        .route("/item/:item_id/tags", delete(delete_item_tag))
        .route("/tag", get(get_all_tag))
        .route("/tag", post(add_tag))
        .route("/tag/:tag_id", delete(delete_tag))
        .route("/fulltext_indexing", post(fulltext_indexing))
        .route("/delete_text_indexing", post(delete_text_indexing));

    let app = Router::new().nest(&base_url, key_routes);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();

    log_info!("🏁 End {}", PROGRAM_NAME);
}
