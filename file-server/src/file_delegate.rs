use std::{io, thread};
use std::cmp::{max, min};
use std::collections::HashMap;
use std::io::Read;
use std::thread::JoinHandle;
use std::time::SystemTime;

use anyhow::anyhow;
use base64::Engine;
use log::{debug, error, info, warn};
use rocket::Data;
use rocket::http::{ContentType, RawStr, Status};
use rocket::response::Content;
use rocket::response::status::Custom;
use rs_uuid::iso::uuid_v4;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use commons_error::*;
use commons_pg::{CellValue, SQLChange, SQLConnection, SQLDataSet, SQLQueryBlock};
use commons_services::database_lib::open_transaction;
use commons_services::key_lib::fetch_customer_key;
use commons_services::property_name::{DOCUMENT_SERVER_HOSTNAME_PROPERTY, DOCUMENT_SERVER_PORT_PROPERTY, TIKA_SERVER_HOSTNAME_PROPERTY, TIKA_SERVER_PORT_PROPERTY};
use commons_services::session_lib::valid_sid_get_session;
use commons_services::token_lib::SessionToken;
use commons_services::try_or_return;
use commons_services::x_request_id::{Follower, XRequestID};
use dkconfig::properties::get_prop_value;
use dkcrypto::dk_crypto::DkEncrypt;
use dkdto::{DownloadReply, EntrySession, ErrorSet, GetFileInfoReply, GetFileInfoShortReply, ListOfFileInfoReply, ListOfUploadInfoReply, UploadInfoReply, UploadReply, WebType, WebTypeBuilder};
use dkdto::error_codes::{FILE_INFO_NOT_FOUND, INTERNAL_DATABASE_ERROR, INTERNAL_TECHNICAL_ERROR, INVALID_TOKEN, UPLOAD_WRONG_ITEM_INFO};
use doka_cli::request_client::{DocumentServerClient, TikaServerClient, TokenType};

const TIKA_CONTENT_META : &str = "X-TIKA:content";
const CONTENT_TYPE_META : &str = "Content-Type";

type IndexedParts = HashMap<u32, Vec<u8>>;

#[derive(Debug, Clone)]
pub(crate) struct FileDelegate {
    pub session_token: SessionToken,
    pub follower: Follower,
}


impl FileDelegate {
    const BLOCK_SIZE : usize = 1_048_576;

    pub fn new(session_token: SessionToken, x_request_id: XRequestID) -> Self {
        Self {
            session_token,
            follower: Follower {
                x_request_id: x_request_id.new_if_null(),
                token_type: TokenType::None,
            }
        }
    }

    fn read_and_write_incoming_data(&self, item_info_str: &str, file_ref: &str, file_data: Data, entry_session: &EntrySession) -> anyhow::Result<(usize, u32)> {
        // Create parts
        log_info!("Start creating clear parts in the database, follower=[{}]", &self.follower);

        const INSERT_GROUP_SIZE: usize = 10;
        let mut block_set: HashMap<u32, Vec<u8>> = HashMap::with_capacity(INSERT_GROUP_SIZE);

        let mut total_size: usize = 0;
        let mut block_num: u32 = 0;

        let mut buf: [u8; Self::BLOCK_SIZE] = [0; Self::BLOCK_SIZE];
        let mut buf_pos: usize = 0;
        let mut datastream = file_data.open();

        loop {
            match datastream.read(&mut buf[buf_pos..]) {
                Ok(0) => break,
                Ok(n) => {
                    buf_pos += n;
                    if buf_pos < Self::BLOCK_SIZE {
                        continue;
                    }

                    let slice = &buf[..Self::BLOCK_SIZE];
                    block_set.insert(block_num, slice.to_vec());
                    block_num += 1;

                    if block_set.len() >= INSERT_GROUP_SIZE {
                        if let Err(e) = self.store_group_block(item_info_str, file_ref, &block_set, entry_session) {
                            if cfg!(windows) {
                                self.empty_datastream(&mut datastream.take(u64::MAX));
                            }
                            return Err(anyhow!("💣 Cannot store the set of blocks, follower=[{}], error=[{}]", &self.follower, e));
                        }
                        block_set.clear();
                    }

                    total_size += Self::BLOCK_SIZE;
                    buf_pos = 0;

                    // Store the bytes in a memory file
                    // mem_file.extend_from_slice(slice);
                }
                Err(e) => {
                    if cfg!(windows) {
                        self.empty_datastream(&mut datastream.take(u64::MAX));
                    }
                    return Err(anyhow!("💣 Cannot read input data, follower=[{}], error=[{}]", &self.follower, e));
                }
            }
        }

        // Process the remaining part
        if buf_pos > 0 {
            let slice = &buf[..buf_pos];
            block_set.insert(block_num, slice.to_vec());

            if let Err(e) = self.store_group_block(item_info_str, file_ref, &block_set, entry_session) {
                if cfg!(windows) {
                    self.empty_datastream(&mut datastream.take(u64::MAX));
                }
                return Err(anyhow!("💣 Cannot store the last set of blocks, follower=[{}], error=[{}]", &self.follower, e));
            }

            total_size += buf_pos;
            block_num += 1;
        }

        Ok((total_size, block_num))
    }


    // Get all the encrypted parts of the file
    // ( "application/pdf", {0 : "...", 1: "...", ...} )
    fn search_incoming_blocks(&self, file_ref : &str, customer_code : &str) -> anyhow::Result<SQLDataSet> {

        log_info!("Search the incoming blocks for the file, file_ref=[{}], follower=[{}]", file_ref, &self.follower);
        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx)?;

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

        let query = SQLQueryBlock {
            sql_query,
            start: 0,
            length: None,
            params,
        };

        let dataset = query.execute(&mut trans).map_err(err_fwd!("💣 Query failed, follower=[{}]", &self.follower))?;
        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;
        log_info!("😎 Found incoming blocks for the file, file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        Ok(dataset)
    }

    fn write_part(&self, file_id: i64, block_number: u32, enc_data: &str, customer_code: &str) -> anyhow::Result<()> {

        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx)?;

        let sql_query = format!(r"
                    INSERT INTO fs_{}.file_parts (file_reference_id, part_number, part_data)
                    VALUES (:p_file_reference_id, :p_part_number, :p_part_data)", customer_code);

        let sequence_name = format!("fs_{}.file_parts_id_seq", customer_code);

        let mut params = HashMap::new();
        params.insert("p_file_reference_id".to_string(), CellValue::from_raw_int(file_id));
        params.insert("p_part_number".to_string(), CellValue::from_raw_int_32(block_number as i32));
        params.insert("p_part_data".to_string(), CellValue::from_raw_str(enc_data));

        let sql_insert = SQLChange {
            sql_query,
            params,
            sequence_name,
        };

        let _file_part_id = sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;
        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;
        log_info!("Encrypted block inserted as a part, block_num=[{}], follower=[{}]", block_number, &self.follower);
        Ok(())
    }

