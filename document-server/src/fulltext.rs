use std::collections::HashMap;

use log::*;
use rocket::http::Status;
use rocket_contrib::json::Json;

use commons_error::*;
use commons_pg::{CellValue, SQLChange, SQLConnection, SQLQueryBlock, SQLTransaction};
use commons_services::database_lib::open_transaction;
use commons_services::key_lib::fetch_customer_key;
use commons_services::property_name::{TIKA_SERVER_HOSTNAME_PROPERTY, TIKA_SERVER_PORT_PROPERTY};
use commons_services::session_lib::fetch_entry_session;
use commons_services::token_lib::SessionToken;
use commons_services::x_request_id::{Follower, XRequestID};
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::{FullTextReply, FullTextRequest, WebType, WebTypeBuilder};
use dkdto::error_codes::{INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR, INVALID_TOKEN};
use doka_cli::request_client::{TikaServerClient, TokenType};

use crate::ft_tokenizer::{encrypt_tsvector, FTTokenizer};
use crate::language::{lang_name_from_code_2, map_code};

pub(crate) struct FullTextDelegate {
    pub session_token: SessionToken,
    pub follower: Follower,
}

impl FullTextDelegate {
    pub fn new(session_token: SessionToken, x_request_id: XRequestID) -> Self {
        Self {
            session_token,
            follower: Follower {
                x_request_id: x_request_id.new_if_null(),
                token_type: TokenType::None,
            }
        }
    }


