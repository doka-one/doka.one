use std::cmp::min;
use std::collections::HashMap;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::{Duration, SystemTime};

use anyhow::anyhow;
use axum::body::Body;
use axum::extract::Multipart;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use base64::Engine;
use bytes::Bytes;

use futures::stream::Stream;
use futures::{future, TryFutureExt};
use log::*;
use mime::Mime;
use rs_uuid::iso::uuid_v4;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use tokio::task;

use commons_error::*;
use commons_pg::sql_transaction::{CellValue, SQLDataSet};
use commons_pg::sql_transaction_async::{SQLChangeAsync, SQLConnectionAsync, SQLQueryBlockAsync};
use commons_services::key_lib::fetch_customer_key;
use commons_services::session_lib::valid_sid_get_session;
use commons_services::token_lib::SessionToken;
use commons_services::try_or_return;
use commons_services::x_request_id::{Follower, XRequestID};
use dkconfig::properties::get_prop_value;
use dkconfig::property_name::{
    DOCUMENT_SERVER_HOSTNAME_PROPERTY, DOCUMENT_SERVER_PORT_PROPERTY, TIKA_SERVER_HOSTNAME_PROPERTY,
    TIKA_SERVER_PORT_PROPERTY,
};
use dkcrypto::dk_crypto::CypherMode::CC20;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::error_codes::{FILE_INFO_NOT_FOUND, INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR};
use dkdto::{
    DownloadReply, EntrySession, ErrorSet, GetFileInfoReply, GetFileInfoShortReply, ListOfFileInfoReply,
    ListOfUploadInfoReply, UploadInfoReply, UploadReply, WebType, WebTypeBuilder,
};
use doka_cli::async_request_client::{DocumentServerClientAsync, TikaServerClientAsync};
use doka_cli::request_client::TokenType;

// use tokio::stream;

const TIKA_CONTENT_META: &str = "X-TIKA:content";
const CONTENT_TYPE_META: &str = "Content-Type";

pub type IndexedParts = HashMap<u32, Vec<u8>>;

/// ---

// Une structure pour encapsuler le Stream personnalisé
struct PartsStream {
    parts: Vec<Vec<u8>>,   // Les différentes parties en mémoire
    current_index: usize,  // L'index de la partie actuellement traitée
    current_offset: usize, // Offset actuel dans la partie en cours
}

impl PartsStream {
    pub fn new(parts: IndexedParts) -> Self {
        // Trier les parties par clé et les convertir en un Vec
        let mut sorted_parts: Vec<_> = parts.into_iter().collect();
        sorted_parts.sort_by_key(|&(k, _)| k);

        // On récupère juste les Vec<u8> triés
        let sorted_data = sorted_parts.into_iter().map(|(_, data)| data).collect();

        PartsStream { parts: sorted_data, current_index: 0, current_offset: 0 }
    }
}

// Implémentation du Stream pour PartsStream
impl Stream for PartsStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Accéder aux champs via `Pin` pour obtenir un accès mutable
        let self_mut = self.as_mut().get_mut();

        // Si on a parcouru toutes les parties, on termine le Stream
        if self_mut.current_index >= self_mut.parts.len() {
            return Poll::Ready(None);
        }

        // Accéder à la partie en cours
        let current_part = &self_mut.parts[self_mut.current_index];
        let remaining = &current_part[self_mut.current_offset..];

        // Lire un chunk (ici, on lit tout le reste de la partie)
        let chunk_size = remaining.len();
        let chunk = Bytes::copy_from_slice(remaining);

        // Mettre à jour l'offset
        self_mut.current_offset += chunk_size;

        // Si nous avons fini cette partie, passer à la suivante
        if self_mut.current_offset >= current_part.len() {
            self_mut.current_index += 1;
            self_mut.current_offset = 0;
        }

        // Retourner le chunk sous forme de Poll
        Poll::Ready(Some(Ok(chunk)))
    }
}

/// ---

#[derive(Debug, Clone)]
pub(crate) struct FileDelegate {
    pub session_token: SessionToken,
    pub follower: Follower,
}

impl FileDelegate {
    const BLOCK_SIZE: usize = 1_048_576;

    pub fn new(session_token: SessionToken, x_request_id: XRequestID) -> Self {
        Self {
            session_token,
            follower: Follower { x_request_id: x_request_id.new_if_null(), token_type: TokenType::None },
        }
    }

    async fn read_and_write_incoming_data(
        &self,
        item_info: &str,
        file_ref: &str,
        file_data: &mut Multipart,
        entry_session: &EntrySession,
    ) -> anyhow::Result<(usize, u32)> {
        // Create parts
        log_info!("Start creating clear parts in the database, follower=[{}]", &self.follower);

        const INSERT_GROUP_SIZE: usize = 10;
        let mut block_set: HashMap<u32, Vec<u8>> = HashMap::with_capacity(INSERT_GROUP_SIZE);

        let mut total_size: usize = 0;
        let mut block_num: u32 = 0;

        loop {
            let mut field = match file_data.next_field().await {
                Ok(Some(field)) => field,
                Ok(None) => {
                    break; // No more fields to process
                }
                Err(e) => {
                    return Err(anyhow!(
                        "💣 Error reading multipart field, follower=[{}], error=[{}]",
                        &self.follower,
                        e
                    ));
                }
            };

            let file_name = field.file_name().unwrap_or("default_name").to_string();

            use bytes::Bytes;
            let mut buffer = Bytes::new();

            loop {
                let chunk = match field.chunk().await {
                    Ok(Some(chunk)) => chunk,
                    Ok(None) => break, // No more chunks in this field
                    Err(e) => {
                        return Err(anyhow!("💣 Error reading chunk, follower=[{}], error=[{}]", &self.follower, e));
                    }
                };

                buffer = Bytes::copy_from_slice(&[buffer, chunk].concat());
                // Tant que le buffer contient au moins BLOCK_SIZE octets, écrire un bloc
                while buffer.len() >= Self::BLOCK_SIZE {
                    let slice = buffer.slice(..Self::BLOCK_SIZE); // Prendre le bloc de taille fixe

                    block_set.insert(block_num, slice.to_vec());
                    block_num += 1;
                    total_size += slice.len();

                    log_info!("block num {}, size = {}", block_num, slice.len());

                    if block_set.len() >= INSERT_GROUP_SIZE {
                        if let Err(e) = self.store_group_block(item_info, file_ref, &block_set, entry_session).await {
                            return Err(anyhow!(
                                "💣 Cannot store the set of blocks, follower=[{}], error=[{}]",
                                &self.follower,
                                e
                            ));
                        }
                        block_set.clear();
                    }

                    // Rester les octets non écrits dans le buffer
                    buffer = buffer.slice(Self::BLOCK_SIZE..);
                }
            }

            // Si le buffer contient encore des données (moins que BLOCK_SIZE), les écrire
            if !buffer.is_empty() {
                // file.write_all(&buffer).await.unwrap();
                block_set.insert(block_num, buffer.to_vec());
                block_num += 1;
                total_size += buffer.len();

                log_info!("block num {}, size = {}", block_num, buffer.len());

                if let Err(e) = self.store_group_block(item_info, file_ref, &block_set, entry_session).await {
                    return Err(anyhow!(
                        "💣 Cannot store the set of blocks, follower=[{}], error=[{}]",
                        &self.follower,
                        e
                    ));
                }
                block_set.clear();
            }

            println!("File upload completed: {}", file_name);
        }

        //

        // let mut datastream = file_data.open();
        //
        // loop {
        //     match datastream.read(&mut buf[buf_pos..]) {
        //         Ok(0) => break,
        //         Ok(n) => {
        //             buf_pos += n;
        //             if buf_pos < Self::BLOCK_SIZE {
        //                 continue;
        //             }
        //
        //             let slice = &buf[..Self::BLOCK_SIZE];
        //             block_set.insert(block_num, slice.to_vec());
        //             block_num += 1;
        //
        //             if block_set.len() >= INSERT_GROUP_SIZE {
        //                 if let Err(e) = self
        //                     .store_group_block(item_info_str, file_ref, &block_set, entry_session)
        //                     .await
        //                 {
        //                     return Err(anyhow!(
        //                         "💣 Cannot store the set of blocks, follower=[{}], error=[{}]",
        //                         &self.follower,
        //                         e
        //                     ));
        //                 }
        //                 block_set.clear();
        //             }
        //
        //             total_size += Self::BLOCK_SIZE;
        //             buf_pos = 0;
        //
        //             // Store the bytes in a memory file
        //             // mem_file.extend_from_slice(slice);
        //         }
        //         Err(e) => {
        //             return Err(anyhow!(
        //                 "💣 Cannot read input data, follower=[{}], error=[{}]",
        //                 &self.follower,
        //                 e
        //             ));
        //         }
        //     }
        // }
        //
        // // Process the remaining part
        // if buf_pos > 0 {
        //     let slice = &buf[..buf_pos];
        //     block_set.insert(block_num, slice.to_vec());
        //
        //     if let Err(e) =
        //         self.store_group_block(item_info_str, file_ref, &block_set, entry_session)
        //     {
        //         return Err(anyhow!(
        //             "💣 Cannot store the last set of blocks, follower=[{}], error=[{}]",
        //             &self.follower,
        //             e
        //         ));
        //     }
        //
        //     total_size += buf_pos;
        //     block_num += 1;
        // }

        Ok((total_size, block_num))
    }