    ///
    ///
    ///
    fn serial_encrypt(&self, file_id: i64, file_ref: &str, block_count: u32, customer_code: &str, customer_key: &str) -> anyhow::Result<()> {
        // Query the blocks from file_upload table

        let mut dataset = self.search_incoming_blocks(file_ref , customer_code)?; //todo
        let mut row_index: u32 = 0;

        // Loop the blocks
        while dataset.next() {
            let block_number = dataset.get_int_32("part_number").ok_or(anyhow!("Wrong part_number col"))? as u32;
            let part_data = dataset.get_string("part_data").ok_or(anyhow!("Wrong part_data col"))?;

            // | Encrypt the data
            let raw_value = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(part_data)
                .map_err(tr_fwd!())?;

            let encrypted_block = DkEncrypt::encrypt_vec(&raw_value, &customer_key)
                .map_err(err_fwd!("Cannot encrypt the data block, follower=[{}]", &self.follower))?;

            // | Store the data in the file_parts

            let enc_data = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&encrypted_block);
            log_info!("Encrypted the row number=[{}/{}], enc_parts=[{}], follower=[{}]",
                                         row_index, block_count-1, &enc_data[..10] , &self.follower );
            let _ = self.write_part(file_id, block_number, &enc_data, customer_code)?;

            row_index += 1;
        }

