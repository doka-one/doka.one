use std::collections::HashMap;
use rocket_contrib::json::Json;
use rocket::{post};
use commons_pg::{CellValue, SQLChange, SQLConnection, SQLQueryBlock, SQLTransaction};
use commons_services::database_lib::open_transaction;
use commons_services::property_name::{TIKA_SERVER_HOSTNAME_PROPERTY, TIKA_SERVER_PORT_PROPERTY};
use commons_services::session_lib::fetch_entry_session;
use commons_services::token_lib::SessionToken;
use dkconfig::properties::get_prop_value;
use dkdto::{FullTextReply, FullTextRequest, JsonErrorSet};
use dkdto::error_codes::{SUCCESS};
use doka_cli::request_client::TikaServerClient;
use crate::ft_tokenizer::{encrypt_tsvector, FTTokenizer};
use log::{info,error};
use commons_error::*;
use commons_error::{log_error};
use commons_services::key_lib::fetch_customer_key;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::error_replies::ErrorReply;
use crate::language::{lang_name_from_code_2, map_code};


fn select_tsvector(mut trans : &mut SQLTransaction, lang: Option<&str>, text: &str) -> anyhow::Result<String> {
    let sql_query = match lang {
        None => {
            r"SELECT CAST( to_tsvector(unaccent_lower(:p_doc_text)) as VARCHAR ) as tsv".to_string()
        }
        Some(lg) => {
            format!(r"SELECT CAST( to_tsvector('{}',  unaccent_lower(:p_doc_text)) as VARCHAR ) as tsv", lg)
        }
    };

    let mut params = HashMap::new();
    params.insert("p_doc_text".to_string(), CellValue::from_raw_string(text.to_string()));
    let sql_block = SQLQueryBlock {
        sql_query: sql_query.to_string(),
        start: 0,
        length: None,
        params
    };

    let mut data = sql_block.execute(&mut trans).map_err(err_fwd!("Error compute tsvector"))?;

    let tsv = if data.next() {
        data.get_string("tsv").unwrap_or("ERROR".to_string())
    } else {
        return Err(anyhow::anyhow!("Impossible to compute the tsvector"));
    };

    Ok(tsv)
}


fn insert_document_part( mut trans : &mut SQLTransaction,
                         file_ref : &str,
                         part_no: u32,
                         words_text: &str,
                         lang: &str,
                         customer_code: &str,
                         customer_key: &str) -> anyhow::Result<i64> {

    log_info!("Insert document, file_ref=[{}], part_no=[{}]", file_ref, part_no);

    let words_encrypted = DkEncrypt::encrypt_str(words_text, customer_key)
        .map_err(err_fwd!("Cannot encrypt the words"))?;

    let tsv = select_tsvector(&mut trans, Some(lang), words_text )
        .map_err(err_fwd!("Cannot build the tsvector"))?;

    // Encrypt the words of the tsvector
    let tsv_encrypted = encrypt_tsvector(&tsv, customer_key);
    log_info!("Encrypted tsvector: [{}]", &tsv_encrypted);

    // Use a stored proc to hide the TSVECTOR type from Rust
    let sql_query = format!(r"CALL cs_{}.insert_document( :p_file_ref, :p_part_no, :p_doc_text, :p_tsv, :p_lang )", customer_code);

    // Still very important to name the sequence to get the id back after the insert
    let sequence_name = format!( "cs_{}.document_id_seq", customer_code );

    let mut params = HashMap::new();
    params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.to_string()));
    params.insert("p_part_no".to_string(), CellValue::from_raw_int_32(part_no as i32));
    params.insert("p_doc_text".to_string(), CellValue::from_raw_string(words_encrypted));
    params.insert("p_tsv".to_string(), CellValue::from_raw_string(tsv_encrypted));
    params.insert("p_lang".to_string(), CellValue::from_raw_string(lang.to_string()));

    let sql_insert = SQLChange {
        sql_query,
        params,
        sequence_name,
    };

    let document_id = sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion failed"))?;

    Ok(document_id)
}


// fn create_full_default_item(mut trans : &mut SQLTransaction, file_name : &str, customer_code: &str, sid: &str) -> anyhow::Result<i64> {
//     let item_id = create_item(trans, file_name, customer_code)?;
//
//     // TODO the item_file table is totally useless, please move the file_ref into the item table...
//     let _ = create_item_file(trans, item_id, file_ref, customer_code);
//
//     Ok(item_id)
// }