    /// Download the parts into a body stream
    async fn download_from_parts(
        parts: IndexedParts,
        media: &Mime,
    ) -> Result<(HeaderMap, Body), (axum::http::StatusCode, String)> {
        // Créer le stream à partir de IndexedParts
        let parts_stream = PartsStream::new(parts);

        // Créer un flux compatible avec Body
        //let body_stream = tokio_util::io::StreamReader::new(parts_stream);
        let body = Body::from_stream(parts_stream);

        // Ajouter les en-têtes HTTP
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_str(media.to_string().as_str()).unwrap());
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str("inline; filename=\"file_from_parts\"").unwrap(),
        );

        // Retourner le corps et les en-têtes
        Ok((headers, body))
    }

    /// Get all the encrypted parts of the file
    /// ( "application/pdf", {0 : "...", 1: "...", ...} )
    async fn search_incoming_blocks(&self, file_ref: &str, customer_code: &str) -> anyhow::Result<SQLDataSet> {
        log_info!("Search the incoming blocks for the file, file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

        let sql_str = r"
            SELECT
                fu.file_ref,
                fu.part_number,
                fu.part_data
            FROM  fs_{customer_code}.file_uploads fu
            WHERE
                fu.file_ref = :p_file_ref
            ORDER BY fu.file_ref, fu.part_number";

        let sql_query = sql_str.replace("{customer_code}", customer_code);
        let mut params = HashMap::new();
        params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.to_string()));

        let query = SQLQueryBlockAsync { sql_query, start: 0, length: None, params };

        let dataset =
            query.execute(&mut trans).await.map_err(err_fwd!("💣 Query failed, follower=[{}]", &self.follower))?;
        trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;
        log_info!("😎 Found incoming blocks for the file, file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        Ok(dataset)
    }

    async fn write_part(
        &self,
        file_id: i64,
        block_number: u32,
        enc_data: &str,
        customer_code: &str,
    ) -> anyhow::Result<()> {
        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

        let sql_query = format!(
            r"
                    INSERT INTO fs_{}.file_parts (file_reference_id, part_number, part_data)
                    VALUES (:p_file_reference_id, :p_part_number, :p_part_data)",
            customer_code
        );

        let sequence_name = format!("fs_{}.file_parts_id_seq", customer_code);

        let mut params = HashMap::new();
        params.insert("p_file_reference_id".to_string(), CellValue::from_raw_int(file_id));
        params.insert("p_part_number".to_string(), CellValue::from_raw_int_32(block_number as i32));
        params.insert("p_part_data".to_string(), CellValue::from_raw_str(enc_data));

        let sql_insert = SQLChangeAsync { sql_query, params, sequence_name };

        let _file_part_id =
            sql_insert.insert(&mut trans).await.map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;
        trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;
        log_info!("Encrypted block inserted as a part, block_num=[{}], follower=[{}]", block_number, &self.follower);
        Ok(())
    }

    ///
    ///
    ///
    async fn serial_encrypt(
        &self,
        file_id: i64,
        file_ref: &str,
        block_count: u32,
        customer_code: &str,
        customer_key: &str,
    ) -> anyhow::Result<()> {
        // Query the blocks from file_upload table

        let mut dataset = self.search_incoming_blocks(file_ref, customer_code).await.map_err(tr_fwd!())?;
        let mut row_index: u32 = 0;

        // Loop the blocks
        while dataset.next() {
            let block_number = dataset.get_int_32("part_number").ok_or(anyhow!("Wrong part_number col"))? as u32;
            let part_data = dataset.get_string("part_data").ok_or(anyhow!("Wrong part_data col"))?;

            // | Encrypt the data
            let raw_value = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(part_data).map_err(tr_fwd!())?;

            let encrypted_block = DkEncrypt::new(CC20)
                .encrypt_vec(&raw_value, &customer_key)
                .map_err(err_fwd!("Cannot encrypt the data block, follower=[{}]", &self.follower))?;

            // | Store the data in the file_parts

            let enc_data = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&encrypted_block);
            log_info!(
                "Encrypted the row number=[{}/{}], enc_parts=[{}], follower=[{}]",
                row_index,
                block_count - 1,
                &enc_data[..10],
                &self.follower
            );
            let _ = self.write_part(file_id, block_number, &enc_data, customer_code).await?;

            row_index += 1;
        }

        Ok(())
    }

    async fn process_file_blocks(
        &self,
        file_id: i64,
        file_ref: &str,
        _item_info_str: &str,
        block_count: u32,
        customer_code: &str,
        customer_key: &str,
    ) -> anyhow::Result<()> {
        log_info!("Process the blocks for file ref = [{}], follower=[{}]", &file_ref, &self.follower);
        // Read the file parts from the file_uploads table, encrypt the blocks and store the encrypted part into file_parts
        let _ = self.serial_encrypt(file_id, file_ref, block_count, customer_code, customer_key).await?;

        // Parse the file (Tika)
        let _r = self.serial_parse_content(file_id, &file_ref, block_count, customer_code).await?;
        log_info!(
            "😎 Successful process file for file_ref=[{}], file_id=[{}], follower=[{}]",
            file_ref,
            file_id,
            &self.follower
        );
        Ok(())
    }

    ///
    /// REF_TAG : FILE_UPLOAD
    ///
    /// The Upload is made of 2 phases :
    ///  1. Initial Phase : we read and write the blocks in the upload table.
    ///     In this table, we find the  session id, customer id, user id, item_info, file_reference, block_size, ...
    ///     This phase will maintain the session open as long as necessary
    ///  2. Processing Phase : Process the blocks for the file_ref, to encrypt, parse and so on.
    ///     2.b Clean all the data in the upload table for the file_ref.
    ///         Clean the data for the (customer/user), older than 4 days.
    ///
    pub async fn upload2(
        &mut self,
        item_info: &str,
        content_length: &Option<u64>,
        file_data: &mut Multipart,
    ) -> WebType<UploadReply> {
        // Pre-processing
        log_info!("🚀 Start upload api, item_info=[{}], follower=[{}]", &item_info, &self.follower);

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        let customer_code = entry_session.customer_code.as_str();

        // Get the crypto key

        let Ok(customer_key) = fetch_customer_key(customer_code, &self.follower)
            .await
            .map_err(err_fwd!("💣 Cannot get the customer key, follower=[{}]", &self.follower))
        else {
            return WebType::from_api_error(&INTERNAL_TECHNICAL_ERROR);
        };

        // Create an entry in file_reference
        let Ok((file_id, file_ref)) = self
            .create_file_reference(customer_code, content_length)
            .await
            .map_err(err_fwd!("💣 Cannot create an entry in the file reference table, follower=[{}]", &self.follower))
        else {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR);
        };

        log_info!(
            "😎 Created entry in file reference, file_id=[{}], file_ref=[{}], follower=[{}]",
            file_id,
            &file_ref,
            &self.follower
        );

        // Phase 1 :  Read all the incoming blocks and write them in the DB (file_uploads table)

        let Ok(item_info_decoded) = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(item_info)
            .map_err(err_fwd!("💣 Cannot decode item_info, follower=[{}]", &self.follower))
        else {
            return WebType::from_api_error(&INTERNAL_TECHNICAL_ERROR);
        };

        let item_info = String::from_utf8_lossy(&item_info_decoded);

        let Ok((total_size, block_count)) = self
            .read_and_write_incoming_data(&item_info, &file_ref, file_data, &entry_session)
            .await
            .map_err(err_fwd!("💣 Cannot write parts, follower=[{}]", &self.follower))
        else {
            // The stream is managed by the routine above, so no need to empty it here.
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR);
        };

        log_info!(
            "😎 Upload complete. About to process blocks, file_ref=[{}], follower=[{}]",
            &file_ref,
            &self.follower
        );

        // Phase 2 : Run a thread to perform all the other operations (encrypt, tika parse, ...)
        self.thread_processing_block(&item_info, file_id, &file_ref, customer_code, &customer_key, block_count).await;

        // Return the file_reference

        log_info!("🏁 End upload api, follower=[{}]", &self.follower);

        WebType::from_item(
            StatusCode::OK.as_u16(),
            UploadReply { file_ref: file_ref.clone(), size: total_size, block_count },
        )
    }

    async fn thread_processing_block(
        &self,
        item_info_str: &str,
        file_id: i64,
        file_ref: &str,
        customer_code: &str,
        customer_key: &str,
        block_count: u32,
    ) {
        let local_self = self.clone();
        let local_item_info_str = String::from(item_info_str);
        let local_file_ref = String::from(file_ref);
        let local_customer_code = String::from(customer_code);
        let local_customer_key = String::from(customer_key);
        let parallel_process = tokio::spawn(async move {
            log_info!(
                "Blocks processing is flying away for file_ref=[{}], follower=[{}]",
                &local_file_ref,
                &local_self.follower
            );

            let status = local_self
                .process_file_blocks(
                    file_id,
                    &local_file_ref,
                    &local_item_info_str,
                    block_count,
                    &local_customer_code,
                    &local_customer_key,
                )
                .await;
            if status.is_err() {
                log_error!(
                    "💣 The file processing failed. Enter the rollback process, file_ref=[{}], follower=[{}]",
                    &local_file_ref,
                    &local_self.follower
                );
                // Clean the tables : file_parts (file_id) + file_metadata (file_id)
                let _ = local_self.delete_from_target_table("file_parts", file_id, &local_customer_code).await;
                let _ = local_self.delete_from_target_table("file_metadata", file_id, &local_customer_code).await;
                // Change the status of file_reference (file_id) : put all the values to "0" (size + total_part)
                let _ = local_self.update_file_reference(file_id, 0, 0, "text", &local_customer_code).await;
                // Call the document server to delete the text indexing
                if let Ok(document_server) =
                    Self::find_document_server_client().map_err(err_fwd!("Cannot find the document server"))
                {
                    let _ = document_server
                        .delete_text_indexing(&local_file_ref, &local_self.follower.token_type.value())
                        .await;
                }
            }

            log_info!(
                "End of the block processing. Ready to delete upload parts, file_ref=[{}], follower=[{}]",
                &local_file_ref,
                &local_self.follower
            );

            // Clean file_uploads (file_ref)
            let _ = local_self.delete_from_file_uploads(&local_file_ref, &local_customer_code).await;
        });

        let _ = parallel_process.map_err(|e| {
            log_error!("💣 Error in the thread processing block, error=[{}], follower=[{}]", e, &self.follower);
            e
        });
    }

    // Windows only
    // fn empty_datastream(&self, reader: &mut dyn Read) {
    //     // BUG https://github.com/SergioBenitez/Rocket/issues/892
    //     log_warn!("⛔ Running on Windows, need to read the datastream");
    //     let _r = io::copy(reader, &mut io::sink());
    // }

    ///
    /// Run a thread to process the block and store it in the DB
    ///
    // fn _parallel_crypto_and_store_block(
    //     &self,
    //     file_id: i64,
    //     block_set: &HashMap<u32, Vec<u8>>,
    //     customer_code: &str,
    //     customer_key: &str,
    // ) -> JoinHandle<anyhow::Result<()>> {
    //     let s_customer_code = customer_code.to_owned();
    //     let s_customer_key = customer_key.to_owned();
    //     let local_block_set = block_set
    //         .into_iter()
    //         .map(|(key, value)| (*key, (*value).to_owned()))
    //         .collect();
    //
    //     let local_self = self.clone();
    //
    //     let th = thread::spawn(move || {
    //         local_self._crypto_and_store_block(
    //             file_id,
    //             local_block_set,
    //             s_customer_code,
    //             s_customer_key,
    //         )
    //     });
    //
    //     th
    // }

    async fn serial_parse_content(
        &self,
        file_id: i64,
        file_ref: &str,
        block_count: u32,
        customer_code: &str,
    ) -> anyhow::Result<()> {
        // Build the file in memory
        let mut mem_file: Vec<u8> = vec![];
        let mut dataset =
            self.search_incoming_blocks(/*&mut trans,*/ file_ref, customer_code).await.map_err(tr_fwd!())?;

        // Loop the blocks
        while dataset.next() {
            let part_data = dataset.get_string("part_data").ok_or(anyhow!("Wrong part_data col"))?;

            // | Encrypt the data
            let raw_value = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(part_data).map_err(tr_fwd!())?;

            mem_file.extend(&raw_value);
        }

        let total_size = mem_file.len();
        // Read the metadata and the raw text of the file
        let media_type = self.analyse_entire_content(&file_ref, mem_file, &customer_code).await.map_err(tr_fwd!())?;
        // Update the file_reference table : checksum, original_file_size, total_part, media_type
        let _ = self
            .update_file_reference(file_id, total_size, block_count, &media_type, customer_code)
            .await
            .map_err(tr_fwd!())?;
        Ok(())
    }

    fn min_max<T>(map: &HashMap<u32, T>) -> (u32, u32) {
        let mut min: u32 = u32::MAX;
        let mut max: u32 = u32::MIN;

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

    /// Store the encrypted file's blocks in the database.
    async fn store_group_block(
        &self,
        item_info_str: &str,
        file_ref: &str,
        block_set: &HashMap<u32, Vec<u8>>,
        entry_session: &EntrySession,
    ) -> anyhow::Result<()> {
        // Open the transaction
        let block_range = Self::min_max(&block_set);

        log_info!("Block range processing, block range=[{:?}], follower=[{}]", &block_range, &self.follower);

        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

        for (block_num, block) in block_set {
            log_debug!("Block processing... : block_num=[{}], follower=[{}]", block_num, &self.follower);

            let original_part_size = block.len();
            // Store in the DB
            let data = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&block);
            let sql_query = format!(
                r"
            INSERT INTO fs_{}.file_uploads (session_id, start_time_gmt,
                                      user_id, item_info, file_ref, part_number, original_part_size, part_data)
            VALUES (:p_session_id, :p_start_time_gmt,
                    :p_user_id, :p_item_info, :p_file_ref, :p_part_number, :p_original_part_size, :p_part_data)",
                &entry_session.customer_code
            );

            let mut params = HashMap::new();

            params.insert("p_session_id".to_string(), CellValue::from_raw_str(&self.follower.token_type.value()));
            params.insert("p_start_time_gmt".to_string(), CellValue::from_raw_systemtime(SystemTime::now()));
            params.insert("p_user_id".to_string(), CellValue::from_raw_int(entry_session.user_id));
            params.insert("p_item_info".to_string(), CellValue::from_raw_str(item_info_str));
            params.insert("p_file_ref".to_string(), CellValue::from_raw_str(file_ref));
            params.insert("p_part_number".to_string(), CellValue::from_raw_int_32(*block_num as i32));
            params.insert("p_original_part_size".to_string(), CellValue::from_raw_int(original_part_size as i64));
            params.insert("p_part_data".to_string(), CellValue::from_raw_string(data));

            let sql_insert = SQLChangeAsync { sql_query, params, sequence_name: "".to_uppercase() };

            let _ = sql_insert
                .insert_no_pk(&mut trans)
                .await
                .map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;
            log_debug!("...Block inserted, block_num=[{}], follower=[{}]", block_num, &self.follower);
        }

        // End of the 'pink' multiple transaction
        trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;
        log_info!("😎 Committed. Block inserted, block_range=[{:?}], follower=[{}]", &block_range, &self.follower);

        // Slow down the process for 2 seconds
        // tokio::time::sleep(Duration::from_secs(5)).await;

        Ok(())
    }

    //
    // fn _crypto_and_store_block(
    //     &self,
    //     file_id: i64,
    //     block_set: HashMap<u32, Vec<u8>>,
    //     customer_code: String,
    //     customer_key: String,
    // ) -> anyhow::Result<()> {
    //     // Open the transaction
    //     let block_range = Self::min_max(&block_set);
    //
    //     log_info!(
    //         "Block range processing, block range=[{:?}], follower=[{}]",
    //         &block_range,
    //         &self.follower
    //     );
    //
    //     let mut r_cnx = SQLConnection2::from_pool().await;
    //     let mut trans = open_transaction(&mut r_cnx).map_err(err_fwd!(
    //         "Open transaction error, block_range=[{:?}], follower=[{}]",
    //         &block_range,
    //         &self.follower
    //     ))?;
    //
    //     for (block_num, block) in block_set {
    //         log_debug!(
    //             "Block processing... : block_num=[{}], follower=[{}]",
    //             block_num,
    //             &self.follower
    //         );
    //
    //         // Encrypt the block
    //         let encrypted_block =
    //             DkEncrypt::new(CC20).encrypt_vec(&block, &customer_key).map_err(err_fwd!(
    //                 "Cannot encrypt the data block, follower=[{}]",
    //                 &self.follower
    //             ))?;
    //
    //         // and store in the DB
    //
    //         let data = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(encrypted_block);
    //         // let data = encrypted_block.to_base64(URL_SAFE);
    //
    //         let sql_query = format!(
    //             r"
    //                 INSERT INTO fs_{}.file_parts (file_reference_id, part_number, part_data)
    //                 VALUES (:p_file_reference_id, :p_part_number, :p_part_data)",
    //             customer_code
    //         );
    //
    //         let sequence_name = format!("fs_{}.file_parts_id_seq", customer_code);
    //
    //         let mut params = HashMap::new();
    //         params.insert(
    //             "p_file_reference_id".to_string(),
    //             CellValue::from_raw_int(file_id),
    //         );
    //         params.insert(
    //             "p_part_number".to_string(),
    //             CellValue::from_raw_int_32(block_num as i32),
    //         );
    //         params.insert("p_part_data".to_string(), CellValue::from_raw_string(data));
    //
    //         let sql_insert = SQLChange {
    //             sql_query,
    //             params,
    //             sequence_name,
    //         };
    //
    //         let _file_part_id = sql_insert
    //             .insert(&mut trans)
    //             .map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;
    //
    //         log_debug!(
    //             "...Block inserted, block_num=[{}], follower=[{}]",
    //             block_num,
    //             &self.follower
    //         );
    //     }
    //
    //     trans
    //         .commit()
    //         .map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;
    //
    //     log_info!(
    //         "😎 Committed. Block inserted, block_range=[{:?}], follower=[{}]",
    //         &block_range,
    //         &self.follower
    //     );
    //
    //     Ok(())
    // }

    fn find_document_server_client() -> anyhow::Result<DocumentServerClientAsync> {
        let document_server_host = get_prop_value(DOCUMENT_SERVER_HOSTNAME_PROPERTY)?;
        let document_server_port = get_prop_value(DOCUMENT_SERVER_PORT_PROPERTY)?.parse::<u16>()?;
        Ok(DocumentServerClientAsync::new(&document_server_host, document_server_port))
    }

    /// Call the tika server to parse the file and get the text data
    /// Insert the metadata
    /// Call the document server to fulltext parse the text data
    /// return the media type
    async fn analyse_entire_content(
        &self,
        file_ref: &str,
        mem_file: Vec<u8>,
        customer_code: &str,
    ) -> anyhow::Result<String> {
        log_info!("Parsing file content ... ,file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        let tika_server_host = get_prop_value(TIKA_SERVER_HOSTNAME_PROPERTY)?;
        let tika_server_port = get_prop_value(TIKA_SERVER_PORT_PROPERTY)?.parse::<u16>()?;

        // Get the raw text from the original file
        let tsc = TikaServerClientAsync::new(&tika_server_host, tika_server_port);
        let raw_json = tsc.parse_data_json(&mem_file).await.map_err(err_fwd!("Cannot parse the original file"))?;
        let x_tika_content = raw_json[TIKA_CONTENT_META].as_str().ok_or(anyhow!("Bad tika content"))?;
        let content_type = raw_json[CONTENT_TYPE_META].as_str().ok_or(anyhow!("Bad content type"))?;

        let metadata = match raw_json.as_object() {
            Some(obj) => obj.clone(),
            None => {
                return Err(anyhow!("Bad metadata"));
            }
        };

        log_info!(
            "Parsing done for file_ref=[{}], content size=[{}], content type=[{}], follower=[{}]",
            file_ref,
            x_tika_content.len(),
            &content_type,
            &self.follower
        );

        let _ = self.insert_metadata(&customer_code, file_ref, &metadata).await?;

        // TODO TikaParsing can contain all the metadata, so keep them and return then instead of getting only the content-type.
        log_info!("Metadata done for file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        let document_server =
            Self::find_document_server_client().map_err(err_fwd!("Cannot find the document server"))?;

        // TODO we must also pass the  self.follower.x_request_id + handle the file name
        let wr_reply = document_server
            .fulltext_indexing(&x_tika_content, "no_filename_for_now", file_ref, &self.follower.token_type.value())
            .await;
        match wr_reply {
            Ok(reply) => {
                log_info!(
                    "Fulltext indexing done, number of text parts=[{}], follower=[{}]",
                    reply.part_count,
                    &self.follower
                );
                self.set_file_reference_fulltext_indicator(file_ref, customer_code).await.map_err(err_fwd!(
                    "Cannot set the file reference to fulltext parsed indicator, follower=[{}]",
                    &self.follower
                ))?;
            }
            Err(e) => {
                log_error!("Error while sending the raw text to the fulltext indexing, file_ref=[{}], reply=[{:?}], follower=[{}], ",  file_ref, e, &self.follower);
                return Err(anyhow::anyhow!(e.message));
            }
        }

        log_info!("... End of parse file content processing, file_ref=[{}], follower=[{}]", file_ref, &self.follower);
        Ok(content_type.to_owned())
    }

    async fn insert_metadata(
        &self,
        customer_code: &str,
        file_ref: &str,
        metadata: &Map<String, Value>,
    ) -> anyhow::Result<()> {
        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

        let sql_query = format!(
            r"INSERT INTO fs_{0}.file_metadata ( file_reference_id, meta_key,  value )
        VALUES ((SELECT id FROM fs_{0}.file_reference WHERE file_ref = :p_file_ref), :p_meta_key, :p_value)",
            customer_code
        );
        let sequence_name = format!("fs_{}.file_metadata_id_seq", customer_code);

        // TODO Could be done with a specific batch insert sql routine that will build a big insert statement!
        for (key, value) in metadata.iter() {
            if key != TIKA_CONTENT_META && value.to_string().len() < 200 {
                let mut params: HashMap<String, CellValue> = HashMap::new();
                params.insert("p_file_ref".to_owned(), CellValue::from_raw_str(file_ref));
                params.insert("p_meta_key".to_owned(), CellValue::from_raw_str(key));
                params.insert("p_value".to_owned(), CellValue::from_raw_string(value.to_string()));

                let sql_insert =
                    SQLChangeAsync { sql_query: sql_query.clone(), params, sequence_name: sequence_name.clone() };
                let meta_id = sql_insert.insert(&mut trans).await.map_err(err_fwd!(
                    "💣 Cannot insert the metadata, file_ref=[{}], key=[{}], value=[{}], follower=[{}]",
                    file_ref,
                    key,
                    value.to_string(),
                    &self.follower
                ))?;

                log_info!(
                    "Success inserting meta meta_id=[{}], file_ref=[{}], key=[{}], value=[{}], follower=[{}]",
                    meta_id,
                    file_ref,
                    key,
                    value.to_string(),
                    &self.follower
                );
            }
        }

        trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        Ok(())
    }

    //
    async fn set_file_reference_fulltext_indicator(&self, file_ref: &str, customer_code: &str) -> anyhow::Result<()> {
        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

        let sql_query = format!(
            r"UPDATE fs_{}.file_reference
                SET is_fulltext_parsed = true
                WHERE file_ref = :p_file_ref ",
            customer_code
        );

        let sequence_name = format!("fs_{}.file_reference_id_seq", customer_code);

        let mut params = HashMap::new();
        params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.to_string()));

        let sql_update = SQLChangeAsync { sql_query, params, sequence_name };

        let _ =
            sql_update.update(&mut trans).await.map_err(err_fwd!("Update failed, follower=[{}]", &self.follower))?;

        trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        log_info!(
            "😎 Committed. Successfully set the full text indicator, file_ref=[{:?}], follower=[{}]",
            file_ref,
            &self.follower
        );

        Ok(())
    }

    fn web_type_error<T>() -> impl Fn(&ErrorSet<'static>) -> WebType<T>
    where
        T: DeserializeOwned,
    {
        |e| {
            log_error!("💣 Error after try {:?}", e);
            WebType::from_api_error(e)
        }
    }

    ///
    /// 🌟 Get the information about the composition of a file [file_ref]
    ///
    pub async fn file_info(&mut self, file_ref: &str) -> WebType<Option<GetFileInfoReply>> {
        log_info!("🚀 Start file_info api, follower=[{}]", &self.follower);
        // Check if the token is valid

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        let customer_code = entry_session.customer_code.as_str();

        let mut files =
            try_or_return!(self.fetch_files_information(file_ref, &customer_code).await, Self::web_type_error());

        let web_type = if !files.list_of_files.is_empty() {
            let item = files.list_of_files.remove(0);
            WebType::from_item(StatusCode::OK.as_u16(), Some(item))
        } else {
            WebType::from_item(StatusCode::OK.as_u16(), None)
        };

        log_info!("🏁 End file_info api, follower=[{}]", &self.follower);
        web_type
    }

    /// 🌟 Find the files in the system
    pub async fn file_list(&mut self, match_expression: &str) -> WebType<ListOfFileInfoReply> {
        log_info!("🚀 Start file_list api, follower=[{}]", &self.follower);

        let entry_session = try_or_return!(valid_sid_get_session(&self.session_token, &mut self.follower).await, |e| {
            WebType::from_api_error(e)
        });

        log_info!(
            "😎 We read the session information, customer_code=[{}], user_id=[{}], follower=[{}]",
            &entry_session.customer_code,
            &entry_session.user_id,
            &self.follower
        );

        let r_files = self.fetch_files_information(&match_expression, &entry_session.customer_code).await;

        log_info!("🏁 End file_list api, follower=[{}]", &self.follower);

        match r_files {
            Ok(files) => WebType::from_item(StatusCode::OK.as_u16(), files),
            Err(e) => WebType::from_api_error(e),
        }
    }

    fn is_valid_pattern(s: &str) -> bool {
        s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '*')
    }

    ///
    async fn query_file_reference(
        customer_code: &str,
        sql_pattern: &str,
        follower: &Follower,
    ) -> anyhow::Result<SQLDataSet> {
        let sql_query = format!(
            r"SELECT
                    fr.file_ref,
                    fr.mime_type,
                    fr.checksum,
                    fr.total_part,
                    fr.original_file_size,
                    fr.encrypted_file_size,
                    fr.is_encrypted,
                    fr.is_fulltext_parsed,
                    fr.is_preview_generated
                FROM  fs_{0}.file_reference fr
                WHERE
                    fr.file_ref like :p_file_reference",
            customer_code
        );

        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;
        let mut params = HashMap::new();
        params.insert("p_file_reference".to_string(), CellValue::from_raw_str(sql_pattern));

        let query = SQLQueryBlockAsync { sql_query: sql_query.to_string(), start: 0, length: None, params };
        let dataset = query.execute(&mut trans).await?;
        trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &follower))?;
        Ok(dataset)
    }

    /// Inner function
    async fn build_file_reference_block_info(data_set: &mut SQLDataSet) -> anyhow::Result<GetFileInfoReply> {
        let file_ref = data_set.get_string("file_ref").ok_or(anyhow!("Wrong file_ref"))?;
        let media_type = data_set.get_string("mime_type");
        let checksum = data_set.get_string("checksum");
        let original_file_size = data_set.get_int("original_file_size");
        let encrypted_file_size = data_set.get_int("encrypted_file_size");
        let total_part = data_set.get_int_32("total_part");
        // let total_part = None;
        let is_encrypted = data_set.get_bool("is_encrypted").ok_or(anyhow!("Wrong is_encrypted col"))?;
        let is_fulltext_parsed = data_set.get_bool("is_fulltext_parsed");
        let is_preview_generated = data_set.get_bool("is_preview_generated");

        Ok(GetFileInfoReply {
            file_ref,
            media_type,
            checksum,
            original_file_size,
            encrypted_file_size,
            block_count: total_part,
            is_encrypted,
            is_fulltext_parsed,
            is_preview_generated,
        })
    }

    /// Query the information related to the existing files whose the reference matches the given pattern
    async fn fetch_files_information(
        &self,
        pattern: &str,
        customer_code: &str,
    ) -> Result<ListOfFileInfoReply, &ErrorSet<'static>> {
        if !Self::is_valid_pattern(&pattern) {
            return Err(&FILE_INFO_NOT_FOUND);
        }

        let sql_pattern = pattern.replace('*', "%");

        let Ok(mut data_set) = Self::query_file_reference(&customer_code, &sql_pattern, &self.follower).await else {
            return Err(&INTERNAL_DATABASE_ERROR);
        };

        let mut files = ListOfFileInfoReply { list_of_files: vec![] };

        while data_set.next() {
            match Self::build_file_reference_block_info(&mut data_set).await {
                Ok(block_info) => files.list_of_files.push(block_info),
                Err(e) => {
                    log_error!("💣 Error while building file info, e=[{}], follower=[{}]", e, &self.follower);
                    return Err(&INTERNAL_DATABASE_ERROR);
                }
            }
        }
        Ok(files)
    }

    /// 🌟 Get the information about the files being loaded
    ///
    /// Get all the upload information. Only the session id is required to identify the (customer id/user id)
    ///
    /// All the current uploads will be extracted for the current user and a then a list of information will be returned
    ///
    /// * start_date_time :
    /// * item_info :  Is a non unique string to make link with the item element during the initial phase of upload (ex: the file name)
    /// * file_reference :
    /// * session_number :
    pub async fn file_loading(&mut self) -> WebType<ListOfUploadInfoReply> {
        log_info!("🚀 Start file_loading api, follower=[{}]", &self.follower);

        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );

        log_info!(
            "😎 We read the session information, customer_code=[{}], user_id=[{}], follower=[{}]",
            &entry_session.customer_code,
            &entry_session.user_id,
            &self.follower
        );

        let sql_query = format!(
            r"SELECT current_uploads.file_ref,
                            current_uploads.session_id,
                            current_uploads.start_time,
                            current_uploads.item_info,
                            current_uploads.count_uploaded,
                            (SELECT total_part FROM fs_{0}.file_reference
                                               WHERE file_ref = current_uploads.file_ref
                            ) total_part,
                            (SELECT count(*)
                                FROM  fs_{0}.file_parts
                                WHERE file_reference_id = (SELECT id FROM fs_{0}.file_reference
                                                                    WHERE file_ref = current_uploads.file_ref)
                                ) count_encrypted
                        FROM
                            (SELECT file_ref, session_id,
                                     MIN(start_time_gmt) start_time,
                                     user_id, item_info ,
                                     COUNT(*) count_uploaded
                             FROM fs_{0}.file_uploads
                             WHERE user_id = :p_user_id
                             GROUP BY file_ref, session_id, user_id, item_info ) current_uploads",
            entry_session.customer_code
        );

        /// Inner function
        async fn execute_query(
            entry_session: &EntrySession,
            sql_query: String,
            follower: &Follower,
        ) -> anyhow::Result<SQLDataSet> {
            let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
            let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

            let mut params = HashMap::new();
            params.insert("p_user_id".to_string(), CellValue::from_raw_int(entry_session.user_id));

            let query = SQLQueryBlockAsync { sql_query, start: 0, length: None, params };

            let dataset = query.execute(&mut trans).await?;
            trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", follower))?;

            Ok(dataset)
        }

        let Ok(mut data_set) = execute_query(&entry_session, sql_query, &self.follower).await else {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR);
        };

        // Inner function
        async fn build_loading_info_item(data_set: &mut SQLDataSet) -> anyhow::Result<UploadInfoReply> {
            let file_reference = data_set.get_string("file_ref").ok_or(anyhow!("Wrong file_ref col"))?;
            let session_number = data_set.get_string("session_id").ok_or(anyhow!("Wrong session_id col"))?;
            let item_info = data_set.get_string("item_info").ok_or(anyhow!("Wrong item_info col"))?;
            let start_date_time =
                data_set.get_timestamp_as_datetime("start_time").ok_or(anyhow!("Wrong start_time col"))?;
            let total_part = data_set.get_int_32("total_part").ok_or(anyhow!("Wrong total_part col"))?;
            let encrypted_count = data_set.get_int("count_encrypted").ok_or(anyhow!("Wrong count_encrypted col"))?;
            let uploaded_count = data_set.get_int("count_uploaded").ok_or(anyhow!("Wrong count_uploaded col"))?;

            let limit = min(session_number.len() - 2, 22);
            Ok(UploadInfoReply {
                start_date_time,
                item_info,
                file_reference,
                session_number: format!("{}...", session_number[..limit].to_owned()),
                encrypted_count,
                uploaded_count,
                total_part: total_part as i64,
            })
        }

        let mut list_of_upload_info: Vec<UploadInfoReply> = vec![];
        while data_set.next() {
            let Ok(loading_info_item) = build_loading_info_item(&mut data_set)
                .await
                .map_err(err_fwd!("Build loading info item failed, follower=[{}]", &self.follower))
            else {
                return WebType::from_api_error(&INTERNAL_DATABASE_ERROR);
            };
            list_of_upload_info.push(loading_info_item);
        }

        log_info!("😎 Successfully read the loading info, follower=[{}]", &self.follower);

        let upload_info = ListOfUploadInfoReply { list_of_upload_info };

        log_info!("🏁 End file_loading api, follower=[{}]", &self.follower);
        WebType::from_item(StatusCode::OK.as_u16(), upload_info)
    }

    ///
    /// 🌟 Get the information about the loading status of the [file_ref]
    ///
    pub async fn file_stats(&mut self, file_ref: &str) -> WebType<GetFileInfoShortReply> {
        log_info!("🚀 Start file_stats api, file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        // Check if the token is valid
        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::web_type_error()
        );
        let customer_code = entry_session.customer_code.as_str();

        // TODO instead of constant 1, check if the document is fulltext parsed and previewed
        // The blocks in file_parts are encrypted, so the number of bloccks is the same as the number of encrypted blocks
        let sql_query = format!(
            r" SELECT
                fr.mime_type, fr.checksum, fr.original_file_size, fr.total_part, 1 fulltext,  1 preview,
                (SELECT count(*)
                FROM  fs_{0}.file_parts
                WHERE file_reference_id = (SELECT id FROM fs_{0}.file_reference WHERE file_ref = :p_file_ref)
                ) parts_count,

                (SELECT  count(*) from fs_{0}.file_uploads WHERE file_ref = :p_file_ref) count_uploaded
            FROM fs_{0}.file_reference fr
            WHERE file_ref = :p_file_ref",
            customer_code
        );

        /// Inner function
        async fn local_execute_query(
            sql_query: &str,
            file_ref: &str,
            follower: &Follower,
        ) -> anyhow::Result<SQLDataSet> {
            let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
            let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

            let mut params = HashMap::new();
            params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.to_string()));

            let query = SQLQueryBlockAsync { sql_query: sql_query.to_string(), start: 0, length: None, params };

            let dataset = query.execute(&mut trans).await?;
            trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &follower))?;

            Ok(dataset)
        }

        let Ok(mut data_set) = local_execute_query(&sql_query, &file_ref, &self.follower).await else {
            return WebType::from_api_error(&INTERNAL_DATABASE_ERROR);
        };

        // Inner function
        async fn build_file_info(data_set: &mut SQLDataSet, file_ref: &str) -> anyhow::Result<GetFileInfoShortReply> {
            let _mime_type = data_set.get_string("mime_type").unwrap_or("".to_string()); // optional
            let _checksum = data_set.get_string("checksum").unwrap_or("".to_string()); // optional
            let original_file_size = data_set.get_int("original_file_size")/*.ok_or(anyhow!("Wrong original_file_size col"))?*/;
            let total_part = data_set.get_int_32("total_part").ok_or(anyhow!("Wrong total_part col"))?;

            let parts_count = data_set.get_int("parts_count").ok_or(anyhow!("Wrong count_encrypted col"))?;
            let uploaded_count = data_set.get_int("count_uploaded").ok_or(anyhow!("Wrong count_uploaded col"))?;
            let fulltext_indexed_count = data_set.get_int_32("fulltext").ok_or(anyhow!("Wrong fulltext col"))?;
            let preview_generated_count = data_set.get_int_32("preview").ok_or(anyhow!("Wrong preview col"))?;

            Ok(GetFileInfoShortReply {
                file_ref: file_ref.to_string(),
                block_count: total_part as u32,
                original_file_size: original_file_size.unwrap_or(0i64) as u64,
                encrypted_count: parts_count,
                uploaded_count,
                fulltext_indexed_count: fulltext_indexed_count as i64,
                preview_generated_count: preview_generated_count as i64,
            })
        }

        let wt_stats = if data_set.next() {
            let Ok(stats) = build_file_info(&mut data_set, file_ref)
                .await
                .map_err(err_fwd!("Build file info failed, follower=[{}]", &self.follower))
            else {
                return WebType::from_api_error(&INTERNAL_DATABASE_ERROR);
            };

            log_info!("😎 Successfully read the file stats, file_ref=[{}], follower=[{}]", file_ref, &self.follower);
            WebType::from_item(StatusCode::OK.as_u16(), stats)
        } else {
            log_info!("⛔ Cannot find the file stats, file_ref=[{}], follower=[{}]", file_ref, &self.follower);
            WebType::from_api_error(&INTERNAL_TECHNICAL_ERROR)
        };

        log_info!("🏁 End file_stats api, follower=[{}]", &self.follower);
        wt_stats
    }

    fn download_reply_error() -> impl Fn(&ErrorSet<'static>) -> DownloadReply {
        |e| {
            log_error!("💣 Error after try {:?}", e);
            DownloadReply::from_errorset(e)
        }
    }

    /// 🌟 Download the binary content of a file
    pub async fn download(&mut self, file_ref: &str) -> DownloadReply {
        log_info!("🚀 Start download api, file_ref = [{}], follower=[{}]", file_ref, &self.follower);

        // Check if the token is valid
        let entry_session = try_or_return!(
            valid_sid_get_session(&self.session_token, &mut self.follower).await,
            Self::download_reply_error()
        );

        let customer_code = entry_session.customer_code.as_str();
        log_info!("Found session and customer code=[{}], follower=[{}]", &customer_code, &self.follower);

        // Search the document's parts from the database

        let Ok((media_type, enc_parts)) = self.search_parts(file_ref, customer_code).await.map_err(tr_fwd!()) else {
            log_error!("");
            return DownloadReply::from_errorset(&INTERNAL_DATABASE_ERROR);
        };

        log_info!(
            "😎 Found the encrypted parts, number of parts=[{}], follower=[{}]",
            &enc_parts.len(),
            &self.follower
        );

        let Ok(media) = media_type.parse::<Mime>().map_err(tr_fwd!()) else {
            return DownloadReply::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };

        log_info!("😎 Found correct media type=[{}], follower=[{}]", &media, &self.follower);

        // Get the customer key
        let Ok(customer_key) = fetch_customer_key(customer_code, &self.follower)
            .await
            .map_err(err_fwd!("💣 Cannot get the customer key, follower=[{}]", &self.follower))
        else {
            return DownloadReply::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };

        // Parallel decrypt of slides of parts [Parts, Q+(1*)]

        let Ok(clear_parts) = self.parallel_decrypt(enc_parts, &customer_key).await else {
            log_error!(
                "💣 Cannot decrypt the parts for the file, file reference=[{}], follower=[{}]",
                &file_ref,
                &self.follower
            );
            return DownloadReply::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };

        // Output : Get a file array of P parts

        log_info!("😎 Decrypt done, number of parts=[{}], follower=[{}]", &clear_parts.len(), &self.follower);

        // Merge all the parts in one big file
        let Ok(stream) = Self::download_from_parts(clear_parts, &media).await else {
            log_error!(
                "💣 Cannot merge the parts for the file, file reference=[{}], follower=[{}]",
                &file_ref,
                &self.follower
            );
            return DownloadReply::from_errorset(&INTERNAL_TECHNICAL_ERROR);
        };

        log_info!("😎 Merged all the parts, follower=[{}]", &self.follower);
        log_info!("🏁 End download api, follower=[{}]", &self.follower);

        Ok(stream)
    }

    /// Get all the encrypted parts of the file
    /// ( "application/pdf", {0 : "...", 1: "...", ...} )
    async fn search_parts(
        &self,
        file_ref: &str,
        customer_code: &str,
    ) -> anyhow::Result<(String, HashMap<u32, String>)> {
        log_info!("Search the parts for the file, file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        let sql_str = r"
            SELECT fp.id,
                fr.file_ref,
                fr.mime_type,
                fr.is_encrypted,
                fp.part_number,
                length(fp.part_data) as part_data_length,
                fp.part_data
            FROM  fs_{customer_code}.file_reference fr, fs_{customer_code}.file_parts fp
            WHERE
                fp.file_reference_id = fr.id AND
                fr.file_ref = :p_file_ref
            ORDER BY fr.file_ref, fp.part_number";

        let sql_query = sql_str.replace("{customer_code}", customer_code);

        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

        let mut params = HashMap::new();
        params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.to_string()));

        let query = SQLQueryBlockAsync { sql_query, start: 0, length: None, params };

        let mut dataset =
            query.execute(&mut trans).await.map_err(err_fwd!("💣 Query failed, follower=[{}]", &self.follower))?;

        let mut parts: HashMap<u32, String> = HashMap::new();
        let mut media_type = String::new();
        while dataset.next() {
            let part_info_len = dataset.get_int_32("part_data_length").ok_or(anyhow!("Wrong part_data_length col"))?;
            let part_info = Self::read_part(&mut dataset)
                .map_err(err_fwd!("Cannot read part data, follower=[{}]", &self.follower))?;
            media_type = part_info.0; // always the same media type for each row
            parts.insert(part_info.1, part_info.2);
        }

        log_info!(
            "😎 Found parts for the file, file_ref=[{}], n_parts=[{}], follower=[{}]",
            file_ref,
            parts.len(),
            &self.follower
        );

        Ok((media_type, parts))
    }

    /// ( <mdeia_type>, <part_number>, <data> )
    fn read_part(data_set: &mut SQLDataSet) -> anyhow::Result<(String, u32, String)> {
        let media_type = data_set.get_string("mime_type").ok_or(anyhow!("Wrong mime_type col"))?;
        let is_encrypted = data_set.get_bool("is_encrypted").ok_or(anyhow!("Wrong is_encrypted col"))?;
        let part_number = data_set.get_int_32("part_number").ok_or(anyhow!("Wrong part_number col"))?;
        let part_data = data_set.get_string("part_data").ok_or(anyhow!("Wrong part_data col"))?;

        if !is_encrypted {
            return Err(anyhow!("Part is not encrypted, part number=[{}]", part_number));
        }

        Ok((media_type, part_number as u32, part_data))
    }

    //
    // fn merge_parts(&self, clear_parts_slides: &IndexedParts) -> anyhow::Result<Vec<u8>> {
    //     let mut bytes = vec![];
    //     //let mut part_index: u32 = 0;
    //     for i in 0..clear_parts_slides.len() {
    //         log_info!(
    //             "Join part, part number=[{}], follower=[{}]",
    //             i,
    //             &self.follower
    //         );
    //         let index = i as u32;
    //         let parts = clear_parts_slides
    //             .get(&index)
    //             .ok_or(anyhow!("Wrong index"))
    //             .map_err(tr_fwd!())?;
    //         for b in parts {
    //             bytes.push(*b);
    //         }
    //         //     part_index +=1;
    //     }
    //     Ok(bytes)
    // }

    // N = Number of threads = Number of Cores - 1;
    // 5 cores , 20 parts => 4 decrypt by core
    // 5 cores, 22 parts => 5 5 4 4 4
    // 22 eucl 5 = 4,2 => 2 (number of extra decrypts)
    // P eucl N = [Q,R]  Q is the number of decrypts by thread and R is the number of thread with 1 extra decrypt.
    fn compute_pool_size(number_of_threads: u32, number_of_parts: u32) -> Vec<u32> {
        let mut pool_size = vec![];
        let q = number_of_parts / number_of_threads;
        let mut r = number_of_parts % number_of_threads;

        // dbg!(number_of_parts, number_of_threads, q,r);
        for _ in 0..number_of_threads {
            let extra = if r > 0 {
                r -= 1;
                1
            } else {
                0
            };
            pool_size.push(q + extra);
        }
        pool_size
    }

    /// Decypher the file parts in parallel
    // fn parallel_decrypt(
    //     &self,
    //     enc_parts: HashMap<u32, String>,
    //     customer_key: &str,
    // ) -> anyhow::Result<IndexedParts> {
    //     let mut thread_pool = vec![];
    //     let n_threads = max(1, num_cpus::get() - 1); // Number of threads is number of cores - 1
    //
    //     log_debug!(
    //         "Number of threads=[{}], follower=[{}]",
    //         n_threads,
    //         &self.follower
    //     );
    //
    //     let number_of_parts = enc_parts.len();
    //     // For n_threads = 5 and num of part = 22 , we get (5,5,4,4,4)
    //     let pool_size = Self::compute_pool_size(n_threads as u32, number_of_parts as u32);
    //
    //     let mut offset: u32 = 0;
    //     for pool_index in 0..n_threads {
    //         if pool_size[pool_index] != 0 {
    //             log_info!(
    //                 "Prepare the pool number [{}] of size [{}] (parts) : [{} -> {}], follower=[{}]",
    //                 pool_index,
    //                 pool_size[pool_index],
    //                 offset,
    //                 offset + pool_size[pool_index] - 1,
    //                 &self.follower
    //             );
    //
    //             let mut enc_slides = HashMap::new();
    //             for index in offset..offset + pool_size[pool_index] {
    //                 let v = base64::engine::general_purpose::URL_SAFE_NO_PAD
    //                     .decode(
    //                         enc_parts
    //                             .get(&index)
    //                             .ok_or(anyhow!("Wrong index"))
    //                             .map_err(tr_fwd!())?,
    //                     )
    //                     .map_err(tr_fwd!())?;
    //
    //                 // let v = enc_parts.get(&index).ok_or(anyhow!("Wrong index")).map_err(tr_fwd!())?.from_base64().map_err(tr_fwd!())?;
    //                 enc_slides.insert(index, v);
    //             }
    //
    //             offset += pool_size[pool_index];
    //
    //             let s_customer_key = customer_key.to_owned();
    //             let local_self = self.clone();
    //             let th = thread::spawn(move || {
    //                 local_self.decrypt_slide_of_parts(pool_index as u32, enc_slides, s_customer_key)
    //             });
    //
    //             thread_pool.push(th);
    //         }
    //     }
    //
    //     let mut clear_slide_parts: IndexedParts = HashMap::new();
    //
    //     for th in thread_pool {
    //         // Run the decrypt for a specific slide of parts (will use 1 core)
    //         match th.join() {
    //             Ok(v) => {
    //                 if let Ok(clear_parts) = v {
    //                     for x in clear_parts {
    //                         clear_slide_parts.insert(x.0, x.1);
    //                     }
    //                 };
    //             }
    //             Err(e) => {
    //                 log_error!("Thread join error [{:?}], follower=[{}]", e, &self.follower);
    //             }
    //         }
    //     }
    //
    //     Ok(clear_slide_parts)
    // }

    /// Decypher the file parts in parallel
    pub async fn parallel_decrypt(
        &self,
        enc_parts: HashMap<u32, String>,
        customer_key: &str,
    ) -> anyhow::Result<IndexedParts> {
        let mut task_handles = vec![];
        let n_threads = std::cmp::max(1, num_cpus::get() - 1);

        log_debug!("Number of threads=[{}], follower=[{}]", n_threads, &self.follower);

        let number_of_parts = enc_parts.len();
        let pool_size = Self::compute_pool_size(n_threads as u32, number_of_parts as u32);

        let mut offset: u32 = 0;
        for pool_index in 0..n_threads {
            if pool_size[pool_index] != 0 {
                log_info!(
                    "Prepare the pool number [{}] of size [{}] (parts) : [{} -> {}], follower=[{}]",
                    pool_index,
                    pool_size[pool_index],
                    offset,
                    offset + pool_size[pool_index] - 1,
                    &self.follower
                );

                let mut enc_slides = HashMap::new();
                for index in offset..offset + pool_size[pool_index] {
                    let v = base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .decode(enc_parts.get(&index).ok_or(anyhow!("Wrong index"))?)?;

                    enc_slides.insert(index, v);
                }

                offset += pool_size[pool_index];

                let s_customer_key = customer_key.to_owned();
                let local_self = self.clone();

                // Utiliser tokio::spawn pour créer une tâche asynchrone
                let task_handle = task::spawn(async move {
                    local_self.decrypt_slide_of_parts(pool_index as u32, enc_slides, s_customer_key)
                });

                task_handles.push(task_handle);
            }
        }

        let mut clear_slide_parts: IndexedParts = HashMap::new();

        // Wait for all task to be completed
        let results = future::join_all(task_handles).await;

        for result in results {
            match result {
                Ok(Ok(clear_parts)) => {
                    for (key, value) in clear_parts {
                        clear_slide_parts.insert(key, value);
                    }
                }
                Ok(Err(e)) => {
                    log_error!("Decryption error for pool [{:?}], follower=[{}]", e, &self.follower,);
                }
                Err(e) => {
                    log_error!("Task join error [{:?}], follower=[{}]", e, &self.follower);
                }
            }
        }

        Ok(clear_slide_parts)
    }

    /// Decypher a few parts
    fn decrypt_slide_of_parts(
        &self,
        pool_index: u32,
        enc_slides: IndexedParts,
        customer_key: String,
    ) -> anyhow::Result<IndexedParts> {
        let mut clear_slides: HashMap<u32, Vec<u8>> = HashMap::new();

        log_info!(
            "Decrypt, pool_index=[{}], number of parts=[{}], follower=[{}]",
            pool_index,
            enc_slides.len(),
            &self.follower
        );

        for (index, enc_content) in enc_slides {
            let clear_content = DkEncrypt::new(CC20).decrypt_vec(&enc_content, &customer_key).map_err(err_fwd!(
                "Cannot decrypt the part, pool_index=[{}], follower=[{}]",
                pool_index,
                &self.follower
            ))?;

            clear_slides.insert(index, clear_content);
        }

        log_info!("😎 Decrypted, pool_index=[{}], follower=[{}]", pool_index, &self.follower);
        Ok(clear_slides)
    }

    async fn create_file_reference(
        &self,
        customer_code: &str,
        content_length: &Option<u64>,
    ) -> anyhow::Result<(i64, String)> {
        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

        let file_ref = uuid_v4();

        let sql_query = format!(
            r"INSERT INTO fs_{}.file_reference
            ( file_ref, mime_type,  checksum, original_file_size,  encrypted_file_size,  total_part, is_encrypted )
            VALUES ( :p_file_ref, :p_mime_type, :p_checksum, :p_original_file_size, :p_encrypted_file_size, :p_total_part, false)",
            customer_code
        );

        let sequence_name = format!("fs_{}.file_reference_id_seq", customer_code);
        let number_of_parts = content_length.map(|l| ((l / Self::BLOCK_SIZE as u64) + 1) as i32);

        let mut params = HashMap::new();

        params.insert("p_file_ref".to_string(), CellValue::from_raw_string(file_ref.clone()));
        params.insert("p_mime_type".to_string(), CellValue::from_raw_string(String::from("text")));

        params.insert("p_checksum".to_string(), CellValue::String(None));
        params.insert("p_original_file_size".to_string(), CellValue::Int(None));
        params.insert("p_encrypted_file_size".to_string(), CellValue::Int(None));
        params.insert("p_total_part".to_string(), CellValue::Int32(number_of_parts));

        let sql_insert = SQLChangeAsync { sql_query, params, sequence_name };

        let file_id =
            sql_insert.insert(&mut trans).await.map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;

        // End of the 'blue' transaction
        trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        log_info!(
            "😎 Committed. Successfully created a file reference, file_ref=[{}], follower=[{}]",
            &file_ref,
            &self.follower
        );

        Ok((file_id, file_ref))
    }

    async fn update_file_reference(
        &self,
        file_id: i64,
        total_size: usize,
        total_part: u32,
        media_type: &str,
        customer_code: &str,
    ) -> anyhow::Result<()> {
        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

        let sql_query = format!(
            r"UPDATE fs_{}.file_reference
                                        SET
                                            original_file_size = :p_original_file_size,
                                            total_part = :p_total_part,
                                            mime_type = :p_mime_type,
                                            is_encrypted = true
                                        WHERE id = :p_file_id ",
            customer_code
        );

        let sequence_name = format!("fs_{}.file_reference_id_seq", customer_code);

        let mut params = HashMap::new();
        params.insert("p_original_file_size".to_string(), CellValue::from_raw_int(total_size as i64));
        params.insert("p_total_part".to_string(), CellValue::from_raw_int_32(total_part as i32));
        params.insert("p_file_id".to_string(), CellValue::from_raw_int(file_id));
        params.insert("p_mime_type".to_string(), CellValue::from_raw_string(media_type.to_string()));

        let sql_update = SQLChangeAsync { sql_query, params, sequence_name };

        let _ =
            sql_update.update(&mut trans).await.map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;

        // End of the 'purple' transaction
        trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        Ok(())
    }

    async fn delete_from_target_table(
        &self,
        target_table: &str,
        file_id: i64,
        customer_code: &str,
    ) -> anyhow::Result<()> {
        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

        let sql_delete =
            format!("DELETE FROM fs_{}.{} WHERE file_reference_id = :p_file_id", &customer_code, &target_table);

        let mut params = HashMap::new();
        params.insert("p_file_id".to_string(), CellValue::from_raw_int(file_id));

        let query = SQLChangeAsync { sql_query: sql_delete.to_string(), params, sequence_name: "".to_string() };

        let _ = query.delete(&mut trans).await.map_err(err_fwd!(
            "💣 Query failed, [{}], , follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;
        trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        Ok(())
    }

    async fn delete_from_file_uploads(&self, file_ref: &str, customer_code: &str) -> anyhow::Result<()> {
        let mut cnx = SQLConnectionAsync::from_pool().await.map_err(tr_fwd!())?;
        let mut trans = cnx.begin().await.map_err(tr_fwd!())?;

        let sql_delete = format!("DELETE FROM fs_{}.file_uploads WHERE file_ref = :p_file_ref", &customer_code);

        let mut params = HashMap::new();
        params.insert("p_file_ref".to_string(), CellValue::from_raw_str(&file_ref));

        let query = SQLChangeAsync { sql_query: sql_delete.to_string(), params, sequence_name: "".to_string() };

        let _ = query.delete(&mut trans).await.map_err(err_fwd!(
            "💣 Query failed, [{}], , follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;
        trans.commit().await.map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        log_info!(
            "😎 Committed. Successfully delete file uploads, file_ref=[{}], follower=[{}]",
            &file_ref,
            &self.follower
        );

        Ok(())
    }
}

//
// cargo test file_server_tests  -- --nocapture
//
#[cfg(test)]
mod file_server_tests {
    use std::path::Path;
    use std::process::exit;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn init_log() {
        INIT.call_once(|| {
            // TODO Use the future commons-config
            let log_config: String = "E:/doka-configs/dev/ppm/config/log4rs.yaml".to_string();
            let log_config_path = Path::new(&log_config);

            match log4rs::init_file(&log_config_path, Default::default()) {
                Err(e) => {
                    eprintln!("{:?} {:?}", &log_config_path, e);
                    exit(-59);
                }
                Ok(_) => {}
            }
        });
    }

    // #[test]
    // fn test_1() {
    //     init_log();
    //     const N_PARTS: u32 = 100;
    //     let delegate = FileDelegate::new(
    //         SessionToken("MY SESSION".to_owned()),
    //         XRequestID::from_value(Option::None),
    //     );
    //     let mut enc_parts = HashMap::new();
    //     for index in 0..N_PARTS {
    //         let v = "0000".to_string();
    //         enc_parts.insert(index, v);
    //     }
    //     // dbg!(&enc_parts);
    //     let r = delegate
    //         .parallel_decrypt(enc_parts, "MY_CUSTOMER_KEY")
    //         .unwrap();
    //     for i in 0..N_PARTS {
    //         println!("{} -> {:?}", i, r.get(&i).unwrap());
    //     }
    // }
    //
    // #[test]
    // fn test_2() {
    //     fn calculate_code(text: &str) -> String {
    //         let mut code_bytes: Vec<u8> = Vec::new();
    //
    //         for byte in text.bytes() {
    //             let item = (byte % 26) + b'a';
    //             code_bytes.push(item);
    //         }
    //
    //         String::from_utf8_lossy(&code_bytes).into_owned()
    //     }
    //
    //     let input_text = "111-snow image bright.jpg";
    //     let code = calculate_code(input_text);
    //     println!("{}", code);
    // }
}