        Ok(())
    }


    fn process_file_blocks(&self, file_id: i64, file_ref: &str, _item_info_str: &str, block_count: u32, customer_code: &str, customer_key: &str) -> anyhow::Result<()> {
        log_info!("Process the blocks for file ref = [{}], follower=[{}]", &file_ref, &self.follower);
        // Read the file parts from the file_uploads table, encrypt the blocks and store the encrypted part into file_parts
        let _ = self.serial_encrypt(file_id, file_ref, block_count, customer_code, customer_key)?;

        // Parse the file (Tika)
        let _r = self.serial_parse_content(file_id, &file_ref, block_count, customer_code)?;
        log_info!("😎 Successful process file for file_ref=[{}], file_id=[{}], follower=[{}]", file_ref, file_id, &self.follower);
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
    pub fn upload2(&mut self, item_info: &RawStr, file_data: Data) -> WebType<UploadReply> {
        // Pre-processing
        log_info!("🚀 Start upload api, item_info=[{}], follower=[{}]", &item_info, &self.follower);

        // Check if the token is valid
        let entry_session = try_or_return!(valid_sid_get_session(&self.session_token, &mut self.follower), Self::web_type_error());
        let customer_code = entry_session.customer_code.as_str();

        // Read the item_info
        let Ok(item_info_str) =  item_info.percent_decode().map_err(err_fwd!("💣 Invalid item info, [{}]", item_info) ) else {
            return WebType::from_errorset(UPLOAD_WRONG_ITEM_INFO);
        };

        // Get the crypto key

        let Ok(customer_key) = fetch_customer_key(customer_code, &self.follower)
            .map_err(err_fwd!("💣 Cannot get the customer key, follower=[{}]", &self.follower)) else {
            if cfg!(windows) {
                self.empty_datastream(&mut file_data.open().take(u64::MAX));
            }
            return WebType::from_errorset(INTERNAL_TECHNICAL_ERROR);
        };

        // Create an entry in file_reference
        let Ok(( file_id, file_ref )) = self.create_file_reference(customer_code)
            .map_err(err_fwd!("💣 Cannot create an entry in the file reference table, follower=[{}]", &self.follower)) else {
            if cfg!(windows) {
                self.empty_datastream(&mut file_data.open().take(u64::MAX));
            }
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        };

        log_info!("😎 Created entry in file reference, file_id=[{}], file_ref=[{}], follower=[{}]", file_id, &file_ref, &self.follower);

        // Phase 1 :  Read all the incoming blocks and write them in the DB (file_uploads table)
        let Ok((total_size, block_count)) = self.read_and_write_incoming_data(&item_info_str, &file_ref, file_data, &entry_session)
            .map_err(err_fwd!("💣 Cannot write parts, follower=[{}]", &self.follower)) else {
            // The stream is managed by the routine above, so no need to empty it here.
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        };

        // Phase 2 : Run a thread to perform all the other operations (encrypt, tika parse, ...)
        self.thread_processing_block(&item_info_str, file_id, &file_ref, customer_code,  &customer_key, block_count);


        // Return the file_reference

        log_info!("🏁 End upload api, follower=[{}]", &self.follower);

        WebType::from_item(Status::Ok.code, UploadReply {
            file_ref: file_ref.clone(),
            size : total_size,
            block_count,
        })
    }

    fn thread_processing_block(&self, item_info_str: &str, file_id: i64, file_ref: &str, customer_code: &str, customer_key: &str, block_count: u32) {
        let local_self = self.clone();
        let local_item_info_str = String::from(item_info_str);
        let local_file_ref = String::from(file_ref);
        let local_customer_code = String::from(customer_code);
        let local_customer_key = String::from(customer_key);
        let _th = thread::spawn( move || {
            let status = local_self.process_file_blocks(file_id, &local_file_ref, &local_item_info_str, block_count, &local_customer_code, &local_customer_key);
            if status.is_err() {
                log_info!("💣 The file processing failed. Enter the rollback process, file_ref=[{}], follower=[{}]", &local_file_ref, &local_self.follower);
                // Clean the tables : file_parts (file_id) + file_metadata (file_id)
                let _ = local_self.delete_from_target_table( "file_parts",  file_id, &local_customer_code);
                let _ = local_self.delete_from_target_table("file_metadata",  file_id, &local_customer_code);
                // Change the status of file_reference (file_id) : put all the values to "0" (size + total_part)
                let _ = local_self.update_file_reference(file_id, 0, 0, "text", &local_customer_code);
                // Call the document server to delete the text indexing
                if let Ok(document_server) = Self::find_document_server_client().map_err(err_fwd!("Cannot find the document server")) {
                    let _ = document_server.delete_text_indexing( &local_file_ref, &local_self.follower.token_type.value());
                }
            }

            // Clean file_uploads (file_ref)
            let _ = local_self.delete_from_file_uploads(&local_file_ref, &local_customer_code);
        });
    }

    // Windows only
    fn empty_datastream(&self, reader : &mut dyn Read) {
        // BUG https://github.com/SergioBenitez/Rocket/issues/892
        log_warn!("⛔ Running on Windows, need to read the datastream");
        let _r = io::copy( reader, &mut io::sink());
    }

    ///
    /// Run a thread to process the block and store it in the DB
    ///
    fn _parallel_crypto_and_store_block(&self, file_id : i64, block_set : &HashMap<u32, Vec<u8>>,
                                       customer_code : &str,
                                       customer_key : &str) -> JoinHandle<anyhow::Result<()>> {

        let s_customer_code = customer_code.to_owned();
        let s_customer_key = customer_key.to_owned();
        let local_block_set = block_set.into_iter()
            .map(|(key, value)| { (*key, (*value).to_owned())  })
            .collect();


        let local_self = self.clone();

        let th = thread::spawn( move || {
            local_self._crypto_and_store_block(file_id, local_block_set,
                                    s_customer_code, s_customer_key)
        });

        th
    }


    fn serial_parse_content(&self, file_id: i64, file_ref: &str, block_count: u32, customer_code: &str) -> anyhow::Result<()> {
        // Build the file in memory
        let mut mem_file : Vec<u8> = vec![];
        let mut dataset = self.search_incoming_blocks(/*&mut trans,*/ file_ref , customer_code).map_err(tr_fwd!())?;

        // Loop the blocks
        while dataset.next() {
            let part_data = dataset.get_string("part_data").ok_or(anyhow!("Wrong part_data col"))?;

            // | Encrypt the data
            let raw_value = base64::engine::general_purpose::URL_SAFE_NO_PAD
                .decode(part_data)
                .map_err(tr_fwd!())?;

            mem_file.extend(&raw_value);
        }

        let total_size = mem_file.len();
        // Read the metadata and the raw text of the file
        let media_type = self.analyse_entire_content(&file_ref, mem_file, &customer_code).map_err(tr_fwd!())?;
        // Update the file_reference table : checksum, original_file_size, total_part, media_type
        let _ =self.update_file_reference(file_id, total_size,
                                          block_count, &media_type, customer_code).map_err(tr_fwd!())?;
        Ok(())
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


    ///
    ///
    fn store_group_block(&self, item_info_str: &str, file_ref : &str, block_set : &HashMap<u32, Vec<u8>>,
                              entry_session: &EntrySession) -> anyhow::Result<()> {
        // Open the transaction
        let block_range = Self::min_max(&block_set);

        log_info!("Block range processing, block range=[{:?}], follower=[{}]", &block_range, &self.follower);

        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error, block_range=[{:?}], follower=[{}]", &block_range, &self.follower))?;

        for (block_num, block) in block_set {

            log_debug!("Block processing... : block_num=[{}], follower=[{}]", block_num, &self.follower);

            let original_part_size = block.len();
            // Store in the DB
            let data = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&block);
            // let data = encrypted_block.to_base64(URL_SAFE);

            let sql_query = format!(r"
            INSERT INTO fs_{}.file_uploads (session_id, start_time_gmt,
                                      user_id, item_info, file_ref, part_number, original_part_size, part_data)
            VALUES (:p_session_id, :p_start_time_gmt,
                    :p_user_id, :p_item_info, :p_file_ref, :p_part_number, :p_original_part_size, :p_part_data)", &entry_session.customer_code);

            let mut params = HashMap::new();

            params.insert("p_session_id".to_string(),CellValue::from_raw_str(&self.follower.token_type.value()));
            params.insert("p_start_time_gmt".to_string(),CellValue::from_raw_systemtime(SystemTime::now()));
            params.insert("p_user_id".to_string(),CellValue::from_raw_int(entry_session.user_id));
            params.insert("p_item_info".to_string(),CellValue::from_raw_str(item_info_str));
            params.insert("p_file_ref".to_string(),CellValue::from_raw_str(file_ref));
            params.insert("p_part_number".to_string(),CellValue::from_raw_int_32(*block_num as i32));
            params.insert("p_original_part_size".to_string(),CellValue::from_raw_int(original_part_size as i64));
            params.insert("p_part_data".to_string(),CellValue::from_raw_string(data));


            let sql_insert = SQLChange {
                sql_query,
                params,
                sequence_name: "".to_uppercase(),
            };

            sql_insert.insert_no_pk(&mut trans).map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;
            log_debug!("...Block inserted, block_num=[{}], follower=[{}]", block_num, &self.follower);
        }

        // End of the 'pink' multiple transaction
        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;
        log_info!("😎 Committed. Block inserted, block_range=[{:?}], follower=[{}]", &block_range, &self.follower);

        Ok(())
    }


    //
    fn _crypto_and_store_block(&self, file_id : i64, block_set : HashMap<u32, Vec<u8>>,
                              customer_code: String, customer_key: String) -> anyhow::Result<()> {

        // Open the transaction
        let block_range = Self::min_max(&block_set);

        log_info!("Block range processing, block range=[{:?}], follower=[{}]", &block_range, &self.follower);

        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error, block_range=[{:?}], follower=[{}]", &block_range, &self.follower))?;

        for (block_num, block) in block_set {

            log_debug!("Block processing... : block_num=[{}], follower=[{}]", block_num, &self.follower);

            // Encrypt the block
            let encrypted_block = DkEncrypt::encrypt_vec(&block, &customer_key)
                .map_err(err_fwd!("Cannot encrypt the data block, follower=[{}]", &self.follower))?;

            // and store in the DB

            let data = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(encrypted_block);
            // let data = encrypted_block.to_base64(URL_SAFE);

            let sql_query = format!(r"
                    INSERT INTO fs_{}.file_parts (file_reference_id, part_number, part_data)
                    VALUES (:p_file_reference_id, :p_part_number, :p_part_data)", customer_code);

            let sequence_name = format!("fs_{}.file_parts_id_seq", customer_code);

            let mut params = HashMap::new();
            params.insert("p_file_reference_id".to_string(), CellValue::from_raw_int(file_id));
            params.insert("p_part_number".to_string(), CellValue::from_raw_int_32(block_num as i32));
            params.insert("p_part_data".to_string(), CellValue::from_raw_string(data));

            let sql_insert = SQLChange {
                sql_query,
                params,
                sequence_name,
            };

            let _file_part_id = sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;

            log_debug!("...Block inserted, block_num=[{}], follower=[{}]", block_num, &self.follower);

        }

        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        log_info!("😎 Committed. Block inserted, block_range=[{:?}], follower=[{}]", &block_range, &self.follower);

        Ok(())
    }


    fn find_document_server_client() -> anyhow::Result<DocumentServerClient> {
        let document_server_host = get_prop_value(DOCUMENT_SERVER_HOSTNAME_PROPERTY)?;
        let document_server_port = get_prop_value(DOCUMENT_SERVER_PORT_PROPERTY)?.parse::<u16>()?;
        Ok(DocumentServerClient::new(&document_server_host, document_server_port))
    }

    /// Call the tika server to parse the file and get the text data
    /// Insert the metadata
    /// Call the document server to fulltext parse the text data
    /// return the media type
    fn analyse_entire_content(&self, file_ref: &str, mem_file : Vec<u8>, customer_code: &str) -> anyhow::Result<String> {

        log_info!("Parsing file content ... ,file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        let tika_server_host = get_prop_value(TIKA_SERVER_HOSTNAME_PROPERTY)?;
        let tika_server_port = get_prop_value(TIKA_SERVER_PORT_PROPERTY)?.parse::<u16>()?;

        // Get the raw text from the original file
        let tsc = TikaServerClient::new(&tika_server_host, tika_server_port);
        let raw_json = tsc.parse_data_json(&mem_file).map_err(err_fwd!("Cannot parse the original file"))?;
        let x_tika_content = raw_json[TIKA_CONTENT_META].as_str().ok_or(anyhow!("Bad tika content"))?;
        let content_type = raw_json[CONTENT_TYPE_META].as_str().ok_or(anyhow!("Bad content type"))?;

        let metadata = raw_json.as_object().unwrap();
        log_info!("Parsing done for file_ref=[{}], content size=[{}], content type=[{}], follower=[{}]",
            file_ref, x_tika_content.len(), &content_type,  &self.follower);

        let _ = self.insert_metadata(&customer_code, file_ref, &metadata)?;

        // TODO TikaParsing can contain all the metadata, so keep them and return then instead of getting only the content-type.
        log_info!("Metadata done for file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        let document_server = Self::find_document_server_client().map_err(err_fwd!("Cannot find the document server"))?;
        // TODO we must also pass the  self.follower.x_request_id + handle the file name
        let wr_reply = document_server.fulltext_indexing(&x_tika_content,
                                                         "no_filename_for_now",
                                                         file_ref,
                                                         &self.follower.token_type.value());
        match wr_reply {
            Ok(reply) => {
                log_info!("Fulltext indexing done, number of text parts=[{}], follower=[{}]", reply.part_count, &self.follower);
                self.set_file_reference_fulltext_indicator(file_ref, customer_code)
                    .map_err(err_fwd!("Cannot set the file reference to fulltext parsed indicator, follower=[{}]", &self.follower))?;
            }
            Err(e) => {
                log_error!("Error while sending the raw text to the fulltext indexing, file_ref=[{}], reply=[{:?}], follower=[{}], ",  file_ref, e, &self.follower);
                return Err(anyhow::anyhow!(e.message));
            }
        }

        log_info!("... End of parse file content processing, file_ref=[{}], follower=[{}]", file_ref, &self.follower);
        Ok(content_type.to_owned())
    }

    fn insert_metadata(&self, customer_code: &str, file_ref: &str, metadata : &Map<String, Value>) -> anyhow::Result<()> {

        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx)?;

        let sql_query = format!(r"INSERT INTO fs_{0}.file_metadata ( file_reference_id, meta_key,  value )
        VALUES ((SELECT id FROM fs_{0}.file_reference WHERE file_ref = :p_file_ref), :p_meta_key, :p_value)", customer_code);
        let sequence_name = format!( "fs_{}.file_metadata_id_seq", customer_code );

        // TODO Could be done with a specific batch insert sql routine that will build a big insert statement!
        for (key, value) in metadata.iter() {
            if key != TIKA_CONTENT_META && value.to_string().len() < 200 {
                let mut params: HashMap<String, CellValue> = HashMap::new();
                params.insert("p_file_ref".to_owned(), CellValue::from_raw_str(file_ref));
                params.insert("p_meta_key".to_owned(), CellValue::from_raw_str(key));
                params.insert("p_value".to_owned(), CellValue::from_raw_string(value.to_string()));

                let sql_insert = SQLChange {
                    sql_query: sql_query.clone(),
                    params,
                    sequence_name: sequence_name.clone(),
                };
                let meta_id = sql_insert.insert(&mut trans)
                    .map_err(err_fwd!("💣 Cannot insert the metadata, file_ref=[{}], key=[{}], value=[{}], follower=[{}]",
                    file_ref, key, value.to_string(), &self.follower))?;

                log_info!("Success inserting meta meta_id=[{}], file_ref=[{}], key=[{}], value=[{}], follower=[{}]",
                    meta_id, file_ref, key, value.to_string(), &self.follower);
            }
        }

        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        Ok(())
    }

    //
    fn set_file_reference_fulltext_indicator(&self, file_ref: &str, customer_code: &str) -> anyhow::Result<()> {

        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx)?;

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

        let _ = sql_update.update(&mut trans).map_err(err_fwd!("Update failed, follower=[{}]", &self.follower))?;
        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        log_info!("😎 Committed. Successfully set the full text indicator, file_ref=[{:?}], follower=[{}]", file_ref, &self.follower);

        Ok(())
    }

    fn web_type_error<T>() -> impl Fn(ErrorSet<'static>) -> WebType<T>  where T : DeserializeOwned {
        |e| {
            log_error!("💣 Error after try {:?}", e);
            WebType::from_errorset(e)
        }
    }

    ///
    /// ✨ Get the information about the composition of a file [file_ref]
    ///
    pub fn file_info(&mut self, file_ref: &RawStr) -> WebType<GetFileInfoReply> {
        log_info!("🚀 Start upload api, follower=[{}]", &self.follower);
        // Check if the token is valid

        let entry_session = try_or_return!(valid_sid_get_session(&self.session_token, &mut self.follower), Self::web_type_error());

        let Ok(file_ref_str) =  file_ref.percent_decode().map_err(err_fwd!("💣 Invalid file ref, [{}]", file_ref) ) else {
            return WebType::from_errorset(FILE_INFO_NOT_FOUND);
        };

        let customer_code = entry_session.customer_code.as_str();

        let mut files = try_or_return!(self.fetch_files_information(file_ref_str.as_ref(), &customer_code) ,
            Self::web_type_error()
        );

        // FIXME : I don't know I can do a remove(0) without being sure there is at least an item at 0
        let item = files.list_of_files.remove(0);

        log_info!("🏁 End file_info api, follower=[{}]", &self.follower);
        WebType::from_item(Status::Ok.code, item)
    }


    /// ✨ Find the files in the system
    pub fn file_list(&mut self, match_expression: &RawStr) -> WebType<ListOfFileInfoReply> {
        log_info!("🚀 Start file_list api, follower=[{}]", &self.follower);

        let entry_session = try_or_return!(valid_sid_get_session(&self.session_token, &mut self.follower), |e| WebType::from_errorset(e) );

        log_info!("😎 We read the session information, customer_code=[{}], user_id=[{}], follower=[{}]", &entry_session.customer_code,
            &entry_session.user_id, &self.follower);

        let Ok(match_expression_str) =  match_expression.percent_decode().map_err(err_fwd!("💣 Invalid match expression, [{}]", match_expression) ) else {
            return WebType::from_errorset(FILE_INFO_NOT_FOUND);
        };

        let r_files = self.fetch_files_information(&match_expression_str, &entry_session.customer_code);

        log_info!("🏁 End file_list api, follower=[{}]", &self.follower);

        match r_files {
            Ok(files) => {
                WebType::from_item(Status::Ok.code, files)
            }
            Err(e) => {
                WebType::from_errorset(e)
            }
        }
    }

    fn is_valid_pattern(s: &str) -> bool {
        s.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '*')
    }

    /// Query the information related to the existing files whose the reference matches the given pattern
    fn fetch_files_information(&self, pattern: &str, customer_code: &str) -> Result<ListOfFileInfoReply, ErrorSet<'static>> {

        if ! Self::is_valid_pattern(&pattern) {
            return Err(FILE_INFO_NOT_FOUND);
        }

        let sql_pattern = pattern.replace('*', "%");

        let sql_query = format!(r"SELECT
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
                    fr.file_ref like :p_file_reference", customer_code);

        let r_data_set: anyhow::Result<SQLDataSet> = (|| {
            let mut r_cnx = SQLConnection::new();
            let mut trans = open_transaction(&mut r_cnx)?;
            let mut params = HashMap::new();
            params.insert("p_file_reference".to_string(), CellValue::from_raw_string(sql_pattern));
            let query = SQLQueryBlock {
                sql_query,
                start: 0,
                length: None,
                params,
            };
            let dataset = query.execute(&mut trans)?;
            trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", & self.follower))?;
            Ok(dataset)
        })();
        let Ok(mut data_set) = r_data_set else {
            return Err(INTERNAL_DATABASE_ERROR);
        };

        fn build_block_info(data_set: &mut SQLDataSet) -> anyhow::Result<GetFileInfoReply> {
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

        let mut files = ListOfFileInfoReply {
            list_of_files: vec![],
        };

        while data_set.next() {
            match build_block_info(&mut data_set) {
                Ok(block_info) => {
                    files.list_of_files.push(block_info)
                }
                Err(e) => {
                    log_error!("💣 Error while building file info, e=[{}], follower=[{}]", e, & self.follower);
                    return Err(INTERNAL_DATABASE_ERROR)
                }
            }
        }
        Ok(files)
    }

    ///
    /// ✨ Get the information about the files being loaded
    ///
    /// Get all the upload information. Only the session id is required to identify the (customer id/user id)
    ///
    /// All the current uploads will be extracted for the current user and a then a list of information will be returned
    ///
    /// * start_date_time :
    /// * item_info :  Is a non unique string to make link with the item element during the initial phase of upload (ex: the file name)
    /// * file_reference :
    /// * session_number :
    ///
    pub fn file_loading(&mut self) -> WebType<ListOfUploadInfoReply> {

        log_info!("🚀 Start file_loading api, follower=[{}]", &self.follower);

        let entry_session = try_or_return!(valid_sid_get_session(&self.session_token, &mut self.follower), Self::web_type_error());

        log_info!("😎 We read the session information, customer_code=[{}], user_id=[{}], follower=[{}]", &entry_session.customer_code,
            &entry_session.user_id, &self.follower);

        let sql_query = format!(r"SELECT current_uploads.file_ref,
                            current_uploads.session_id,
                            current_uploads.start_time,
                            current_uploads.item_info,
                            current_uploads.count_uploaded,
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
                             GROUP BY file_ref, session_id, user_id, item_info ) current_uploads", entry_session.customer_code);

        let r_data_set : anyhow::Result<SQLDataSet> = (|| {
            let mut r_cnx = SQLConnection::new();
            let mut trans = open_transaction(&mut r_cnx)?;

            let mut params = HashMap::new();
            params.insert("p_user_id".to_string(), CellValue::from_raw_int(entry_session.user_id));

            let query = SQLQueryBlock {
                sql_query,
                start: 0,
                length: None,
                params,
            };

            let dataset = query.execute(&mut trans)?;
            trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

            Ok(dataset)
        })();

        let Ok(mut data_set) = r_data_set else {
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        };

        // inner function
        fn build_loading_info_item(data_set: &mut SQLDataSet) -> anyhow::Result<UploadInfoReply> {
            let file_reference = data_set.get_string("file_ref").ok_or(anyhow!("Wrong file_ref col"))?;
            let session_number = data_set.get_string("session_id").ok_or(anyhow!("Wrong session_id col"))?;
            let item_info = data_set.get_string("item_info").ok_or(anyhow!("Wrong item_info col"))?;
            let start_date_time = data_set.get_timestamp_as_datetime("start_time").ok_or(anyhow!("Wrong start_time col"))?;
            let encrypted_count = data_set.get_int("count_encrypted").ok_or(anyhow!("Wrong count_encrypted col"))?;
            let uploaded_count = data_set.get_int("count_uploaded").ok_or(anyhow!("Wrong count_uploaded col"))?;

            let limit = min(session_number.len()-2, 22);
            Ok(UploadInfoReply{
                start_date_time,
                item_info,
                file_reference,
                session_number: format!("{}...", session_number[..limit].to_owned()),
                encrypted_count,
                uploaded_count,
            })
        }

        let mut list_of_upload_info : Vec<UploadInfoReply> = vec![];
        while data_set.next() {
            let Ok(loading_info_item) = build_loading_info_item(&mut data_set).map_err(err_fwd!("Build loading info item failed, follower=[{}]", &self.follower)) else {
                return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
            };
            list_of_upload_info.push(loading_info_item);
        }

        log_info!("😎 Successfully read the loading info, follower=[{}]", &self.follower);

        let upload_info = ListOfUploadInfoReply {
            list_of_upload_info
        };

        log_info!("🏁 End file_loading api, follower=[{}]", &self.follower);
        WebType::from_item(Status::Ok.code,upload_info)
    }

    ///
    /// ✨ Get the information about the loading status of the [file_ref]
    ///
    pub fn file_stats(&mut self, file_ref: &RawStr) -> WebType<GetFileInfoShortReply> {

        log_info!("🚀 Start file_stats api, file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        // Check if the token is valid
        let entry_session = try_or_return!(valid_sid_get_session(&self.session_token, &mut self.follower), Self::web_type_error());
        let customer_code = entry_session.customer_code.as_str();

        // TODO instead of constant 1, check if the document is fulltext parsed and previewed
        let sql_query = format!(
            r" SELECT
                fr.mime_type, fr.checksum, fr.original_file_size, fr.total_part, 1 fulltext,  1 preview,
                (SELECT count(*)
                FROM  fs_{0}.file_parts
                WHERE file_reference_id = (SELECT id FROM fs_{0}.file_reference WHERE file_ref = :p_file_ref)
                ) count_encrypted,
                (SELECT  count(*) from fs_{0}.file_uploads WHERE file_ref = :p_file_ref) count_uploaded
            FROM fs_{0}.file_reference fr
            WHERE file_ref = :p_file_ref"
            , customer_code);

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
            trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

            Ok(dataset)
        })();

        let Ok(mut data_set) = r_data_set else {
            return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
        };

        // inner function
        fn build_file_info(data_set: &mut SQLDataSet, file_ref: &str) -> anyhow::Result<GetFileInfoShortReply> {
            let _mime_type = data_set.get_string("mime_type").unwrap_or("".to_string()); // optional
            let _checksum = data_set.get_string("checksum").unwrap_or("".to_string()); // optional
            let original_file_size = data_set.get_int("original_file_size")/*.ok_or(anyhow!("Wrong original_file_size col"))?*/;
            let total_part = data_set.get_int_32("total_part")/*.ok_or(anyhow!("Wrong total_part col"))?*/;
            let encrypted_count = data_set.get_int("count_encrypted").ok_or(anyhow!("Wrong count_encrypted col"))?;
            let uploaded_count = data_set.get_int("count_uploaded").ok_or(anyhow!("Wrong count_uploaded col"))?;
            let fulltext_indexed_count = data_set.get_int_32("fulltext").ok_or(anyhow!("Wrong fulltext col"))?;
            let preview_generated_count = data_set.get_int_32("preview").ok_or(anyhow!("Wrong preview col"))?;

            Ok(GetFileInfoShortReply{
                file_ref: file_ref.to_string(),
                block_count: total_part.unwrap_or(0) as u32,
                original_file_size : original_file_size.unwrap_or(0i64) as u64,
                encrypted_count,
                uploaded_count,
                fulltext_indexed_count : fulltext_indexed_count as i64,
                preview_generated_count: preview_generated_count as i64,
            })
        }

        let wt_stats = if data_set.next() {
            let Ok(stats) = build_file_info(&mut data_set, file_ref).map_err(err_fwd!("Build file info failed, follower=[{}]", &self.follower)) else {
              return WebType::from_errorset(INTERNAL_DATABASE_ERROR);
            };

            log_info!("😎 Successfully read the file stats, file_ref=[{}], follower=[{}]", file_ref, &self.follower);
            WebType::from_item(Status::Ok.code,stats)
        } else {
            log_info!("⛔ Cannot find the file stats, file_ref=[{}], follower=[{}]", file_ref, &self.follower);
            WebType::from_errorset(INTERNAL_TECHNICAL_ERROR)
        };

        log_info!("🏁 End file_stats api, follower=[{}]", &self.follower);
        wt_stats
    }

    fn download_reply_error() -> impl Fn(ErrorSet<'static>) -> DownloadReply {
        |e| {
            log_error!("💣 Error after try {:?}", e);
            DownloadReply::from_errorset(e)
        }
    }

    /// ✨ Download the binary content of a file
    pub fn download(&mut self, file_ref: &RawStr) -> DownloadReply {

        log_info!("🚀 Start download api, file_ref = [{}], follower=[{}]", file_ref, &self.follower);

        // Check if the token is valid
        let entry_session = try_or_return!(valid_sid_get_session(&self.session_token, &mut self.follower), Self::download_reply_error());
        let customer_code = entry_session.customer_code.as_str();
        log_info!("Found session and customer code=[{}], follower=[{}]", &customer_code, &self.follower);

        // Search the document's parts from the database

        let Ok((media_type, enc_parts)) = self.search_parts(file_ref, customer_code).map_err(tr_fwd!()) else {
            log_error!("");
            return DownloadReply::from_errorset(INTERNAL_DATABASE_ERROR);
        };

        let o_media : Option<ContentType> = ContentType::parse_flexible(&media_type);
        let Ok(media) = o_media.ok_or(anyhow!("Wrong media type")).map_err(tr_fwd!()) else {
            log_error!("");
            return DownloadReply::from_errorset(INTERNAL_TECHNICAL_ERROR);
        };

        log_info!("😎 Found correct media type=[{}], follower=[{}]", &media, &self.follower);

        // Get the customer key
        let Ok(customer_key) = fetch_customer_key(customer_code, &self.follower)
            .map_err(err_fwd!("💣 Cannot get the customer key, follower=[{}]", &self.follower)) else {
            return DownloadReply::from_errorset(INTERNAL_TECHNICAL_ERROR);
        };

        // Parallel decrypt of slides of parts [Parts, Q+(1*)]

        let Ok(clear_parts) = self.parallel_decrypt(enc_parts, &customer_key) else {
            log_error!("");
            return DownloadReply::from_errorset(INTERNAL_TECHNICAL_ERROR);
        };

        // Output : Get a file array of P parts

        log_info!("😎 Decrypt done, number of parts=[{}], follower=[{}]", &clear_parts.len(), &self.follower);

        // Merge all the parts in one big file (on disk??)
        let Ok(bytes) = self.merge_parts(&clear_parts) else {
            log_error!("");
            return DownloadReply::from_errorset(INTERNAL_TECHNICAL_ERROR);
        };

        log_info!("😎 Merged all the parts, file size=[{}], follower=[{}]", bytes.len(), &self.follower);
        log_info!("🏁 End download api, follower=[{}]", &self.follower);

        Custom( Status::Ok, Content(media, bytes))
    }


    // Get all the encrypted parts of the file
    // ( "application/pdf", {0 : "...", 1: "...", ...} )
    fn search_parts(&self, file_ref : &str, customer_code : &str) -> anyhow::Result<(String, HashMap<u32, String>)> {

        log_info!("Search the parts for the file, file_ref=[{}], follower=[{}]", file_ref, &self.follower);

        let sql_str = r"
            SELECT fp.id,
                fr.file_ref,
                fr.mime_type,
                fr.is_encrypted,
                fp.part_number,
                fp.part_data
            FROM  fs_{customer_code}.file_reference fr, fs_{customer_code}.file_parts fp
            WHERE
                fp.file_reference_id = fr.id AND
                fr.file_ref = :p_file_ref
            ORDER BY fr.file_ref, fp.part_number";

        let sql_query = sql_str.replace("{customer_code}", customer_code);

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

        let mut dataset = query.execute(&mut trans).map_err(err_fwd!("💣 Query failed, follower=[{}]", &self.follower))?;

        let mut parts : HashMap<u32, String> = HashMap::new();
        let mut media_type = String::new();
        while dataset.next() {
            let part_info = Self::read_part(&mut dataset).map_err(err_fwd!("Cannot read part data, follower=[{}]", &self.follower))?;
            media_type = part_info.0; // always the same media type for each row
            parts.insert(part_info.1, part_info.2);
        }

        log_info!("😎 Found parts for the file, file_ref=[{}], n_parts=[{}], follower=[{}]", file_ref, parts.len(), &self.follower);

        Ok((media_type, parts))
    }


    // ( <mdeia_type>, <part_number>, <data> )
    fn read_part(data_set: &mut SQLDataSet) -> anyhow::Result<(String, u32, String)> {
        let media_type = data_set.get_string("mime_type").ok_or(anyhow!("Wrong mime_type col"))?;
        let is_encrypted = data_set.get_bool("is_encrypted").ok_or(anyhow!("Wrong is_encrypted col"))?;
        let part_number = data_set.get_int_32("part_number").ok_or(anyhow!("Wrong part_number col"))?;
        let part_data = data_set.get_string("part_data").ok_or(anyhow!("Wrong part_data col"))?;

        if ! is_encrypted {
            return Err(anyhow!("Part is not encrypted, part number=[{}]", part_number));
        }

        Ok((media_type, part_number as u32, part_data))
    }

    //
    fn merge_parts(&self, clear_parts_slides: &IndexedParts) -> anyhow::Result<Vec<u8>> {
        let mut bytes = vec![];
        //let mut part_index: u32 = 0;
        for i in 0..clear_parts_slides.len() {
            log_info!("Join part, part number=[{}], follower=[{}]", i, &self.follower);
            let index = i as u32;
            let parts = clear_parts_slides.get(&index).ok_or(anyhow!("Wrong index")).map_err(tr_fwd!())?;
            for b in parts {
                bytes.push(*b);
            }
       //     part_index +=1;
        }
        Ok(bytes)
    }

    // N = Number of threads = Number of Cores - 1;
    // 5 cores , 20 parts => 4 decrypt by core
    // 5 cores, 22 parts => 5 5 4 4 4
    // 22 eucl 5 = 4,2 => 2 (number of extra decrypts)
    // P eucl N = [Q,R]  Q is the number of decrypts by thread and R is the number of thread with 1 extra decrypt.
    fn compute_pool_size(number_of_threads : u32, number_of_parts: u32) -> Vec<u32> {
        let mut pool_size = vec![];
        let q = number_of_parts / number_of_threads;
        let mut r = number_of_parts % number_of_threads;

        // dbg!(number_of_parts, number_of_threads, q,r);
        for _ in 0..number_of_threads {
            let extra = if r > 0  {
                r -= 1;
                1
            }
            else {
                0
            };
            pool_size.push(q+extra);

        }
        pool_size
    }

    //
    fn parallel_decrypt(&self, enc_parts: HashMap<u32, String>, customer_key: &str) -> anyhow::Result<IndexedParts> {
        let mut thread_pool = vec![];
        let n_threads = max( 1, num_cpus::get() - 1); // Number of threads is number of cores - 1

        log_debug!("Number of threads=[{}], follower=[{}]", n_threads, &self.follower);

        let number_of_parts = enc_parts.len();
        // For n_threads = 5 and num of part = 22 , we get (5,5,4,4,4)
        let pool_size = Self::compute_pool_size(n_threads as u32, number_of_parts as u32);

        let mut offset : u32 = 0;
        for pool_index in 0..n_threads {

            if pool_size[pool_index] != 0 {
                log_info!("Prepare the pool number [{}] of size [{}] (parts) : [{} -> {}], follower=[{}]",
                pool_index, pool_size[pool_index], offset, offset+pool_size[pool_index]-1, &self.follower );

                let mut enc_slides = HashMap::new();
                for index in offset..offset + pool_size[pool_index] {

                    let v = base64::engine::general_purpose::URL_SAFE_NO_PAD
                        .decode(enc_parts.get(&index).ok_or(anyhow!("Wrong index")).map_err(tr_fwd!())?)
                        .map_err(tr_fwd!())?;

                    // let v = enc_parts.get(&index).ok_or(anyhow!("Wrong index")).map_err(tr_fwd!())?.from_base64().map_err(tr_fwd!())?;
                    enc_slides.insert(index, v);
                }

                offset += pool_size[pool_index];

                let s_customer_key = customer_key.to_owned();
                let local_self = self.clone();
                let th = thread::spawn(move || {
                    local_self.decrypt_slide_of_parts(pool_index as u32, enc_slides, s_customer_key)
                });

                thread_pool.push(th);
            }
        }

        let mut clear_slide_parts : IndexedParts = HashMap::new();

        for th in thread_pool {
            // Run the decrypt for a specific slide of parts (will use 1 core)
            match th.join() {
                Ok(v) => {
                    if let Ok(clear_parts) = v {
                        for x in clear_parts {
                            clear_slide_parts.insert(x.0, x.1);
                        }
                    };
                }
                Err(e) => {
                    log_error!("Thread join error [{:?}], follower=[{}]", e, &self.follower);
                }
            }
        }

        Ok(clear_slide_parts)
    }


    //
    //
    //
    fn decrypt_slide_of_parts(&self, pool_index : u32, enc_slides : IndexedParts, customer_key: String) -> anyhow::Result<IndexedParts> {
        let mut clear_slides : HashMap<u32, Vec<u8>> = HashMap::new();

        // if pool_index == 2 {
        //     sleep(Duration::from_secs(2));
        // }

        for (index, enc_content) in enc_slides {
            log_info!("Decrypt, pool_index=[{}], part number=[{}], follower=[{}]", pool_index, index, &self.follower);

            let clear_content = DkEncrypt::decrypt_vec(&enc_content, &customer_key)
                        .map_err(err_fwd!("Cannot decrypt the part, pool_index=[{}], follower=[{}]", pool_index, &self.follower))?;

            let clear_content_size = clear_content.len();
            clear_slides.insert(index, clear_content);
            log_info!("😎 Decrypted, pool_index=[{}], part number=[{}], clear part size=[{}], follower=[{}]", pool_index, index, clear_content_size, &self.follower);
        }
        Ok(clear_slides)
    }


    ///
    ///
    ///
    fn create_file_reference(&self, customer_code: &str) -> anyhow::Result<(i64, String)> {
        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error, follower=[{}]", &self.follower))?;
        let file_ref = uuid_v4();

        let sql_query = format!(r"INSERT INTO fs_{}.file_reference
            ( file_ref, mime_type,  checksum, original_file_size,  encrypted_file_size,  total_part, is_encrypted )
            VALUES ( :p_file_ref, :p_mime_type, :p_checksum, :p_original_file_size, :p_encrypted_file_size, :p_total_part, false)", customer_code);

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

        let file_id = sql_insert.insert(&mut trans).map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;

        // End of the 'blue' transaction
        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        log_info!("😎 Committed. Successfully created a file reference, file_ref=[{}], follower=[{}]", &file_ref, &self.follower);

        Ok((file_id, file_ref))
    }

    ///
    ///
    ///
    fn update_file_reference(&self, /*mut trans: &mut SQLTransaction,*/
                             file_id : i64,
                             total_size: usize,
                             total_part: u32,
                             media_type : &str,
                             customer_code: &str) -> anyhow::Result<()> {

        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error, follower=[{}]", &self.follower))?;

        let sql_query = format!(r"UPDATE fs_{}.file_reference
                                        SET
                                            original_file_size = :p_original_file_size,
                                            total_part = :p_total_part,
                                            mime_type = :p_mime_type,
                                            is_encrypted = true
                                        WHERE id = :p_file_id "
                                        , customer_code);

        let sequence_name = format!( "fs_{}.file_reference_id_seq", customer_code );

        let mut params = HashMap::new();
        params.insert("p_original_file_size".to_string(), CellValue::from_raw_int(total_size as i64));
        params.insert("p_total_part".to_string(), CellValue::from_raw_int_32(total_part as i32));
        params.insert("p_file_id".to_string(), CellValue::from_raw_int(file_id));
        params.insert("p_mime_type".to_string(), CellValue::from_raw_string(media_type.to_string()));

        let sql_update = SQLChange {
            sql_query,
            params,
            sequence_name,
        };

        let _ = sql_update.update(&mut trans).map_err(err_fwd!("Insertion failed, follower=[{}]", &self.follower))?;

        // End of the 'purple' transaction
        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        Ok(())
    }

    fn delete_from_target_table(&self, target_table: &str,  file_id: i64, customer_code: &str) -> anyhow::Result<()> {
        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error, follower=[{}]", &self.follower))?;

        let sql_delete = format!( "DELETE FROM fs_{}.{} WHERE file_reference_id = :p_file_id", &customer_code, &target_table );

        let mut params = HashMap::new();
        params.insert(
            "p_file_id".to_string(),
            CellValue::from_raw_int(file_id),
        );

        let query = SQLChange {
            sql_query: sql_delete.to_string(),
            params,
            sequence_name: "".to_string(),
        };

        let _ = query
            .delete(&mut trans)
            .map_err(err_fwd!(
            "💣 Query failed, [{}], , follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;
        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        Ok(())
    }

    fn delete_from_file_uploads(&self, file_ref: &str, customer_code: &str) -> anyhow::Result<()> {
        let mut r_cnx = SQLConnection::new();
        let mut trans = open_transaction(&mut r_cnx).map_err(err_fwd!("Open transaction error, follower=[{}]", &self.follower))?;

        let sql_delete = format!( "DELETE FROM fs_{}.file_uploads WHERE file_ref = :p_file_ref", &customer_code);

        let mut params = HashMap::new();
        params.insert(
            "p_file_ref".to_string(),
            CellValue::from_raw_str(&file_ref),
        );

        let query = SQLChange {
            sql_query: sql_delete.to_string(),
            params,
            sequence_name: "".to_string(),
        };

        let _ = query
            .delete(&mut trans)
            .map_err(err_fwd!(
            "💣 Query failed, [{}], , follower=[{}]",
            &query.sql_query,
            &self.follower
        ))?;
        trans.commit().map_err(err_fwd!("💣 Commit failed, follower=[{}]", &self.follower))?;

        log_info!("😎 Committed. Successfully delete file uploads, file_ref=[{}], follower=[{}]", &file_ref, &self.follower);

        Ok(())
    }

    // fn update_file_reference(&self, mut trans: &mut SQLTransaction, file_reference_id: i64) -> anyhow::Result<()> {
    //     let sql_update = format!(
    //         "UPDATE file_reference SET total_part = 0, original_file_size = 0, encrypted_file_size = 0 WHERE id = :p_file_reference_id"
    //     );
    //
    //     let mut params = HashMap::new();
    //     params.insert(
    //         "p_file_reference_id".to_string(),
    //         CellValue::from_raw_int(file_reference_id),
    //     );
    //
    //     let query = SQLChange {
    //         sql_query: sql_update.to_string(),
    //         params,
    //         sequence_name: "".to_string(),
    //     };
    //
    //     let _id = query
    //         .update(&mut trans)
    //         .map_err(err_fwd!(
    //         "💣 Query failed, [{}], , follower=[{}]",
    //         &query.sql_query,
    //         &self.follower
    //     ))?;
    //
    //     Ok(())
    // }


}

//
// cargo test file_server_tests  -- --nocapture
//
#[cfg(test)]
mod file_server_tests {
    use std::collections::HashMap;
    use std::path::Path;
    use std::process::exit;
    use std::sync::Once;

    use commons_services::token_lib::SessionToken;
    use commons_services::x_request_id::XRequestID;

    use crate::FileDelegate;

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


    #[test]
    fn test_1() {
        init_log();
        const N_PARTS: u32 = 100;
        let delegate = FileDelegate::new(SessionToken("MY SESSION".to_owned()), XRequestID::from_value(Option::None));
        let mut enc_parts = HashMap::new();
        for index in 0..N_PARTS {
            let v = "0000".to_string();
            enc_parts.insert(index, v);
        }
        // dbg!(&enc_parts);
        let r = delegate.parallel_decrypt(enc_parts, "MY_CUSTOMER_KEY").unwrap();
        for i in 0..N_PARTS {
            println!("{} -> {:?}", i, r.get(&i).unwrap());
        }
    }


    #[test]
    fn test_2() {

        fn calculate_code(text: &str) -> String {
            let mut code_bytes: Vec<u8> = Vec::new();

            for byte in text.bytes() {
                let item = (byte % 26) + b'a';
                code_bytes.push(item);
            }

            String::from_utf8_lossy(&code_bytes).into_owned()
        }

        let input_text = "111-snow image bright.jpg";
        let code = calculate_code(input_text);
        println!("{}", code);

    }

}
