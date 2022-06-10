#![feature(proc_macro_hygiene, decl_macro)]
#![feature(let_else)]

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read};
use std::path::Path;
use std::process::exit;
use std::{io, thread};
use std::thread::{JoinHandle};

use rocket::config::Environment;
use rocket_contrib::templates::Template;
use rocket::{Config, Data, routes};
use commons_pg::{CellValue, init_db_pool, SQLChange, SQLConnection, SQLDataSet, SQLQueryBlock, };
use commons_services::read_cek_and_store;
use dkconfig::conf_reader::read_config;
use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
use log::{error,info, warn};
use commons_error::*;

use rocket::{post,get};

use rocket::http::{ContentType, RawStr};
use rocket::response::{Content};
use rocket_contrib::json::Json;
use rs_uuid::iso::uuid_v4;
use rustc_serialize::base64::{ToBase64, URL_SAFE};
use commons_services::database_lib::open_transaction;
use commons_services::key_lib::fetch_customer_key;
use commons_services::property_name::{DOCUMENT_SERVER_HOSTNAME_PROPERTY, DOCUMENT_SERVER_PORT_PROPERTY, LOG_CONFIG_FILE_PROPERTY, SERVER_PORT_PROPERTY, TIKA_SERVER_HOSTNAME_PROPERTY, TIKA_SERVER_PORT_PROPERTY};
use commons_services::session_lib::fetch_entry_session;
use commons_services::token_lib::SessionToken;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::{BlockStatus,GetFileInfoReply, GetFileInfoShortReply, JsonErrorSet, UploadReply};
use dkdto::error_codes::{INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR, SUCCESS};
use dkdto::error_replies::ErrorReply;
use doka_cli::request_client::{DocumentServerClient, TikaServerClient};
// use crate::language::{lang_name_from_code_2, map_code};

const BLOCK_SIZE : usize = 1_048_576;
// const BLOCK_SIZE : usize = 2_000;

///
/// Run a thread to process the block and store it in the DB
///
fn parallel_crypto_and_store_block(file_id : i64, block_set : &HashMap<u32, Vec<u8>>,
                                   customer_code : &str,
                                   customer_key : &str) -> JoinHandle<anyhow::Result<()>> {

    let s_customer_code = customer_code.to_owned();
    let s_customer_key = customer_key.to_owned();
    let local_block_set = block_set.into_iter()
        .map(|(key, value)| { (*key, (*value).to_owned())  })
        .collect();

    let th = thread::spawn( move || {
        crypto_and_store_block( file_id, local_block_set,
                                s_customer_code, s_customer_key)
    });

    th
}

fn parallel_parse_content(file_ref: &str, mem_file : Vec<u8>, customer_code: &str, sid: &str) -> JoinHandle<anyhow::Result<()>> {

    let my_file_ref = file_ref.to_owned();
    let my_sid = sid.to_owned();
    let my_customer_code = customer_code.to_owned();

    let th = thread::spawn( move || {
        parse_content(&my_file_ref, mem_file,  &my_customer_code, &my_sid)
    });

    th
}

fn min_max<T>(map : &HashMap<u32, T> ) ->  (u32,u32) {
    let mut min : u32 = u32::MAX;
    let mut max : u32 = u32::MIN;

    for (index, _) in map {
        if *index >= max {
            max = *index;
        }
        if *index <= min {
            min = *index;
        }
    }

    (min, max)
}