fn indexing(mut trans : &mut SQLTransaction, raw_text_request: &FullTextRequest, customer_code: &str, customer_key: &str, _sid: &str) -> anyhow::Result<u32> {

    // Limit the number of languages. Build it the static way.
    const FINESSE_LANGUAGE_BLOCK : usize  = 1_000; // nb of chars for the language detection
    const MAX_LANGUAGE_BUFFER_BLOCK : usize = 200_000; // TODO  > 200_000;

    let mut language_buffer_block: HashMap<String, Vec<String>> = HashMap::new(); // { "french", Vec<PureWord> }

    let tika_server_host = get_prop_value(TIKA_SERVER_HOSTNAME_PROPERTY);
    let tika_server_port = get_prop_value(TIKA_SERVER_PORT_PROPERTY).parse::<u16>()?;

    let tsc = TikaServerClient::new(&tika_server_host, tika_server_port);

    // Clean up the raw text
    let mut ftt = FTTokenizer::new(&raw_text_request.raw_text);

    loop {
        // Parse the words from the raw data until FINESSE_NBCHAR => pure word block (PWB)
        let mut pure_word_block = ftt.next_n_words(FINESSE_LANGUAGE_BLOCK);
        if pure_word_block.is_empty() {
            break;
        }

        // language detection on the language of the pure word block
        let meta_data = tsc.read_meta(&pure_word_block.join(" ")).map_err(err_fwd!("Cannot read meta information"))?;
        let lang_code = map_code(&meta_data.language);

        let language_words = match language_buffer_block.get_mut(lang_code) {
            None => {
                log_info!("Init the language map for language=[{}]", lang_code);
                language_buffer_block.insert(lang_code.to_string(), vec![] );
                language_buffer_block.get_mut(lang_code).unwrap()
            }
            Some(lw) => {lw}
        };

        log_info!("Add words for language, nb words=[{}], language=[{}({})]", pure_word_block.len(), lang_code, &meta_data.language);
        language_words.append(&mut pure_word_block);

    }

    log_info!("Init part counter, file_ref=[{}]", &raw_text_request.file_ref);
    let mut part_no = 0;
    for (l, words) in &language_buffer_block {
        log_info!("For language [{}] : [{}] : {:?}", l , words.len(), words);

        let mut word_text : Vec<String> = vec![];
        let mut word_text_size = 0;
        let last_word_index = words.len()-1;
        for i in 0..=last_word_index {
            let w = words.get(i).ok_or(anyhow::anyhow!("No word to read"))?;
            word_text.push(w.clone());
            word_text_size += w.len();

            if word_text_size >= MAX_LANGUAGE_BUFFER_BLOCK || i == last_word_index {

                log_info!("Create a new part, file_ref=[{}], part_no=[{}]", &raw_text_request.file_ref, part_no);
                let _id = insert_document_part(&mut trans, &raw_text_request.file_ref, part_no,
                                              &word_text.join(" "),
                                              lang_name_from_code_2(l), customer_code, customer_key)
                    .map_err(err_fwd!("Cannot insert the part no [{}]", part_no))?;

                log_info!("Create a new part Done, file_ref=[{}], part_no=[{}]", &raw_text_request.file_ref, part_no);
                part_no += 1;
                word_text.clear();
                word_text_size = 0;
            }
        }

    }

    Ok(part_no)
}

///
/// Parse the raw text data and create the document parts
/// Used from file-server
///
#[post("/fulltext_indexing", format = "application/json", data = "<raw_text_request>")]
pub (crate) fn fulltext_indexing(raw_text_request: Json<FullTextRequest>, session_token: SessionToken) -> Json<FullTextReply> {
    dbg!(&raw_text_request);
    // Check if the token is valid
    if !session_token.is_valid() {
        return Json(FullTextReply::invalid_token_error_reply());
    }
    let sid = session_token.take_value();

    log_info!("🚀 Start fulltext_indexing api, sid={}", &sid);

    let internal_database_error_reply = Json(FullTextReply::internal_database_error_reply());
    let internal_technical_error = Json(FullTextReply::internal_technical_error_reply());

    // Read the session information
    let entry_session = match fetch_entry_session(&sid).map_err(err_fwd!("Session Manager failed")) {
        Ok(x) => x,
        Err(_) => {
            return internal_technical_error;
        }
    };

    let customer_code = entry_session.customer_code.as_str();

    // Get the crypto key

    let customer_key = match fetch_customer_key(customer_code, &sid).map_err(err_fwd!("Cannot get the customer key")) {
        Ok(x) => x,
        Err(e) => {
            log_error!("{}", e);
            return internal_technical_error;
        }
    };

    ////////////////////

    let mut r_cnx = SQLConnection::new();
    let mut trans = match open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error")) {
        Ok(x) => { x },
        Err(_) => { return internal_database_error_reply; },
    };

    // Generate the FT index and create an entry in the "document" table
    let part_count  = match indexing(&mut trans, &raw_text_request, customer_code, &customer_key, &sid) {
        Ok(count) => {count}
        Err(_) => {
            return internal_technical_error;
        }
    };

    let _r = trans.commit();

    Json(FullTextReply {
        part_count,
        status: JsonErrorSet::from(SUCCESS),
    })
}