    /// ✨ Parse the raw text data and create the document parts
    /// Service called from the file-server
    pub fn fulltext_indexing(mut self, raw_text_request: Json<FullTextRequest>) -> WebType<FullTextReply> {

        log_info!("🚀 Start fulltext_indexing api, follower=[{}]", &self.follower);

        // Check if the token is valid
        if !self.session_token.is_valid() {
            log_error!("💣 Invalid session token, token=[{:?}], follower=[{}]", &self.session_token, &self.follower);
            return WebType::from_errorset(INVALID_TOKEN);
        }
        self.follower.token_type = TokenType::Sid(self.session_token.0.clone());

        // Read the session information
        let Ok(entry_session) = fetch_entry_session(&self.follower.token_type.value())
                                                    .map_err(err_fwd!("💣 Session Manager failed, follower=[{}]", &self.follower)) else {
            return WebType::from_errorset(INTERNAL_TECHNICAL_ERROR);
        };

        let customer_code = entry_session.customer_code.as_str();

        // Get the crypto key

        let Ok(customer_key) = fetch_customer_key(customer_code, &self.follower)
                                                    .map_err(err_fwd!("💣 Cannot get the customer key, follower=[{}]", &self.follower)) else {
            return WebType::from_errorset(INTERNAL_TECHNICAL_ERROR);
        };

        let mut r_cnx = SQLConnection::new();
        let r_trans = open_transaction(&mut r_cnx).map_err(err_fwd!("💣 Open transaction error, follower=[{}]", &self.follower));
        let Ok(mut trans) = r_trans else {
             return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        };

        // Generate the FT index and create an entry in the "document" table
        let Ok(part_count)  = self.indexing(&mut trans, &raw_text_request, customer_code, &customer_key)
                            .map_err(err_fwd!("💣 Indexing process failed, follower=[{}]", &self.follower)) else {
            return WebType::from_errorset(INTERNAL_TECHNICAL_ERROR);
        };

        if trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower)).is_err() {
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        }

        log_info!("😎 Generated the indexes and the document part entries, number of parts=[{}], follower=[{}]", part_count, &self.follower);
        log_info!("🏁 End fulltext_indexing api, follower=[{}]", &self.follower);

        WebType::from_item(Status::Ok.code,FullTextReply { part_count  })
    }


    fn indexing(&self, mut trans : &mut SQLTransaction, raw_text_request: &FullTextRequest, customer_code: &str, customer_key: &str) -> anyhow::Result<u32> {

        // Limit the number of languages. Build it the static way.
        const FINESSE_LANGUAGE_BLOCK : usize  = 1_000; // nb of chars for the language detection
        const MAX_LANGUAGE_BUFFER_BLOCK : usize = 200_000; // TODO  > 200_000;

        let mut language_buffer_block: HashMap<String, Vec<String>> = HashMap::new(); // { "french", Vec<PureWord> }

        let tika_server_host = get_prop_value(TIKA_SERVER_HOSTNAME_PROPERTY).map_err(tr_fwd!())?;
        let tika_server_port = get_prop_value(TIKA_SERVER_PORT_PROPERTY).map_err(tr_fwd!())?
            .parse::<u16>().map_err(tr_fwd!())?;

        let tsc = TikaServerClient::new(&tika_server_host, tika_server_port);

        // Clean up the raw text
        let mut ftt = FTTokenizer::new(&raw_text_request.raw_text);

        log_info!("Parsing the words from the raw data, file_ref=[{}], follower=[{}]", &raw_text_request.file_ref, &self.follower);
        loop {
            // Parse the words from the raw data until FINESSE_NBCHAR => pure word block (PWB)
            let mut pure_word_block = ftt.next_n_words(FINESSE_LANGUAGE_BLOCK);
            if pure_word_block.is_empty() {
                break;
            }

            // language detection on the language of the pure word block
            let meta_data = tsc.read_meta(&pure_word_block.join(" ")).map_err(err_fwd!("Cannot read meta information, follower=[{}]", &self.follower))?;
            let lang_code = map_code(&meta_data.language);

            let language_words = match language_buffer_block.get_mut(lang_code) {
                None => {
                    log_debug!("Init the language map for language=[{}], follower=[{}]", lang_code, &self.follower);
                    language_buffer_block.insert(lang_code.to_string(), vec![] );
                    language_buffer_block.get_mut(lang_code).unwrap()
                }
                Some(lw) => {lw}
            };

            log_debug!("Add words for language, nb words=[{}], language=[{}({})], follower=[{}]", pure_word_block.len(), lang_code, &meta_data.language, &self.follower);
            language_words.append(&mut pure_word_block);
        }

        log_info!("Init part counter, file_ref=[{}], follower=[{}]", &raw_text_request.file_ref, &self.follower);
        let mut part_no = 0;
        for (l, words) in &language_buffer_block {
            log_debug!("For language=[{}] : len=[{}] : words={:?}, follower=[{}]", l , words.len(), words, &self.follower);

            let mut word_text : Vec<String> = vec![];
            let mut word_text_size = 0;
            let last_word_index = words.len()-1;
            for i in 0..=last_word_index {
                let w = words.get(i).ok_or(anyhow::anyhow!("No word to read"))?;
                word_text.push(w.clone());
                word_text_size += w.len();

                if word_text_size >= MAX_LANGUAGE_BUFFER_BLOCK || i == last_word_index {

                    log_info!("Create a new part, file_ref=[{}], part_no=[{}]", &raw_text_request.file_ref, part_no);
                    let _id = self.insert_document_part(&mut trans, &raw_text_request.file_ref, part_no,
                                                   &word_text.join(" "),
                                                   lang_name_from_code_2(l), customer_code, customer_key)
                        .map_err(err_fwd!("Cannot insert the part no [{}], follower=[{}]", part_no, &self.follower))?;

                    log_info!("Create a new part Done, file_ref=[{}], part_no=[{}], follower=[{}]", &raw_text_request.file_ref, part_no, &self.follower);
                    part_no += 1;
                    word_text.clear();
                    word_text_size = 0;
                }
            }

        }

        Ok(part_no)
    }

    ///
    fn insert_document_part( &self,
                             mut trans : &mut SQLTransaction,
                             file_ref : &str,
                             part_no: u32,
                             words_text: &str,
                             lang: &str,
                             customer_code: &str,
                             customer_key: &str) -> anyhow::Result<i64> {

        log_info!("Insert document, file_ref=[{}], part_no=[{}], follower=[{}]", file_ref, part_no, &self.follower);

        let words_encrypted = DkEncrypt::encrypt_str(words_text, customer_key)
            .map_err(err_fwd!("Cannot encrypt the words, follower=[{}]", &self.follower))?;

        let tsv = self.select_tsvector(&mut trans, Some(lang), words_text )
            .map_err(err_fwd!("Cannot build the tsvector, follower=[{}]", &self.follower))?;

        // Encrypt the words of the tsvector, it's actually a Sha256 hash for each single word
        let tsv_encrypted = encrypt_tsvector(&tsv, customer_key)
            .map_err(err_fwd!("Cannot encrypt the vector, follower=[{}]", &self.follower))?;
        log_info!("Encrypted tsvector length: [{}]", tsv_encrypted.len());

        // dbg!(&tsv_encrypted);

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

        let document_id = sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;

        Ok(document_id)
    }

    ///
    fn select_tsvector(&self, mut trans : &mut SQLTransaction, lang: Option<&str>, text: &str) -> anyhow::Result<String> {
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

}