fn crypto_and_store_block(file_id : i64, block_set : HashMap<u32, Vec<u8>>,
                          customer_code: String, customer_key: String) -> anyhow::Result<()> {

    // Open the transaction
    let block_range = min_max(&block_set);

    log_info!("Block range processing : [{:?}]", &block_range);

    let mut r_cnx = SQLConnection::new();
    let mut trans = open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error, block_range=[{:?}]", &block_range))?;

    for (block_num, block) in block_set {

        log_info!("Block processing : [{}], len [{}]", block_num, block.len());

        // Encrypt the block
        let encrypted_block = DkEncrypt::encrypt_vec(&block, &customer_key)
            .map_err(err_fwd!("Cannot encrypt the data block"))?;

        // and store in the DB

        let data = encrypted_block.to_base64(URL_SAFE);

        let sql_query = format!(r"
        INSERT INTO fs_{}.file_parts (file_reference_id, part_number, is_encrypted, part_data)
        VALUES (:p_file_reference_id, :p_part_number, :p_is_encrypted,
                 :p_part_data)", customer_code);

        let sequence_name = format!("fs_{}.file_parts_id_seq", customer_code);

        let mut params = HashMap::new();
        params.insert("p_file_reference_id".to_string(), CellValue::from_raw_int(file_id));
        params.insert("p_part_number".to_string(), CellValue::from_raw_int_32(block_num as i32));
        params.insert("p_is_encrypted".to_string(), CellValue::from_raw_bool(true));
        params.insert("p_part_data".to_string(), CellValue::from_raw_string(data));

        let sql_insert = SQLChange {
            sql_query,
            params,
            sequence_name,
        };

        let _file_part_id = sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion failed"))?;

        log_info!("Block inserted, block_num=[{}]", block_num);
    }

    let _r = trans.commit();

    Ok(())
}

///
/// Call the tika server to parse the file and get the text data
/// Call the document server to fulltext parse the text data
///
fn parse_content(file_ref: &str, mem_file : Vec<u8>, customer_code: &str, sid: &str) -> anyhow::Result<()> {

    log_info!("Parse file content for file_ref=[{}], sid=[{}]", file_ref, sid);

    let tika_server_host = get_prop_value(TIKA_SERVER_HOSTNAME_PROPERTY)?;
    let tika_server_port = get_prop_value(TIKA_SERVER_PORT_PROPERTY)?.parse::<u16>()?;
    let document_server_host = get_prop_value(DOCUMENT_SERVER_HOSTNAME_PROPERTY)?;
    let document_server_port = get_prop_value(DOCUMENT_SERVER_PORT_PROPERTY)?.parse::<u16>()?;

    // set_of_language_word_block = [ <language_word_block>, ...]

    // Get the raw text from the original file
    let tsc = TikaServerClient::new(&tika_server_host, tika_server_port);
    let raw_text = tsc.parse_data(&mem_file).map_err(err_fwd!("Cannot parse the original file"))?;

    log_info!("Parsing done for file_ref=[{}], content size=[{}]", file_ref, raw_text.x_tika_content.len());

    let document_server = DocumentServerClient::new(&document_server_host, document_server_port);
    let reply = document_server.fulltext_indexing(&raw_text.x_tika_content,
                                                  "no_filename_for_now",
                                                  file_ref,
                                                  sid);

    if reply.status.error_code == 0 {
        log_info!("Fulltext indexing done, number of text parts=[{}]", reply.part_count);
        set_file_reference_fulltext_indicator(file_ref, customer_code).map_err(err_fwd!("Cannot set the file reference to fulltext parsed indicator"))?;
    } else {
        log_error!("Error while sending the raw text to the fulltext indexing, file_ref=[{}], sid=[{}], reply=[{:?}]",
            file_ref, sid, reply.status);
        return Err(anyhow::anyhow!(reply.status.err_message));
    }

    log_info!("End of parse file content processing : len [{}]", mem_file.len());
    Ok(())

}


fn set_file_reference_fulltext_indicator(file_ref: &str, customer_code: &str) -> anyhow::Result<()> {

    let mut r_cnx = SQLConnection::new();
    let mut trans = open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error"))?;

    let sql_query = format!(r"UPDATE fs_{}.file_reference
    SET is_fulltext_parsed = true
    WHERE file_ref = :p_file_ref ", customer_code);

    let sequence_name = format!( "fs_{}.file_reference_id_seq", customer_code );

    let mut params = HashMap::new();
    params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.to_string()));

    let sql_update = SQLChange {
        sql_query,
        params,
        sequence_name,
    };

    let _ = sql_update.update(&mut trans).map_err(err_fwd!("Update failed"))?;

    let _r = trans.commit();

    Ok(())
}


// Windows only
fn empty_datastream(reader : &mut dyn Read) {
    // TODO test it on linux!
    // BUG https://github.com/SergioBenitez/Rocket/issues/892
    log_warn!("Running on Windows, need to read the datastream");
    let _r = io::copy( reader, &mut io::sink());
}

