use std::collections::HashMap;

use axum::http::StatusCode;
use axum::Json;
use log::*;
use serde::de::DeserializeOwned;

use commons_error::*;
use commons_pg::sql_transaction::CellValue;
use commons_pg::sql_transaction_async::{
    SQLChangeAsync, SQLConnectionAsync, SQLQueryBlockAsync, SQLTransactionAsync,
};
use commons_services::key_lib::fetch_customer_key;
use commons_services::property_name::{TIKA_SERVER_HOSTNAME_PROPERTY, TIKA_SERVER_PORT_PROPERTY};
use commons_services::session_lib::valid_sid_get_session;
use commons_services::token_lib::SessionToken;
use commons_services::try_or_return;
use commons_services::x_request_id::{Follower, XRequestID};
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::{
    DeleteFullTextRequest, ErrorSet, FullTextReply, FullTextRequest, SimpleMessage, WebType,
    WebTypeBuilder,
};
use dkdto::error_codes::{INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR};
use doka_cli::async_request_client::TikaServerClientAsync;
use doka_cli::request_client::TokenType;

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
            },
        }
    }

    fn web_type_error<T>() -> impl Fn(&ErrorSet<'static>) -> WebType<T>
    where
        T: DeserializeOwned,
    {
        |e| {
            log_error!("💣 Error after try {:?}", e);
            WebType::from_errorset(e)
        }
    }

    /// ✨ Delete the information linked to the document full text indexing information
    /// Service called from the file-server
    pub async fn delete_text_indexing(
        mut self,
        delete_text_request: Json<DeleteFullTextRequest>,
    ) -> WebType<SimpleMessage> {
        log_info!(
            "🚀 Start delete_text_indexing api, follower=[{}]",
            &self.follower
        );

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        // Delete all the document related to the file reference
        let _ = self
            .delete_document(&delete_text_request.file_ref, &entry_session.customer_code)
            .await;

        log_info!(
            "😎 Deleted the document part entries, follower=[{}]",
            &self.follower
        );
        log_info!(
            "🏁 End delete_text_indexing api, follower=[{}]",
            &self.follower
        );
        WebType::from_item(
            StatusCode::OK.as_u16(),
            SimpleMessage {
                message: "Ok".to_string(),
            },
        )
    }

    ///
    async fn delete_document(&self, file_ref: &str, customer_code: &str) -> anyhow::Result<()> {
        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

        let sql_delete = format!(
            r"DELETE FROM cs_{0}.document WHERE file_ref = :p_file_ref",
            customer_code
        );

        let mut params = HashMap::new();
        params.insert("p_file_ref".to_string(), CellValue::from_raw_str(file_ref));

        let query = SQLChangeAsync {
            sql_query: sql_delete.to_string(),
            params,
            sequence_name: "".to_string(),
        };

        let _id = query.delete(&mut trans).await.map_err(err_fwd!(
            "💣 Query failed, [{}], , follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;
        trans
            .commit()
            .await
            .map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;
        Ok(())
    }

    /// ✨ Parse the raw text data and create the document parts
    /// Service called from the file-server
    pub async fn fulltext_indexing(
        mut self,
        raw_text_request: Json<FullTextRequest>,
    ) -> WebType<FullTextReply> {
        log_info!(
            "🚀 Start fulltext_indexing api, follower=[{}]",
            &self.follower
        );

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        let customer_code = entry_session.customer_code.as_str();

        // Get the crypto key
        let Ok(customer_key) = fetch_customer_key(customer_code, &self.follower)
            .await
            .map_err(err_fwd!(
                "💣 Cannot get the customer key, follower=[{}]",
                &self.follower
            ))
        else {
            return WebType::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };

        // Open Db connection
        let Ok(mut cnx) = SQLConnectionAsync::from_pool().await.map_err(err_fwd!(
            "💣 New Db connection failed, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        let Ok(mut trans) = cnx.begin().await.map_err(err_fwd!(
            "💣 Transaction issue, follower=[{}]",
            &self.follower
        )) else {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        // Generate the FT index and create an entry in the "document" table
        let Ok(part_count) = self
            .indexing(
                &mut trans,
                &raw_text_request,
                &entry_session.customer_code,
                &customer_key,
            )
            .await
            .map_err(err_fwd!(
                "💣 Indexing process failed, follower=[{}]",
                &self.follower
            ))
        else {
            return WebType::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };

        if trans
            .commit()
            .await
            .map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))
            .is_err()
        {
            return WebType::from_errorset(&INTERNAL_DATABASE_ERROR);
        }

        log_info!("😎 Generated the indexes and the document part entries, number of parts=[{}], follower=[{}]", part_count, &self.follower);
        log_info!(
            "🏁 End fulltext_indexing api, follower=[{}]",
            &self.follower
        );

        WebType::from_item(StatusCode::OK.as_u16(), FullTextReply { part_count })
    }

    async fn indexing(
        &self,
        mut trans: &mut SQLTransactionAsync<'_>,
        raw_text_request: &FullTextRequest,
        customer_code: &str,
        customer_key: &str,
    ) -> anyhow::Result<u32> {
        // Limit the number of languages. Build it the static way.
        const FINESSE_LANGUAGE_BLOCK: usize = 1_000; // nb of chars for the language detection
        const MAX_LANGUAGE_BUFFER_BLOCK: usize = 200_000; // TODO  > 200_000;

        let mut language_buffer_block: HashMap<String, Vec<String>> = HashMap::new(); // { "french", Vec<PureWord> }
        let tika_server_host = get_prop_value(TIKA_SERVER_HOSTNAME_PROPERTY).map_err(tr_fwd!())?;
        let tika_server_port = get_prop_value(TIKA_SERVER_PORT_PROPERTY)
            .map_err(tr_fwd!())?
            .parse::<u16>()
            .map_err(tr_fwd!())?;

        let tsc = TikaServerClientAsync::new(&tika_server_host, tika_server_port);
        // Clean up the raw text
        let mut ftt = FTTokenizer::new(&raw_text_request.raw_text);

        log_info!(
            "Parsing the words from the raw data, file_ref=[{}], follower=[{}]",
            &raw_text_request.file_ref,
            &self.follower
        );
        loop {
            // Parse the words from the raw data until FINESSE_NBCHAR => pure word block (PWB)
            let mut pure_word_block = ftt.next_n_words(FINESSE_LANGUAGE_BLOCK);
            if pure_word_block.is_empty() {
                break;
            }

            // language detection on the language of the pure word block
            let meta_data = tsc
                .read_meta(&pure_word_block.join(" "))
                .await
                .map_err(err_fwd!(
                    "Cannot read meta information, follower=[{}]",
                    &self.follower
                ))?;
            let lang_code = map_code(&meta_data.language);

            let language_words = match language_buffer_block.get_mut(lang_code) {
                None => {
                    log_debug!(
                        "Init the language map for language=[{}], follower=[{}]",
                        lang_code,
                        &self.follower
                    );
                    language_buffer_block.insert(lang_code.to_string(), vec![]);
                    language_buffer_block.get_mut(lang_code).unwrap()
                }
                Some(lw) => lw,
            };

            log_debug!(
                "Add words for language, nb words=[{}], language=[{}({})], follower=[{}]",
                pure_word_block.len(),
                lang_code,
                &meta_data.language,
                &self.follower
            );
            language_words.append(&mut pure_word_block);
        }

        log_info!(
            "Init part counter, file_ref=[{}], follower=[{}]",
            &raw_text_request.file_ref,
            &self.follower
        );
        let mut part_no = 0;
        for (l, words) in &language_buffer_block {
            log_debug!(
                "For language=[{}] : len=[{}] : words={:?}, follower=[{}]",
                l,
                words.len(),
                words,
                &self.follower
            );

            let mut word_text: Vec<String> = vec![];
            let mut word_text_size = 0;
            let last_word_index = words.len() - 1;
            for i in 0..=last_word_index {
                let w = words.get(i).ok_or(anyhow::anyhow!("No word to read"))?;
                word_text.push(w.clone());
                word_text_size += w.len();

                if word_text_size >= MAX_LANGUAGE_BUFFER_BLOCK || i == last_word_index {
                    log_info!(
                        "Create a new part, file_ref=[{}], part_no=[{}]",
                        &raw_text_request.file_ref,
                        part_no
                    );
                    let _id = self
                        .insert_document_part(
                            &mut trans,
                            &raw_text_request.file_ref,
                            part_no,
                            &word_text.join(" "),
                            lang_name_from_code_2(l),
                            customer_code,
                            customer_key,
                        )
                        .await
                        .map_err(err_fwd!(
                            "Cannot insert the part no [{}], follower=[{}]",
                            part_no,
                            &self.follower
                        ))?;

                    log_info!(
                        "Create a new part Done, file_ref=[{}], part_no=[{}], follower=[{}]",
                        &raw_text_request.file_ref,
                        part_no,
                        &self.follower
                    );
                    part_no += 1;
                    word_text.clear();
                    word_text_size = 0;
                }
            }
        }

        Ok(part_no)
    }

    ///
    async fn insert_document_part(
        &self,
        mut trans: &mut SQLTransactionAsync<'_>,
        file_ref: &str,
        part_no: u32,
        words_text: &str,
        lang: &str,
        customer_code: &str,
        customer_key: &str,
    ) -> anyhow::Result<i64> {
        log_info!(
            "Insert document, file_ref=[{}], part_no=[{}], follower=[{}]",
            file_ref,
            part_no,
            &self.follower
        );

        let words_encrypted = DkEncrypt::encrypt_str(words_text, customer_key).map_err(
            err_fwd!("Cannot encrypt the words, follower=[{}]", &self.follower),
        )?;

        let tsv = self
            .select_tsvector(&mut trans, Some(lang), words_text)
            .await
            .map_err(err_fwd!(
                "Cannot build the tsvector, follower=[{}]",
                &self.follower
            ))?;

        // Encrypt the words of the tsvector, it's actually a Sha256 hash for each single word
        let tsv_encrypted = encrypt_tsvector(&tsv, customer_key).map_err(err_fwd!(
            "Cannot encrypt the vector, follower=[{}]",
            &self.follower
        ))?;
        log_info!("Encrypted tsvector length: [{}]", tsv_encrypted.len());

        // dbg!(&tsv_encrypted);

        // Use a stored proc to hide the TSVECTOR type from Rust
        let sql_query = format!(
            r"CALL cs_{}.insert_document( :p_file_ref, :p_part_no, :p_doc_text, :p_tsv, :p_lang )",
            customer_code
        );

        // Still very important to name the sequence to get the id back after the insert
        let sequence_name = format!("cs_{}.document_id_seq", customer_code);

        let mut params = HashMap::new();
        params.insert(
            "p_file_ref".to_string(),
            CellValue::from_raw_string(file_ref.to_string()),
        );
        params.insert(
            "p_part_no".to_string(),
            CellValue::from_raw_int_32(part_no as i32),
        );
        params.insert(
            "p_doc_text".to_string(),
            CellValue::from_raw_string(words_encrypted),
        );
        params.insert(
            "p_tsv".to_string(),
            CellValue::from_raw_string(tsv_encrypted),
        );
        params.insert(
            "p_lang".to_string(),
            CellValue::from_raw_string(lang.to_string()),
        );

        let sql_insert = SQLChangeAsync {
            sql_query,
            params,
            sequence_name,
        };

        let document_id = sql_insert
            .insert(&mut trans)
            .await
            .map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;

        Ok(document_id)
    }

    ///
    async fn select_tsvector(
        &self,
        mut trans: &mut SQLTransactionAsync<'_>,
        lang: Option<&str>,
        text: &str,
    ) -> anyhow::Result<String> {
        let sql_query = match lang {
            None => r"SELECT CAST( to_tsvector(unaccent_lower(:p_doc_text)) as VARCHAR ) as tsv"
                .to_string(),
            Some(lg) => {
                format!(
                    r"SELECT CAST( to_tsvector('{}',  unaccent_lower(:p_doc_text)) as VARCHAR ) as tsv",
                    lg
                )
            }
        };

        let mut params = HashMap::new();
        params.insert(
            "p_doc_text".to_string(),
            CellValue::from_raw_string(text.to_string()),
        );
        let sql_block = SQLQueryBlockAsync {
            sql_query: sql_query.to_string(),
            start: 0,
            length: None,
            params,
        };

        let mut data = sql_block
            .execute(&mut trans)
            .await
            .map_err(err_fwd!("Error compute tsvector"))?;

        let tsv = if data.next() {
            data.get_string("tsv").unwrap_or("ERROR".to_string())
        } else {
            return Err(anyhow::anyhow!("Impossible to compute the tsvector"));
        };

        Ok(tsv)
    }
}