///
///
///
fn create_file_reference(r_cnx : &mut anyhow::Result<SQLConnection>, customer_code: &str) -> anyhow::Result<(i64, String)> {

    let mut trans = open_transaction(r_cnx).map_err(err_fwd!("Open transaction error"))?;
    let file_ref = uuid_v4();

    let sql_query = format!(r"INSERT INTO fs_{}.file_reference
    ( file_ref, mime_type,  checksum, original_file_size,  encrypted_file_size,  total_part )
    VALUES ( :p_file_ref, :p_mime_type, :p_checksum, :p_original_file_size, :p_encrypted_file_size, :p_total_part)", customer_code);

    let sequence_name = format!( "fs_{}.file_reference_id_seq", customer_code );

    let mut params = HashMap::new();
    params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.clone()));
    params.insert("p_mime_type".to_string(), CellValue::from_raw_string(String::from("text")));
    params.insert("p_checksum".to_string(), CellValue::String(None));
    params.insert("p_original_file_size".to_string(), CellValue::Int(None));
    params.insert("p_encrypted_file_size".to_string(), CellValue::Int(None));
    params.insert("p_total_part".to_string(), CellValue::Int32(None));

    let sql_insert = SQLChange {
        sql_query,
        params,
        sequence_name,
    };

    let file_id = sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion failed"))?;

    let _r = trans.commit();

    Ok((file_id, file_ref))
}

///
///
///
fn update_file_reference(r_cnx : &mut anyhow::Result<SQLConnection>,
                         file_id : i64,
                         total_size: usize,
                         total_part: u32,
                         customer_code: &str) -> anyhow::Result<()> {

    // TODO check where the file_ref is actually created and stored ...
    let mut trans = open_transaction(r_cnx).map_err(err_fwd!("Open transaction error"))?;

    let sql_query = format!(r"UPDATE fs_{}.file_reference
    SET original_file_size = :p_original_file_size, total_part = :p_total_part
    WHERE id = :p_file_id "
    , customer_code);

    let sequence_name = format!( "fs_{}.file_reference_id_seq", customer_code );

    let mut params = HashMap::new();
    params.insert("p_original_file_size".to_string(), CellValue::from_raw_int(total_size as i64));
    params.insert("p_total_part".to_string(), CellValue::from_raw_int_32(total_part as i32));
    params.insert("p_file_id".to_string(), CellValue::from_raw_int(file_id));

    let sql_update = SQLChange {
        sql_query,
        params,
        sequence_name,
    };

    let _ = sql_update.update(&mut trans).map_err(err_fwd!("Insertion failed"))?;

    let _ = trans.commit();

    Ok(())
}


///
/// Upload the binary content of a file
/// Split into parts and store them (parallel)
/// Call tika
/// Call document_server.ft_indexing() (parallel process)
///
/// TODO keeping the entire binary content in memory is not a neat idea
///     please, store the parts in the db and pass the file handle around.
///
#[post("/upload", data = "<file_data>")]
pub fn upload(file_data: Data, session_token : SessionToken) -> Json<UploadReply> {

    // Check if the token is valid
    if !session_token.is_valid() {
        if cfg!(windows) {
            empty_datastream(&mut file_data.open().take(u64::MAX));
        }
        return Json(UploadReply::invalid_token_error_reply());
    }
    let sid = session_token.take_value();

    log_info!("🚀 Start upload api, sid={}", &sid);

    let internal_database_error_reply = Json(UploadReply::internal_database_error_reply());
    let internal_technical_error = Json(UploadReply::internal_technical_error_reply());

    // Read the session information
    let entry_session = match fetch_entry_session(&sid).map_err(err_fwd!("Session Manager failed")) {
        Ok(x) => x,
        Err(e) => {
            log_error!("{}", e);
            if cfg!(windows) {
                empty_datastream(&mut file_data.open().take(u64::MAX));
            }
            return internal_technical_error;
        }
    };

    let customer_code = entry_session.customer_code.as_str();

    // Get the crypto key

    let customer_key = match fetch_customer_key(customer_code, &sid).map_err(err_fwd!("Cannot get the customer key")) {
        Ok(x) => x,
        Err(e) => {
            log_error!("{}", e);
            if cfg!(windows) {
                empty_datastream(&mut file_data.open().take(u64::MAX));
            }
            return internal_technical_error;
        }
    };

    // Create an entry in file_reference
    let mut r_cnx = SQLConnection::new();

    let ( file_id, file_ref ) = match create_file_reference(&mut r_cnx, customer_code)
            .map_err(err_fwd!("Cannot create an entry in the file reference table")) {
        Ok(x) => x,
        Err(e) => {
            log_error!("{}", e);
            if cfg!(windows) {
                empty_datastream(&mut file_data.open().take(u64::MAX));
            }
            return internal_database_error_reply;
        }
    };

    log_info!("Created entry in file reference, file_id=[{}], file_ref=[{}]", file_id, &file_ref);

    // Create parts

    let mut mem_file : Vec<u8> = vec![];
    let mut thread_pool = vec![];

    const MAX_BUF : usize = 1_024;
    // const MAX_BUF : usize = 600;
    let mut block : [u8; BLOCK_SIZE] = [0; BLOCK_SIZE];
    let mut block_index : usize = 0;
    let mut block_num : u32 = 0;
    let mut total_size : usize = 0;
    let mut datastream = file_data.open();
    let mut block_set : HashMap<u32, Vec<u8>> = HashMap::new();

    loop {
        let mut buf : [u8; MAX_BUF] = [0; MAX_BUF];
        let r_bytes = datastream.read(&mut buf);

        match r_bytes {
            Ok(nb_bytes) => {
                // log_info!("Ok : [{:?}]", &buf);
                if nb_bytes == 0 {
                    break;
                } else {

                    for b_index in 0..nb_bytes {
                        if block_index >= BLOCK_SIZE {
                            let slice = &block[0..block_index];
                            block_set.insert(block_num, slice.to_vec());
                            if block_set.len() >= 10 {
                                thread_pool.push(
                                    parallel_crypto_and_store_block(file_id, &block_set,  customer_code, &customer_key)
                                );
                                block_set.clear();
                            }

                            total_size += block_index;
                            block_num += 1;
                            block = [0; BLOCK_SIZE];
                            block_index = 0;
                        }

                        let x  = buf[b_index];
                        block[block_index] = x;
                        block_index += 1;

                        // Store the byte in a memory file
                        mem_file.push(x);
                    }

                }
            }
            Err(_) => {
                break;
            }
        }
    }

    if block_index > 0 {
        let slice = &block[0..block_index];
        log_info!("End slice : {} {}", block_index, slice.len());
        block_set.insert(block_num, slice.to_vec());


        thread_pool.push(parallel_crypto_and_store_block(file_id, &block_set,/*block_num, slice,*/
                                                             customer_code,
                                                             &customer_key));
        block_set.clear();

        total_size += block_index;
        block_num += 1;
    }

    let original_file_size = mem_file.len();

    // Create the content parsing process
    // | We know that all the blocks have been read and the mem_file contains all the data.
    // | We don't know the state of each parts
    // | But still we know the original_file_size (mem_file.len()) and the total_part (block_num)
    thread_pool.push(parallel_parse_content(&file_ref, mem_file, customer_code, &sid));

    thread::spawn( || {
        for th in thread_pool {
            if let Err(e) = th.join() {
                log_error!("Thread join error [{:?}]", e);
            }
        }
    });

    // Update the file_reference table : checksum, original_file_size, total_part
    if update_file_reference(&mut r_cnx, file_id, original_file_size, block_num, customer_code)
        .map_err(err_fwd!("Cannot create an entry in the file reference table")).is_err() {
            return internal_database_error_reply;
    }

    log_info!("🏁 End upload api, sid={}", &sid);

    Json(UploadReply {
        file_ref,
        size : total_size,
        block_count: block_num,
        status: JsonErrorSet::from(SUCCESS),
    })
}


#[get("/info/<file_ref>")]
pub fn file_info(file_ref: &RawStr, session_token : SessionToken) -> Json<GetFileInfoReply> {

    // Check if the token is valid
    if !session_token.is_valid() {
        return Json(GetFileInfoReply::invalid_token_error_reply());
    }
    let sid = session_token.take_value();

    log_info!("🚀 Start upload api, sid={}", &sid);

    // Read the session information
    let entry_session = match fetch_entry_session(&sid).map_err(err_fwd!("Session Manager failed")) {
        Ok(x) => x,
        Err(e) => {
            log_error!("{}", e);
            return Json(GetFileInfoReply::internal_technical_error_reply());
        }
    };

    let customer_code = entry_session.customer_code.as_str();

    let mut block_status = vec![];


    let sql_query = format!(r"SELECT fp.id, fr.file_ref, fp.part_number,
            fp.is_encrypted,
            fr.is_fulltext_parsed,
            fr.is_preview_generated
        FROM  fs_{}.file_reference fr, fs_{}.file_parts fp
        WHERE
            fp.file_reference_id = fr.id AND
            fr.file_ref = :p_file_reference
        ORDER BY fr.file_ref, fp.part_number", customer_code, customer_code);

    let r_data_set : anyhow::Result<SQLDataSet> = (|| {
        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx)?;

        let mut params = HashMap::new();
        params.insert("p_file_reference".to_string(), CellValue::from_raw_string(file_ref.to_string()));

        let query = SQLQueryBlock {
            sql_query,
            start: 0,
            length: None,
            params,
        };

        let dataset = query.execute(&mut trans)?;

        let _ = trans.commit();

        Ok(dataset)

    })();

    let mut data_set = match r_data_set {
        Ok(x) => {x}
        Err(_) => {
            return Json(GetFileInfoReply::internal_database_error_reply());
        }
    };

    while data_set.next() {

        let block_number = data_set.get_int_32("part_number").unwrap_or(-1) as u32;
        let is_encrypted = data_set.get_bool("is_encrypted").unwrap_or(false);
        let is_fulltext_indexed = data_set.get_bool("is_fulltext_indexed").unwrap_or(false);
        let is_preview_generated = data_set.get_bool("is_preview_generated").unwrap_or(false);
        //let part_length = data_set.get_int_32("part_length").unwrap_or(-1) as u32;

        block_status.push(Some(BlockStatus {
            original_size: 0,
            block_number,
            is_encrypted,
            is_fulltext_indexed,
            is_preview_generated
        }));
    }

    Json(GetFileInfoReply {
        file_ref : file_ref.to_string(),
        block_count: block_status.len() as u32,
        block_status,
        status: JsonErrorSet::from(SUCCESS),
    })
}

#[get("/stats/<file_ref>")]
pub fn file_stats(file_ref: &RawStr, session_token : SessionToken) -> Json<GetFileInfoShortReply> {

    // Check if the token is valid
    if !session_token.is_valid() {
        return Json(GetFileInfoShortReply::invalid_token_error_reply());
    }
    let sid = session_token.take_value();

    log_info!("🚀 Start upload api, sid={}", &sid);

    // Read the session information
    let entry_session = match fetch_entry_session(&sid).map_err(err_fwd!("Session Manager failed")) {
        Ok(x) => x,
        Err(e) => {
            log_error!("{}", e);
            return Json(GetFileInfoShortReply::internal_technical_error_reply());
        }
    };

    let customer_code = entry_session.customer_code.as_str();

    // TODO instead of constant 1, check if the document is fulltext parsed and previewed
    let sql_query = format!(r"
SELECT fr.mime_type, fr.checksum, fr.original_file_size, fr.total_part, enc.encrypted, fulltext.fulltext, preview.preview
	FROM fs_{}.file_reference fr,
	(SELECT count(*) as encrypted
		FROM  fs_{}.file_parts
		WHERE file_reference_id = (SELECT id FROM fs_{}.file_reference WHERE file_ref = :p_file_ref)
		AND is_encrypted = true) enc,
	1 fulltext,
	1 preview
	WHERE file_ref = :p_file_ref ", customer_code, customer_code, customer_code );

    let r_data_set : anyhow::Result<SQLDataSet> = (|| {
        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx)?;

        let mut params = HashMap::new();
        params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.to_string()));

        let query = SQLQueryBlock {
            sql_query,
            start: 0,
            length: None,
            params,
        };

        let dataset = query.execute(&mut trans)?;
        trans.commit()?;

        Ok(dataset)
    })();

    let mut data_set = match r_data_set {
        Ok(x) => {x}
        Err(_) => {
            return Json(GetFileInfoShortReply{
                file_ref: "".to_string(),
                original_file_size : 0,
                block_count: 0,
                encrypted_count: 0,
                fulltext_indexed_count: 0,
                preview_generated_count: 0,
                status: JsonErrorSet::from(INTERNAL_DATABASE_ERROR),
            })
        }
    };

    let stats;
    if data_set.next() {

        let _mime_type = data_set.get_string("mime_type").unwrap_or("".to_string());
        let _checksum = data_set.get_string("checksum").unwrap_or("".to_string());
        let original_file_size = data_set.get_int("original_file_size").unwrap_or(0i64);
        let total_part = data_set.get_int_32("total_part").unwrap_or(0i32);
        let encrypted_count = data_set.get_int("encrypted").unwrap_or(0);
        let fulltext_indexed_count = data_set.get_int("fulltext").unwrap_or(0);
        let preview_generated_count = data_set.get_int("preview").unwrap_or(0);

        stats = GetFileInfoShortReply{
            file_ref: file_ref.to_string(),
            block_count: total_part as u32,
            original_file_size : original_file_size as u64,
            encrypted_count,
            fulltext_indexed_count,
            preview_generated_count,
            status: JsonErrorSet::from(SUCCESS),
        };

    } else {
        stats = GetFileInfoShortReply{
            file_ref: "".to_string(),
            original_file_size : 0,
            block_count: 0,
            encrypted_count: 0,
            fulltext_indexed_count: 0,
            preview_generated_count: 0,
            status: JsonErrorSet::from(INTERNAL_TECHNICAL_ERROR),
        };
    }

    Json(stats)

}



#[get("/download/<file_ref>")]
pub fn download(file_ref: &RawStr, _session_token : SessionToken) -> Content<Vec<u8>> {

    // Check if the token is valid
    let sid = "n/a";
    log_info!("🚀 Start download api, file_ref = [{}], sid=[{}]", file_ref, &sid);
    //
    // // Create parts
    // log_info!("🏁 End upload api, sid={}", &sid);

    let mut file = File::open("c:/Users/denis/Dropbox/Upload/russian_planet.pdf").unwrap();

    let mut bytes = vec![];
    let _b = file.read_to_end(&mut bytes);

    Content(ContentType::PDF, bytes)

}


fn main() {

    const PROGRAM_NAME: &str = "File Server";

    println!("😎 Init {}", PROGRAM_NAME);

    const PROJECT_CODE: &str = "file-server";
    const VAR_NAME: &str = "DOKA_ENV";

    // Read the application config's file
    println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);

    let props = read_config(PROJECT_CODE, VAR_NAME);
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
            file_info,
            file_stats,
            download,
        ])
        .attach(Template::fairing())
        .launch();

    log_info!("🏁 End {}", PROGRAM_NAME);
}

#[cfg(test)]
mod test {
    use std::path::Path;
    use std::process::exit;
    use commons_pg::{init_db_pool, SQLConnection, SQLTransaction};
    use commons_services::database_lib::open_transaction;
    use dkconfig::conf_reader::read_config;
    use dkconfig::properties::{get_prop_pg_connect_string, get_prop_value, set_prop_values};
    use crate::{insert_document_part, parse_content, select_tsvector};
    use log::{error,info};
    use commons_error::*;

    fn init_test() {
        const PROGRAM_NAME: &str = "Test File Server";

        println!("😎 Init {}", PROGRAM_NAME);

        const PROJECT_CODE: &str = "file-server";
        const VAR_NAME: &str = "DOKA_ENV";

        // Read the application config's file
        println!("😎 Config file using PROJECT_CODE={} VAR_NAME={}", PROJECT_CODE, VAR_NAME);

        let props = read_config(PROJECT_CODE, VAR_NAME);
        set_prop_values(props);

        log_info!("🚀 Start {}", PROGRAM_NAME);

        let log_config: String = get_prop_value(LOG_CONFIG_FILE_PROPERTY);
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
    }

    #[test]
    fn test_parse_content() -> anyhow::Result<()> {
        init_test();
        let mem_file: Vec<u8> = std::fs::read("C:/Users/denis/wks-poc/tika/big_planet.pdf")?;
        let ret = parse_content("0f373b54-5dbb-4c75-98e7-98fd141593dc", mem_file, "f1248fab", "MY_SID")?;
        Ok(())
    }


    #[test]
    fn test_compute_tsvector() -> anyhow::Result<()> {
        init_test();
        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx)?;
        let ret = select_tsvector(&mut trans,Some("french"), "Planète Phase formation cœurs planétaires Phase formation noyaux telluriques moderne")?;
        assert_eq!("'coeur':4 'format':3,7 'modern':10 'noyal':8 'phas':2,6 'planet':1 'planetair':5 'tellur':9", ret);
        Ok(())
    }

    #[test]
    fn test_insert_document() -> anyhow::Result<()> {
        init_test();
        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx)?;

        let id = insert_document_part(&mut trans, "0f373b54-5dbb-4c75-98e7-98fd141593dc", 107,
                                     "Phase formation cœurs planétaires Phase formation noyaux telluriques moderne",
                                     "french", "f1248fab" )?;

        trans.commit();
        log_info!("ID = [{}]", id);
        assert!(id > 0);
        Ok(())
    }

}